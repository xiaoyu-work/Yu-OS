import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import * as ai from "./ai";
import * as tools from "./tools";
import { WireDecimal, stringifyWireJson } from "./generated";
import {
  installFakeYu,
  wireError,
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
  assert.equal(res.text, "hello there");
  assert.equal(res.usage.units, 8);
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
