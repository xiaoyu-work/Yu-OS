# @yuos/sdk

Official **Node.js** SDK for [Yu OS](https://github.com/xiaoyu-work/Yu-OS).

This package is the public Node.js surface for building Yu OS Apps.
It provides a manifest-bound MCP service runtime, gated AI and catalog
tool clients, and desktop GUI bootstrap. Tool schemas come exclusively
from the App's verified `app.json`; Node code binds implementations to
those declared names.

## Install

From your App's Node.js project (Node.js 18 or later):

```sh
npm install \
  https://github.com/xiaoyu-work/Yu-OS/releases/download/yuos-sdk-v0.1.0/yuos-sdk-node-0.1.0.tgz
```

Live calls require the [Yu OS runtime](../REFERENCE.md#compatibility).

## Modules

| Module       | What                                                          | Equivalent CLI                       |
|--------------|---------------------------------------------------------------|--------------------------------------|
| `app`        | Command-line and desktop functions (`command`, `desktop`, `run`) | `yu app <id> <command>` / `--gui` |
| `mcp`        | Manifest-bound MCP App service runtime over private stdio      | launched by the Yu OS App Host        |
| `ai`         | `chat`, `embed`, `tts`, `stt`, `imagegen`, and `decide` | `yu ai chat` / `yu ai infer` |
| `tools`      | Explicit catalog operations (`call`, `catalog`)               | `yu ai tool <name> --app <id>`      |
| `gui`        | Desktop GUI bootstrap (kernel context, agent overlay)          | launched via `yu app <id> --gui`    |
| `objects`    | Pure canonical App object identifiers                        | No transport or dispatch           |
| `manifest`   | Pure manifest path validation and preview                        | No transport or grants             |
| `events`, `channels` | Report App events and control authorized App-owned channels | —                               |
| `transport`  | The subprocess transport + error types (advanced)              | —                                    |
| (root)       | Typed structs generated from `wire/v1/*.schema.json`           | —                                    |

## Command line and desktop

Set `"runtime": "node"` and register the function for each version the App
declares in `app.json`:

```js
const { app, chat } = require("@yuos/sdk");

app.command("summarize", async (args) => ({ summary: (await chat(args[0])).output.text }));
app.desktop((files) => { /* open your window */ });

app.run().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
```

See [Command line and desktop](../REFERENCE.md#command-line-and-desktop).

## MCP App service

Use the [MCP example manifest](../../docs/app-development.md#quickstart-an-mcp-only-app) with `"runtime": "node"` and
`mcp.entry` set to `server.cjs`. Include the entrypoint and its runtime
dependencies in the App before signing.

```js
const { mcp } = require("@yuos/sdk");
const app = mcp.App.fromManifest();

app.tool("hello-mcp.say", ({ message }, call) => {
  call.throwIfCancelled();
  return mcp.ToolResult.text(message);
});

app.serve().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
```

`App.fromManifest(path?)` reads one immutable manifest snapshot. An explicit
path wins, followed by `YUOS_APP_MANIFEST`, then `./app.json` for direct
development. A local manifest does not supply an authenticated call context.
Every name in `app.json.mcp.tools` must be bound before serving.
Descriptions, argument schemas, defaults, choices, and conditional requirements
remain manifest-owned.

Every call carries a Gateway-authenticated `mcp.CallContext`. Handlers receive
it as their second argument and can also use `mcp.currentContext()`. The context
provides immutable caller and lineage fields, an `AbortSignal`, deadline-aware
`throwIfCancelled()`, and progress reporting when the caller supplied a progress
token.

## AI and catalog tools

AI and catalog calls return promises. In an MCP handler, pass `call.signal`
to propagate cancellation and the deadline:

```ts
const reply = await ai.chat(body, { signal: call.signal });
```

`tools.call`, `tools.catalog`, and `ai.budget` also accept `{ signal }`
in their options. `ai.budget()` reports the App, period, and units used.
Check the outcome of an accepted mutation before retrying it.

## App object references

```ts
import { objects, type ObjectRef } from "@yuos/sdk";

const reference: ObjectRef = {
  app_id: "notes",
  object_type: "note",
  object_id: "draft/1",
};
const uri = objects.format_reference(reference);
const parsed = objects.parse_reference(uri);
// uri === "app://notes/note?id=draft%2F1"; parsed.object_id === "draft/1"
```

CamelCase aliases `objects.formatReference` and `objects.parseReference` are
also exported. Helpers validate the generated structure and semantic bounds,
reject unpaired UTF-16 surrogates rather than replacement-encoding them, and
preserve opaque IDs unchanged. They perform no discovery, filesystem access,
or dispatch, and references do not assert existence, freshness, or permission.

See [the shared contract](../wire/v1/object-references.md) for canonical
spelling, byte limits, and the optional manifest `objects` resolver declaration.

## AI support

Import named calls directly from `@yuos/sdk`. Each model reply has
`provider`, `model`, `units`, and `output`. See the
[model capabilities](../REFERENCE.md#ai-support) and
[typed errors](../REFERENCE.md#handle-ai-errors).
`await ai.openPermissionSettings()` opens the [model permission page](../../docs/app-development.md#model-permissions-in-settings)
after a user action and with the separate desktop launcher permission.

## Errors

Transport-facing domain errors extend `transport.BridgeError`:

- AI errors carry the server's `code` and `payload` when available.
- `tools.ToolUnavailable` — transport failure (binary missing, timeout,
  non-JSON output).
- `tools.ToolDenied` — capability / unknown-tool / arg-shape refusal.

Pure reference validation throws `objects.ObjectRefError`, which extends
`Error` directly and does not imply a transport call was attempted.

## Binary resolution

The SDK runs `yu` from `$PATH`. Override with `YUOS_KERNEL_BIN` for
tests and dev setups.

## Develop

```sh
npm install
npm run build      # tsc → dist/ (shipped; no test files)
npm test           # compiles + runs node:test, including in-memory MCP transport tests
```

Generated types in `src/generated.ts` are produced by
`python3 ../wire/codegen.py` — do not hand-edit them.
