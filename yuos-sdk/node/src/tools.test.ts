import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";

import * as tools from "./tools";
import * as events from "./events";
import * as channels from "./channels";
import {
  WireDecimal,
  WireJsonSerializationError,
  stringifyWireJson,
} from "./generated";
import {
  installFakeYu,
  wireError,
  wireSuccessJson,
  withCos,
} from "./testutil";

test("App events keep business payload on stdin without caller identity selectors", async () => {
  const fake = installFakeYu(wireSuccessJson('{"accepted":true}'));
  const captured = fake.bin + ".stdin";
  const script = readFileSync(fake.bin, "utf8").replace(
    "process.stdout.write(",
    `fs.writeFileSync(${JSON.stringify(captured)}, fs.readFileSync(0));\nprocess.stdout.write(`,
  );
  writeFileSync(fake.bin, script);
  const result = await withCos(fake, {}, () =>
    events.publish("changed", "stable", { payload: { object: "app://fs/file?id=%2Ftmp%2Ffile" } }),
  );
  assert.deepEqual(result, { accepted: true });
  assert.deepEqual(readFileSync(fake.argvOut, "utf8").split("\n"), ["--wire=1", "__app_event", "publish"]);
  assert.deepEqual(JSON.parse(readFileSync(captured, "utf8")), {
    event_type: "changed", key: "stable", payload: { object: "app://fs/file?id=%2Ftmp%2Ffile" },
  });
});

test("channel replies keep only verified event text and confirmation on stdin", async () => {
  const fake = installFakeYu(wireSuccessJson('{"submitted":true,"delivery_confirmed":false}'));
  const captured = fake.bin + ".stdin";
  writeFileSync(fake.bin, readFileSync(fake.bin, "utf8").replace(
    "process.stdout.write(",
    `fs.writeFileSync(${JSON.stringify(captured)}, fs.readFileSync(0));\nprocess.stdout.write(`,
  ));
  const result = await withCos(fake, {}, () =>
    channels.control("reply", { eventId: "channel-event-synthetic", text: "Synthetic reply", confirm: true }),
  );
  assert.deepEqual(result, { submitted: true, delivery_confirmed: false });
  assert.deepEqual(readFileSync(fake.argvOut, "utf8").split("\n"), ["--wire=1", "__channel", "control"]);
  assert.deepEqual(JSON.parse(readFileSync(captured, "utf8")), {
    action: "reply", event_id: "channel-event-synthetic", text: "Synthetic reply", confirm: true,
  });
});

test("call builds argv and parses the result", async () => {
  const fake = installFakeYu(
    wireSuccessJson(
      JSON.stringify({
        tool: "fs.read_text",
        app_id: "notes",
        status: "ok",
        result: { text: "hi" },
      }),
    ),
  );
  const res = await withCos(fake, { YUOS_APP_ID: "notes" }, () =>
    tools.call("fs.read_text", { path: "/etc/hostname" }),
  );
  assert.equal(res.name, "fs.read_text");
  assert.equal(res.status, "ok");
  assert.deepEqual(res.value, { text: "hi" });

  const argv = readFileSync(fake.argvOut, "utf8").split("\n");
  assert.deepEqual(argv.slice(0, 6), [
    "--wire=1",
    "ai",
    "tool",
    "fs.read_text",
    "--app",
    "notes",
  ]);
  const argsIdx = argv.indexOf("--args");
  assert.ok(argsIdx >= 0);
  assert.deepEqual(JSON.parse(argv[argsIdx + 1]), { path: "/etc/hostname" });
});

test("call maps an error envelope to ToolDenied", async () => {
  const fake = installFakeYu(wireError("unknown tool", "UNKNOWN_TOOL"), 1);
  await assert.rejects(
    () => withCos(fake, { YUOS_APP_ID: "notes" }, () => tools.call("nope")),
    tools.ToolDenied,
  );
});

test("call requires an app id", async () => {
  await assert.rejects(
    () => withCos(installFakeYu("{}"), { YUOS_APP_ID: undefined }, () => tools.call("x")),
    tools.ToolError,
  );
});

test("catalog parses tool rows", async () => {
  const lexeme = "0.12345678901234567890";
  const fake = installFakeYu(wireSuccessJson(`{"tools":[{
    "name":"fs.read_text","summary":"read a file","verb":"fs.read",
    "stability":"stable","args_schema":{"type":"number","minimum":${lexeme}},
    "returns_schema":{"type":"number","maximum":${lexeme}}
  }]}`));
  const rows = await withCos(fake, {}, () => tools.catalog());
  assert.equal(rows.length, 1);
  assert.equal(rows[0].verb, "fs.read");
  assert.ok(rows[0].argsSchema?.minimum instanceof WireDecimal);
  assert.throws(() => JSON.stringify(rows[0].argsSchema), /stringifyWireJson/);
  assert.equal(stringifyWireJson(rows[0].argsSchema!), `{"type":"number","minimum":${lexeme}}`);
  assert.equal(stringifyWireJson(rows[0].returnsSchema!), `{"type":"number","maximum":${lexeme}}`);
  // catalog takes no --app
  const argv = readFileSync(fake.argvOut, "utf8").split("\n").filter(Boolean);
  assert.deepEqual(argv, ["--wire=1", "ai", "tools"]);
});

test("call preserves explicit null, scalar, and array arguments", async () => {
  for (const args of [null, "scalar", [1, true]]) {
    const fake = installFakeYu(
      wireSuccessJson(
        JSON.stringify({ tool: "echo", app_id: "notes", status: "ok", result: null }),
      ),
    );
    await withCos(fake, {}, () => tools.call("echo", args, { appId: "notes" }));
    const argv = readFileSync(fake.argvOut, "utf8").split("\n");
    assert.equal(argv[argv.indexOf("--args") + 1], stringifyWireJson(args));
  }
});

test("tool result preserves unsafe fractional lexemes", async () => {
  const lexeme = "0.12345678901234567890";
  const fake = installFakeYu(
    wireSuccessJson(
      `{"tool":"echo","app_id":"notes","status":"ok","result":${lexeme}}`,
    ),
  );
  const result = await withCos(fake, {}, () => tools.call("echo", {}, { appId: "notes" }));
  assert.ok(result.value instanceof WireDecimal);
  assert.equal(stringifyWireJson(result.value), lexeme);
});

test("tool invocation rejects unsafe native numeric arguments", async () => {
  for (const args of [Number.NaN, Number.POSITIVE_INFINITY, Number.MAX_SAFE_INTEGER + 1]) {
    await assert.rejects(
      () => tools.call("echo", args, { appId: "notes" }),
      WireJsonSerializationError,
    );
  }
});

test("forChat trims and drops empties", () => {
  assert.deepEqual(tools.forChat("fs.read_text", " kv.get ", ""), ["fs.read_text", "kv.get"]);
});
