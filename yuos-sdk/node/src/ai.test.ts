import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import * as ai from "./ai";
import * as tools from "./tools";
import { WireDecimal, stringifyWireJson } from "./generated";
import {
  installFakeYu,
  wireError,
  wireSuccess,
  wireSuccessJson,
  withCos,
} from "./testutil";

const OK_CHAT_DATA = JSON.stringify({
  verb: "ai.chat",
  text: "hello there",
  model: "m",
  provider: "p",
  usage: { input_tokens: 3, output_tokens: 5, units: 8 },
  budget: { period: "2026-05", units_used: 8, units_cap: 1000 },
  review: { safety: "strict", prompt_redacted: false },
  tool_calls: [{ id: "c1", name: "fs.read_text", input: { path: "/x" } }],
});
const OK_CHAT = wireSuccessJson(OK_CHAT_DATA);
const trusted = { origin: "trusted" } as const;

test("chat builds the right argv and parses the envelope", async () => {
  const fake = installFakeYu(OK_CHAT);
  const res = await withCos(fake, { YUOS_APP_ID: "notes" }, () =>
    ai.chat("summarise", { origin: "external-content", maxUnits: 100, tools: ["fs.read_text"] }),
  );
  assert.equal(res.output.text, res.text);
  assert.equal(res.text, "hello there");
  assert.equal(res.units, res.usage.units);
  assert.equal(res.units, 8);
  assert.equal(res.budget.unitsCap, 1000);
  assert.equal(res.toolCalls.length, 1);
  assert.equal(res.toolCalls[0].name, "fs.read_text");

  const argv = readFileSync(fake.argvOut, "utf8").split("\n");
  assert.deepEqual(argv.slice(0, 7), [
    "--wire=1",
    "ai",
    "chat",
    "--app",
    "notes",
    "--origin",
    "external-content",
  ]);
  assert.ok(argv.includes("--prompt-file"));
  assert.ok(argv.includes("--max-units"));
  assert.ok(argv.includes("--tools"));
});

test("chat rejects an empty prompt", async () => {
  await assert.rejects(() => ai.chat("  ", trusted), /prompt must be non-empty/);
});

test("model calls take and return media bytes", async () => {
  const png = Uint8Array.from(Buffer.from("\x89PNG\r\n\x1a\nfixture", "latin1"));
  const wav = Uint8Array.from(Buffer.from("RIFF\x24\0\0\0WAVEfmt ", "latin1"));
  const base64 = (bytes: Uint8Array) => Buffer.from(bytes).toString("base64");
  const inference = (output: object) => wireSuccess({ provider: "p", model: "m", units: 50, output });
  const sentRequest = (path: string) => JSON.parse(readFileSync(path, "utf8"));

  const chatFake = installFakeYu(OK_CHAT, 0, { recordStdin: true });
  await withCos(chatFake, { YUOS_APP_ID: "notes" }, () => ai.chat("Describe this image.", { images: [png] }));
  assert.deepEqual(sentRequest(chatFake.stdinOut).images, [base64(png)]);

  const sttFake = installFakeYu(inference({ kind: "stt", text: "hello" }), 0, { recordStdin: true });
  assert.equal((await withCos(sttFake, {}, () => ai.stt(wav))).output.text, "hello");
  assert.deepEqual(sentRequest(sttFake.stdinOut).input, { kind: "stt", audio_base64: base64(wav) });

  const speech = await withCos(
    installFakeYu(inference({ kind: "tts", format: "wav", audio_base64: base64(wav) })), {}, () => ai.tts("Hello"),
  );
  assert.deepEqual(speech.output, { format: "wav", audio: wav });
  const image = await withCos(
    installFakeYu(inference({ kind: "imagegen", format: "png", image_base64: base64(png) })), {}, () => ai.imagegen("A cat"),
  );
  assert.deepEqual(image.output, { format: "png", image: png });
  await assert.rejects(() => ai.stt(base64(wav) as unknown as Uint8Array), ai.AiInvalidArgs);
});

test("chat requires an app id", async () => {
  const fake = installFakeYu(OK_CHAT);
  await assert.rejects(
    () => withCos(fake, { YUOS_APP_ID: undefined }, () => ai.chat("hi", trusted)),
    ai.AiError,
  );
});

test("budget-exceeded code maps to AiBudgetExceeded", async () => {
  const fake = installFakeYu(wireError("opaque", "BUDGET_EXCEEDED"), 1);
  await assert.rejects(
    () => withCos(fake, { YUOS_APP_ID: "notes" }, () => ai.chat("hi", trusted)),
    ai.AiBudgetExceeded,
  );
});

test("safety code maps to AiSafetyViolation", async () => {
  const fake = installFakeYu(wireError("opaque", "SAFETY_VIOLATION"), 1);
  await assert.rejects(
    () => withCos(fake, { YUOS_APP_ID: "notes" }, () => ai.chat("hi", trusted)),
    ai.AiSafetyViolation,
  );
});

test("generic error envelope maps to AiDenied", async () => {
  const fake = installFakeYu(wireError("capability denied", "PERMISSION_DENIED"), 1);
  await assert.rejects(
    () => withCos(fake, { YUOS_APP_ID: "notes" }, () => ai.chat("hi", trusted)),
    ai.AiDenied,
  );
});

test("stable error code determines the typed error", async () => {
  const budget = installFakeYu(wireError("safety words", "BUDGET_EXCEEDED"), 1);
  await assert.rejects(
    () => withCos(budget, { YUOS_APP_ID: "notes" }, () => ai.chat("hi", trusted)),
    ai.AiBudgetExceeded,
  );

  const safety = installFakeYu(wireError("budget words", "SAFETY_VIOLATION"), 1);
  await assert.rejects(
    () => withCos(safety, { YUOS_APP_ID: "notes" }, () => ai.chat("hi", trusted)),
    ai.AiSafetyViolation,
  );

  const unavailable = installFakeYu(wireError("opaque", "KERNEL_UNAVAILABLE"), 1);
  await assert.rejects(
    () => withCos(unavailable, { YUOS_APP_ID: "notes" }, () => ai.chat("hi")),
    (error: unknown) => error instanceof ai.AiUnavailable && error.code === "KERNEL_UNAVAILABLE",
  );
  assert.deepEqual(readFileSync(unavailable.argvOut, "utf8").trim().split("\n"),
    ["--wire=1", "ai", "chat", "--request-stdin"]);
  const invalid = installFakeYu(wireError("opaque", "INVALID_ARGS"), 1);
  await assert.rejects(() => withCos(invalid, { YUOS_APP_ID: "notes" }, () => ai.embed("text")),
    ai.AiInvalidArgs);
});

test("malformed tool call rejects the entire response", async () => {
  const payload = JSON.parse(OK_CHAT_DATA) as Record<string, unknown>;
  payload.tool_calls = [{ id: "c1", input: {} }];
  const fake = installFakeYu(wireSuccessJson(JSON.stringify(payload)));
  await assert.rejects(
    () => withCos(fake, { YUOS_APP_ID: "notes" }, () => ai.chat("hi", trusted)),
    (error: unknown) =>
      error instanceof ai.AiUnavailable &&
      error.message.includes("WIRE_REQUIRED") &&
      error.message.includes("$.tool_calls[0].name"),
  );
});

test("chat preserves unrestricted tool input and mathematical integers", async () => {
  const payload = wireSuccessJson(OK_CHAT_DATA
    .replace('"input_tokens":3', '"input_tokens":1.0')
    .replace('"output_tokens":5', '"output_tokens":1e0')
    .replace('"units":8', '"units":18446744073709551615')
    .replace('"input":{"path":"/x"}', '"input":["a",1]'));
  const fake = installFakeYu(payload);
  const response = await withCos(fake, { YUOS_APP_ID: "notes" }, () => ai.chat("hi", trusted));
  assert.equal(response.usage.inputTokens, 1);
  assert.equal(response.usage.outputTokens, 1);
  assert.equal(response.usage.units, 18446744073709551615n);
  assert.deepEqual(response.toolCalls[0].input, ["a", 1]);
});

test("chat rejects scalar root with stable wire error", async () => {
  const fake = installFakeYu("null");
  await assert.rejects(
    () => withCos(fake, { YUOS_APP_ID: "notes" }, () => ai.chat("hi", trusted)),
    (error: unknown) =>
      error instanceof ai.AiUnavailable &&
      error.message.includes("WIRE_ONE_OF") &&
      error.message.includes(" at $:"),
  );
});

test("budget accepts the producer budget-show shape", async () => {
  const fake = installFakeYu(
    wireSuccessJson(JSON.stringify({ app: "notes", period: "2026-08", units_used: 7 })),
  );
  const result = await withCos(fake, {}, () => ai.budget("notes"));
  assert.deepEqual(result, { app: "notes", period: "2026-08", unitsUsed: 7 });
});

test("proposed lossless input round-trips directly through tools.call", async () => {
  const lexeme = "0.12345678901234567890";
  const chatFake = installFakeYu(
    wireSuccessJson(OK_CHAT_DATA.replace('"input":{"path":"/x"}', `"input":${lexeme}`)),
  );
  const response = await withCos(chatFake, { YUOS_APP_ID: "notes" }, () => ai.chat("hi", trusted));
  assert.ok(response.toolCalls[0].input instanceof WireDecimal);
  assert.equal(stringifyWireJson(response.toolCalls[0].input), lexeme);

  const toolFake = installFakeYu(
    wireSuccessJson(
      JSON.stringify({ tool: "echo", app_id: "notes", status: "ok", result: null }),
    ),
  );
  await withCos(toolFake, {}, () =>
    tools.call("echo", response.toolCalls[0].input, { appId: "notes" }),
  );
  const argv = readFileSync(toolFake.argvOut, "utf8").split("\n");
  assert.equal(argv[argv.indexOf("--args") + 1], lexeme);
});

test("dev AI mode calls local HTTP endpoint when YUOS_SESSION is unset and ignores it when YUOS_SESSION is set", async () => {
  const { createServer } = await import("node:http");
  const seen: Array<{ url: string; auth?: string; body: Record<string, unknown> }> = [];
  const server = createServer((req, res) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk: Buffer) => chunks.push(chunk));
    req.on("end", () => {
      const body = JSON.parse(Buffer.concat(chunks).toString("utf8")) as Record<string, unknown>;
      seen.push({
        url: req.url ?? "",
        auth: req.headers.authorization,
        body,
      });
      res.setHeader("Content-Type", "application/json");
      if (req.url === "/v1/chat/completions") {
        res.end(
          JSON.stringify({
            model: "llama3.2",
            choices: [{ message: { role: "assistant", content: "dev reply" } }],
            usage: { prompt_tokens: 2, completion_tokens: 3, total_tokens: 5 },
          }),
        );
      } else if (req.url === "/v1/embeddings") {
        res.end(
          JSON.stringify({
            model: "llama3.2",
            data: [{ embedding: [0.1, 0.2, 0.3] }],
            usage: { total_tokens: 3 },
          }),
        );
      } else {
        res.statusCode = 404;
        res.end();
      }
    });
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;
  const devUrl = `http://127.0.0.1:${port}/v1/`;

  const savedEnv = {
    YUOS_DEV_AI_URL: process.env.YUOS_DEV_AI_URL,
    YUOS_DEV_AI_KEY: process.env.YUOS_DEV_AI_KEY,
    YUOS_SESSION: process.env.YUOS_SESSION,
  };
  try {
    const fake = installFakeYu(OK_CHAT);
    process.env.YUOS_DEV_AI_URL = devUrl;
    process.env.YUOS_DEV_AI_KEY = "dev-token";
    delete process.env.YUOS_SESSION;

    const chatRes = await withCos(fake, { YUOS_APP_ID: undefined }, () => ai.chat("hello dev"));
    assert.equal(chatRes.text, "dev reply");
    assert.equal(chatRes.provider, "dev");
    assert.equal(chatRes.model, "llama3.2");
    assert.equal(chatRes.units, 5);

    const embedRes = await withCos(fake, { YUOS_APP_ID: undefined }, () => ai.embed("embed text"));
    assert.equal(embedRes.provider, "dev");
    assert.deepEqual(embedRes.output.vector, [0.1, 0.2, 0.3]);

    assert.equal(seen.length, 2);
    assert.equal(seen[0].url, "/v1/chat/completions");
    assert.equal(seen[0].auth, "Bearer dev-token");
    assert.equal(seen[1].url, "/v1/embeddings");

    // When YUOS_SESSION is set, YUOS_DEV_AI_URL must be ignored and `yu` binary used.
    seen.length = 0;
    process.env.YUOS_SESSION = "sandbox-session-1";
    const sandboxRes = await withCos(fake, { YUOS_APP_ID: "notes" }, () => ai.chat("hello sandbox"));
    assert.equal(sandboxRes.text, "hello there");
    assert.equal(sandboxRes.provider, "p");
    assert.equal(seen.length, 0);
  } finally {
    for (const [k, v] of Object.entries(savedEnv)) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});
