# Yu OS SDK Reference

New to the SDK? Start with the [Quickstart](QUICKSTART.md), or see
[Integrating an Existing Linux App](../docs/linux-app-integration.md).

**Contents**
- [Installation](#installation)
- [Modules overview](#modules-overview)
- [Command line and desktop (`app`)](#command-line-and-desktop)
- [Desktop dialogs and context (`gui`)](#gui-api-yuos_sdkgui)
- [System Agent tools (`mcp`)](#mcp-api-yuos_sdkmcp)
- [Permissions](#permissions)
- [AI models (`ai`)](#ai-support)
- [Object references, file plans, and effects](#app-object-references)
- [CLI commands](#cli-commands)

## Installation

On Yu OS, all SDK runtimes and the `yu` CLI are preinstalled. For standalone
development on macOS, Windows, or Linux:

<details open>
<summary><b>Python</b> (3.10+)</summary>

```sh
pip install yuos-sdk
```

</details>

<details>
<summary><b>Node.js / TypeScript</b> (Node 18+)</summary>

```sh
npm install @yuos/sdk
```

</details>

<details>
<summary><b>Go</b> (Go 1.21+)</summary>

```sh
go get github.com/xiaoyu-work/Yu-OS/yuos-sdk/go@v0.1.0
```

</details>

<details>
<summary><b>Rust</b> (Edition 2021)</summary>

```sh
cargo add yuos-sdk
```

</details>

### Environment variables

| Variable | Scope | Description |
|---|---|---|
| `YUOS_APP_ID` | Runtime | App identity set automatically by `yuosd` or `yu app dev`. |
| `YUOS_DATA_DIR` | Runtime | Private per-App storage path returned by `app.data_dir()`. |
| `YUOS_DEV_AI_URL` | Local dev | OpenAI-compatible `/v1` base URL (e.g. `http://localhost:11434/v1`) used when outside a `YUOS_SESSION`. |
| `YUOS_DEV_AI_MODEL` | Local dev | Model name for `YUOS_DEV_AI_URL` (default `llama3.2`). |
| `YUOS_DEV_AI_KEY` | Local dev | Optional Bearer token for `YUOS_DEV_AI_URL`. |

## Modules overview

| Module | Python | Node.js | Go | Rust | Purpose |
|---|---|---|---|---|---|
| **App** | `yuos_sdk.app` | `app` | `yuossdk` | `yuos_sdk::app` | `@command`, `@desktop`, `run()`, `data_dir()`, `notify()` |
| **AI** | `yuos_sdk.ai` | `ai` | `yuossdk` | `yuos_sdk::ai` | `chat`, `chat_stream`, `embed`, `tts`, `stt`, `imagegen`, `decide`, `budget` |
| **GUI** | `yuos_sdk.gui` | `gui` | `yuossdk` | `yuos_sdk::gui` | `open_file()`, `context()`, `open_agent_overlay()`, `is_gui_launch()` |
| **MCP** | `yuos_sdk.mcp` | `mcp` | `yuossdk` | `yuos_sdk::mcp` | Expose App tools to the built-in System Agent |
| **Objects** | `yuos_sdk.objects` | `objects` | `yuossdk` | `yuos_sdk::objects` | Canonical `app://` object references (`ObjectRef`) |
| **Events** | `yuos_sdk.events` | `events` | `yuossdk` | `yuos_sdk::events` | Publish App events (`ipc.publish` on `app:<id>`) |

## The model

The built-in System Agent discovers and calls enabled App MCP tools.
Owners use an App's [command line or desktop window](#command-line-and-desktop).
Each App runs with its own capabilities.

AI and catalog calls use `yu --wire=1`. The OS authenticates the registered
workload and checks its capabilities. App MCP tools run over private stdio
managed by the App Host. Read caller identity from the Gateway's call context.
See [App, Agent, and AI integration](../docs/app-ai-integration.md).

Manifest helpers validate declarations such as `owner-path`.
[Named GUI entries and optional needs](../docs/app-development.md#multiple-gui-entries-under-one-app-identity)
retain one App identity. The OS resolves scopes, grants capabilities, and
mounts resources.

For other capabilities, see [App events](wire/v1/app-events.md),
[managed channels](wire/v1/channel-ingress.md), and
[App data services](wire/v1/applet-services.md). Each requires the applicable
OS service and grant.

## Command line and desktop

Declare `operations` once in `app.json`; command-line and desktop launches share
its permissions. In code, mark the command-line function with `@command` and the
desktop function with `@desktop`:

| Surface | Started by | Python | If omitted in code |
|---|---|---|---|
| Command line | `yu app <id> <command> [args]` | `@command` | Returns `{"available": false, "surface": "cli", ...}` |
| Desktop | The App launcher or `yu app <id> --gui` | `@desktop` | Returns `{"available": false, "surface": "desktop", ...}` |

```json
{
  "id": "hello-ai",
  "version": "0.1.0",
  "name": {"en": "Hello AI"},
  "ai": {},
  "operations": {
    "summarize": {
      "label": {"en": "Summarize text"},
      "args": [{"name": "text", "kind": "text", "required": true}],
      "needs": ["ai.chat"]
    }
  }
}
```

<details open>
<summary><b>Python</b></summary>

```python
from yuos_sdk.ai import chat
from yuos_sdk.app import command, desktop

def summarize_text(text: str) -> str:
    return chat(text, system="Summarize in one short sentence.").output.text

@command
def summarize(args):
    return {"summary": summarize_text(args[0])}

@desktop
def main():
    ...  # Open your window with any GUI toolkit and call summarize_text(...).
```

</details>

<details>
<summary><b>Node.js / TypeScript</b></summary>

```ts
import { ai, app } from "@yuos/sdk";

async function summarizeText(text: string): Promise<string> {
  return (await ai.chat(text, { system: "Summarize in one short sentence." })).output.text;
}

app.command(async function summarize(args) {
  return { summary: await summarizeText(args[0]) };
});

app.desktop(async () => {
  // Open your window with any GUI toolkit and call summarizeText(...).
});

app.run();
```

</details>

<details>
<summary><b>Go</b></summary>

```go
yuossdk.HandleCommand("summarize", func(args []string) (any, error) {
	reply, err := yuossdk.Chat(args[0], yuossdk.ChatOptions{System: "Summarize in one short sentence."})
	if err != nil {
		return nil, err
	}
	return map[string]any{"summary": reply.Output.Text}, nil
})
yuossdk.HandleDesktop(func() error {
	// Open your window with any GUI toolkit.
	return nil
})
_ = yuossdk.RunApp()
```

</details>

<details>
<summary><b>Rust</b></summary>

```rust
use yuos_sdk::{ai, app};

app::App::new()
    .command("summarize", |args| {
        let reply = ai::chat(&args[0], ai::ChatOpts::new().system("Summarize in one short sentence."))?;
        Ok(serde_json::json!({ "summary": reply.output.text }))
    })
    .desktop(|| {
        // Open your window with any GUI toolkit.
        Ok(())
    })
    .run()?;
```

</details>

`@command` uses the function name as the command name (or pass `@command("name")`
to customize it). A command function receives the validated arguments after the
command name and returns a JSON value. `@desktop` marks the desktop main entry
(file-viewer Apps that open associated files can read `gui.context().files`). If
an App only implements `@command` or only implements `@desktop`, launching the
other surface returns the built-in unavailable-surface response automatically.

Node uses `app.command(fn)`, `app.desktop(fn)`, and `app.run()`. Go uses
`HandleCommand`, `HandleDesktop`, and `RunApp()`. Rust uses
`app::App::new().command(name, fn).desktop(fn).run()`.
`yu app create <id> --kind both` creates an App with both versions.

### App API (`yuos_sdk.app`)

| Python | Node.js | Go | Rust | Description |
|---|---|---|---|---|
| `@command` / `@command(name)` | `app.command(fn)` / `app.command(name, fn)` | `HandleCommand(name, fn)` | `App::command(name, fn)` | Register a CLI command handler (`args -> dict`). |
| `@desktop` | `app.desktop(fn)` | `HandleDesktop(fn)` | `App::desktop(fn)` | Register the zero-argument desktop entry (`() -> None`). |
| `run()` | `await app.run()` | `RunApp()` | `App::run()` | Dispatch the current launch (or return the built-in unavailable-surface fallback). |
| `data_dir() -> Path` | `app.dataDir(): string` | `DataDir() (string, error)` | `app::data_dir() -> io::Result<PathBuf>` | Return (and create) this App's private data directory. |
| `notify(summary, body="")` | `app.notify(summary, body)` | `Notify(summary, body)` | `app::notify(summary, body)` | Post a system notification (`ui.notify`). |
| `clipboard_read() -> str` | `app.clipboardRead(): string` | `ClipboardRead() (string, error)` | `app::clipboard_read() -> Result<String, BridgeError>` | Read text from the system clipboard (`clipboard.read`). |
| `clipboard_write(text)` | `app.clipboardWrite(text)` | `ClipboardWrite(text) error` | `app::clipboard_write(text) -> Result<(), BridgeError>` | Write text to the system clipboard (`clipboard.write`). |

### GUI API (`yuos_sdk.gui`)

| Python | Node.js | Go | Rust | Description |
|---|---|---|---|---|
| `gui.open_file(title="Open file") -> str \| None` | `gui.openFile(title): string \| null` | `OpenFile(title) (string, error)` | `gui::open_file(title) -> Result<Option<String>, String>` | Open the system file picker and return the selected file path (or `None` if cancelled). |
| `gui.save_file(default_name, title="Save file") -> str \| None` | `gui.saveFile(defaultName, title): string \| null` | `SaveFile(defaultName, title) (string, error)` | `gui::save_file(default_name, title) -> Result<Option<String>, String>` | Open the system Save dialog (`ui.prompt`) and return the chosen path. |
| `gui.select_folder(title="Select folder") -> str \| None` | `gui.selectFolder(title): string \| null` | `SelectFolder(title) (string, error)` | `gui::select_folder(title) -> Result<Option<String>, String>` | Open the system folder picker (`ui.prompt`) and return the selected directory. |
| `gui.context() -> GuiContext` | `gui.context(): GuiContext` | `Context(nil) (*GuiContext, error)` | `gui::Context::from_env()` | Read `app_id`, associated `files`, and `open_agent_overlay(hint)`. |
| `gui.is_gui_launch() -> bool` | `gui.isGuiLaunch(): boolean` | `IsGuiLaunch() bool` | `gui::is_gui_launch() -> bool` | Return `True` when started as a desktop GUI (`YUOS_APP_GUI=1`). |

### MCP API (`yuos_sdk.mcp`)

| Python | Node.js | Go | Rust | Description |
|---|---|---|---|---|
| `App.from_manifest()` | `mcp.App.fromManifest()` | `LoadMCPApp()` | `mcp::App::from_environment()` | Load `app.json` and initialize the MCP service runtime. |
| `@app.tool(name)` | `app.tool(name, fn)` | `app.Bind(name, fn)` | `app.bind(tool)` | Bind a handler to a tool declared in `mcp.tools`. |
| `app.serve()` | `await app.serve()` | `app.ServeStdio()` | `app.serve_stdio().await` | Run the JSON-RPC stdio server loop for the App Host. |
| `ToolResult.text(s)` / `ToolResult.json(v)` | `mcp.ToolResult.text(s)` | `TextResult(s)` | `ToolResult::text(s)` | Construct a structured tool reply. |

## Desktop defaults

Apps get a private data directory. Ordinary signed Apps launched by the owner
get a [public-only outbound TCP proxy](../docs/app-development.md#10-what-an-operation-can-reach).

In a signed GUI, the [system Open dialog](../docs/app-development.md#open-a-user-selected-file)
grants the requesting window read access to the exact file the user selects.
The selection is the user's approval.

## Permissions

Like macOS and Windows, Yu OS separates **App permissions** from **system
permissions**. Declare both in `needs`. AI permissions need no extra fields and
can be written directly as strings (`"needs": ["ai.chat"]`); permissions that
take a `scope` or `why` use an object (`{"verb": "fs.read", "scope": ..., "why": ...}`).

Any signed App may declare App permissions. The owner allows or denies them.

| App permission | macOS | Windows |
|---|---|---|
| `fs.read`, `fs.write`, `fs.delete`, `fs.meta`, `fs.watch` | Files and Folders | File system and library capabilities |
| `ui.prompt` (system Save and Folder dialogs) | User-selected files | File and folder pickers |
| `net.dial`, `net.resolve`, `net.probe` | Outgoing connections (client) | `internetClient` |
| `net.listen` | Incoming connections (server) | `internetClientServer` |
| `device.camera` | Camera | `webcam` |
| `device.microphone` | Microphone | `microphone` |
| `device.location` | Location Services | `location` |
| `device.bluetooth` | Bluetooth | `bluetooth`, `radios` |
| `device.sensor` | — | `activity` |
| `device.printer` | Printing | — |
| `device.audio` | — | — |
| `desktop.capture` | Screen & System Audio Recording | `graphicsCapture` |
| `desktop.media.observe`, `desktop.media.control` | — | `globalMediaControl` |
| `clipboard.read`, `clipboard.write` | — | — |
| `ui.notify` | Notifications | — |
| `ui.window`, `time.cron`, `time.delay` | — | — |
| `ai.*` except `ai.bypass` | — | — |
| The App's own data: `data.kv.*`, `data.log.write`, `memory.read`, `memory.write`, and `ipc.publish` on `app:<app-id>` for [App events](wire/v1/app-events.md) | — | — |

A dash means that platform has no separate permission for it.

Every other capability is a system permission, for example `sys.*`,
`secret.*`, `agent.*`, `browser.*`, `proc.*`, `fs.exec`, `net.manage`,
`net.firewall`, `device.display`, `device.usb`, `ui.input`,
`ui.accessibility`, `ui.panel`, `data.log.read` (every App's and the
Agent's logs) and other Apps' data. Official Yu OS Apps
keep them. A third-party App may declare them, but like Windows UAC, an
administrator must approve each use, and Yu OS never saves that approval as
a lasting choice. In a terminal, `yu` asks for the password in place, like
sudo, and does not ask again in that terminal session for five minutes.

## App object references

Every binding exports the generated `ObjectRef` data contract and pure
format/parse helpers for canonical references such as
`app://notes/note?id=draft%2F1&revision=7`. A reference identifies an App,
object type, opaque object ID, and optional revision. Resolve it through the
declared App operation or MCP tool; the OS checks capabilities at dispatch.

See [the normative object-reference contract](wire/v1/object-references.md)
for byte limits, strict URI spelling, generated validation, shared golden
vectors, and the optional manifest `objects` declaration.

## App-reported file change plans

The generated `FileChangePlan` value and validators describe a bounded,
App-owned staged proposal, including its public diff and lifecycle state.
The App owns the plan's contents and lifecycle. A review fingerprint binds the
proposal data. Applying the plan still requires authorization and a current-file
check; another writer can change the file after that check.
See [the file change plan contract](wire/v1/file-change-plans.md) for exact
fields, nullable values, reference semantics, and validation limits.

## App effect declarations

An operation or MCP tool may declare up to 16 effects for a metadata-only
preview. Undeclared effects are unknown. Recovery text gives App-authored
guidance.

See [the effect declaration and preview contract](wire/v1/operation-effects.md)
for fields, label limits, target bindings, unresolved runtime arguments, and
generated binding compatibility.

## AI support

Use the six models configured in Yu OS Settings. Yu OS selects the model,
authorizes the call, and accounts for usage. The optional `system` field
carries your App's instructions to the chat model.

In `app.json`, add `"ai": {}` and the capability to `needs` for the
operation, MCP tool, or GUI entry.

In **Settings → AI Budget**, the owner sets one limit across all Apps and
model slots for the UTC month. `0` means unlimited. Yu OS tracks usage
and notifies the owner near or at the limit.

| SDK call | Model input | App capability | `output` |
|---|---|---|---|
| `chat` | Prompt, optional images and App functions | `ai.chat` | `text`, optional function calls |
| `chat_stream` | Prompt and optional images | `ai.chat` | Iterator / stream of `str` text chunks |
| `embed` | Text | `ai.embed` | `vector` (`list[float]`) |
| `tts` | Text | `ai.audio.tts` | `format`, `audio` bytes, optional `sample_rate_hz` |
| `stt` | WAV, MP3, or Ogg audio bytes | `ai.audio.stt` | `text` |
| `imagegen` | Prompt | `ai.image.generate` | `format`, `image` bytes |
| `decide` | `state` and [questions](../docs/decision-model.md) | `ai.decide` | `answers` |

Each `chat`, `embed`, `tts`, `stt`, `imagegen`, and `decide` call returns
`provider`, `model`, `units`, and `output`. `chat_stream` yields text chunks as
they arrive:

<details open>
<summary><b>Python</b></summary>

```python
from yuos_sdk.ai import chat, chat_stream, embed

reply = chat("Summarize this sentence.")
for chunk in chat_stream("Write a two-sentence summary."):
    print(chunk, end="", flush=True)
vector = embed("Text to index.").output.vector
```

</details>

<details>
<summary><b>Node.js / TypeScript</b></summary>

```ts
import { ai } from "@yuos/sdk";

const reply = await ai.chat("Summarize this sentence.");
for await (const chunk of ai.chatStream("Write a two-sentence summary.")) {
  process.stdout.write(chunk);
}
const vector = (await ai.embed("Text to index.")).output.vector;
```

</details>

<details>
<summary><b>Go</b></summary>

```go
reply, err := yuossdk.Chat("Summarize this sentence.", yuossdk.ChatOptions{})
_, err = yuossdk.ChatStream(ctx, "Write a two-sentence summary.", yuossdk.ChatOptions{}, func(chunk string) error {
	fmt.Print(chunk)
	return nil
})
emb, err := yuossdk.Embed(ctx, "Text to index.", yuossdk.ModelOptions{})
```

</details>

<details>
<summary><b>Rust</b></summary>

```rust
use yuos_sdk::ai;

let reply = ai::chat("Summarize this sentence.", ai::ChatOpts::new())?;
ai::chat_stream("Write a two-sentence summary.", ai::ChatOpts::new(), |chunk| {
    print!("{chunk}");
})?;
let vector = ai::embed("Text to index.", ai::ModelOpts::default())?.output.vector;
```

</details>

For local development outside a Yu OS session, set `YUOS_DEV_AI_URL` (for
example `http://localhost:11434/v1` for Ollama or any OpenAI-compatible `/v1`
endpoint), plus optional `YUOS_DEV_AI_MODEL` and `YUOS_DEV_AI_KEY`. Inside an
authenticated Yu OS session (`YUOS_SESSION`), the SDK ignores `YUOS_DEV_AI_*`
and routes through `yuosd`.

[Node](node/README.md#ai-and-catalog-tools), [Go](go/README.md#ai-support),
and [Rust](rust/README.md#ai-support) expose the same six calls.

### Handle AI errors

Use error types or [wire codes](wire/v1/error_codes.md), not message text.

| Code | Python and Node error | What the App can do |
|---|---|---|
| `PERMISSION_DENIED` | `AiDenied` | For an [optional GUI permission](../docs/app-development.md#model-permissions-in-settings), show your own message and offer a button to Settings → Applications. |
| `BUDGET_EXCEEDED` | `AiBudgetExceeded` | Tell the user the call exceeds the available budget. The owner can review the App limit in Settings → AI Budget. |
| `KERNEL_UNAVAILABLE` | `AiUnavailable` | Show that AI is unavailable. The owner can check the configured model in Settings. |
| `INVALID_ARGS` | `AiInvalidArgs` | Check the input and its limits before sending it again. |
| `SAFETY_VIOLATION` | `AiSafetyViolation` | Tell the user the request was refused. |

An unavailability error can also come from the local SDK transport, without
a server code. A timeout or lost reply can occur after the model has begun
work. Let the user decide whether to submit another call.

### Image input

Use a chat model that supports images. Pass the image bytes; Yu OS detects
PNG, JPEG, or WebP automatically. Send up to four images, 512 KiB combined.

<details open>
<summary><b>Python</b></summary>

```python
from yuos_sdk.ai import chat

def describe_image(png_bytes: bytes) -> str:
    return chat("Describe this image.", images=[png_bytes]).output.text
```

</details>

<details>
<summary><b>Node.js / TypeScript</b></summary>

```ts
import { ai } from "@yuos/sdk";

async function describeImage(pngBytes: Uint8Array): Promise<string> {
  return (await ai.chat("Describe this image.", { images: [pngBytes] })).output.text;
}
```

</details>

<details>
<summary><b>Go</b></summary>

```go
func describeImage(pngBytes []byte) (string, error) {
	reply, err := yuossdk.Chat("Describe this image.", yuossdk.ChatOptions{Images: [][]byte{pngBytes}})
	if err != nil {
		return "", err
	}
	return reply.Output.Text, nil
}
```

</details>

<details>
<summary><b>Rust</b></summary>

```rust
use yuos_sdk::ai;

fn describe_image(png_bytes: Vec<u8>) -> Result<String, ai::AiError> {
    let opts = ai::ChatOpts::new().images(vec![png_bytes]);
    Ok(ai::chat("Describe this image.", opts)?.output.text)
}
```

</details>

### App-owned tools

With a chat model that supports function calling, pass the schemas for your
App's functions. The model proposes a call; **your App validates and runs
it** using its own permissions, then sends back the result.

```python
from dataclasses import asdict
from yuos_sdk.ai import chat

request = {
    "prompt": "Look up the price of tea, then tell me the price.",
    "functions": [{
        "name": "lookup_price",
        "description": "Look up the App's price for an item.",
        "parameters": {
            "type": "object",
            "properties": {"item": {"type": "string", "enum": ["tea"]}},
            "required": ["item"],
        },
    }],
}

reply = chat(**request).output
if reply.tool_calls:
    results = []
    for call in reply.tool_calls:
        if call.name != "lookup_price" or call.input != {"item": "tea"}:
            results.append({"id": call.id, "content": "Unknown item", "is_error": True})
        else:
            results.append({"id": call.id, "content": "Tea costs $3."})
    turn = {"text": reply.text, "calls": [asdict(c) for c in reply.tool_calls], "results": results}
    if reply.provider_state:
        turn["provider_state"] = reply.provider_state
    reply = chat(**request, tool_turns=[turn]).output
print(reply.text)
```

Keep prior `tool_turns` in order if the model asks for another call.
Pass one result per call ID and echo optional `provider_state` unchanged.
Each request counts toward the App AI budget.

## CLI commands

| Command | Description |
|---|---|
| `yu app create <id> [--kind cli\|desktop\|both]` | Scaffold a new App directory (`app.json` + entrypoint). |
| `yu app dev <dir> <command> [args…]` | Run a local App directory during development (`--gui` starts the desktop entry). |
| `yu app lint [<id>]` | Validate `app.json`, declared entrypoints, and SDK usage rules. |
| `yu app pack <dir> [--out <file.deb>] [--arch all\|amd64\|arm64]` | Package an App directory into a compliant `.deb` archive. |
| `yu app submit <file.deb> [--repo <owner/repo>]` | Submit a packaged `.deb` to the Store review queue (`submissions/incoming/`). |

## SDK source and code generation

The [wire schemas](wire/v1/README.md) and [decoder contract](wire/v1/contract.json)
define generated types, validators, and MCP error constants. The language
directories own their transport and idiomatic API wrappers. See [MODULE.md](MODULE.md)
for the source and test layout.

Run codegen with:

```sh
cd yuos-sdk
python3 wire/codegen.py            # writes generated.* into each language tree
python3 wire/codegen.py --check    # verifies all generated outputs are current
```

Use `--sdk-only` with either command when regenerating a standalone SDK or
when another integrator owns the core MCP constants. It still generates all
four language bindings and the Rust SDK's MCP constants.

## Versioning

- **Wire protocol:** subprocess calls request `--wire=1` and check the version
  and shape of each response.
- **SDK packages:** Python, Node, Rust, and Go share the `yuos-sdk-v<version>`
  release and target the same generated CLI/MCP contract. Python derives its
  package metadata from the source `yuos_sdk.__version__`.
- **App runtime ABI:** the separate [App development platform](../docs/app-platform.md)
  records its ABI, export inventory, and version.

## Compatibility

Use SDK `0.1.0` with `yu` and the Yu OS runtime built from the same source
revision as a baseline. Live App calls need the installed runtime and running
broker; GUI and App data clients also need their OS services. Use pure helpers
and business logic tests off-system. `yu --wire=1 --version` checks the CLI
and envelope. Other OS builds must provide wire v1 and the operations the App uses.

Wire v1 supports additive changes within extensible shapes and preserves
existing behavior. Breaking changes need a versioned contract.

Generated decoders follow JSON Schema's mathematical-integer semantics:
finite `1`, `1.0`, and `1e0` values are equivalent integers. Bounds are
checked after type validation using lossless decimal-rational lexemes, and all
SDK adapters preserve accepted values. Node exposes u64-domain values as
`bigint` only when they cannot be represented safely as `number`.

Unrestricted JSON payloads use a stable lossless public model:

- Rust uses `serde_json::Value` and `serde_json::to_string`.
- Python uses ordinary JSON values plus `Decimal`/`WireDecimal`; use
  `generated.encode_wire_json`.
- Node uses ordinary JSON values plus `bigint` and `WireDecimal`; use
  `generated.stringifyWireJson`.
- Go uses `any` plus `json.Number` and the standard `encoding/json` encoder.

Fixed AI catalog tool arguments and model-proposed inputs can represent any
JSON value, including explicit `null`, scalars, and arrays. Each tool's own
schema still constrains what it accepts. Using the SDK serializer preserves
number precision. App MCP tools instead take an object of arguments declared
in the signed manifest.
Compact exponents that are unsafe to materialize remain `WireDecimal`
wrappers; serializers never expand them in memory. Node serialization rejects
non-finite and unsafe integer-valued native `number` values—use `bigint` or
`WireDecimal` instead.

## Releases

SDK packages are distributed through GitHub Releases.
[Publish SDK Release](../.github/workflows/publish-sdk-release.yml)
is dispatched manually on `main` and requires successful Root validation
for the exact source commit. It checks version metadata and generated
bindings, tests all four languages, and produces:

- `yuos-sdk-v<version>` for the GitHub Release and Python/Node/Rust consumers;
- `yuos-sdk/go/v<version>` for Go module resolution;
- Python wheel and source distribution;
- Node package tarball;
- Rust `.crate` archive;
- Go and complete SDK source archives;
- portable Linux signing tools for amd64 and arm64, with notices;
- `SHA256SUMS`.

For distribution, pin a release artifact or tag. Verify downloaded archives
and signing tools against that release's `SHA256SUMS`. See the
[App development platform](../docs/app-platform.md) for development artifacts.

## Language support

| Binding | Development toolchain |
|---|---|
| [Rust](rust/README.md) | Stable Cargo/Rust used by CI |
| [Python](python/README.md) | Python 3.10+ |
| [Node](node/README.md) | Node.js 18+ |
| [Go](go/README.md) | Go 1.21+ |

All four bindings support model and catalog calls, manifest-bound MCP with
progress and cancellation, GUI bootstrap, object references, manifest
helpers, events, and channels. MCP tool inputs follow the
[manifest argument model](../docs/app-development.md#3-the-minimum-viable-app).

Python, Node, and Go support standalone AI usage queries. Rust provides
[App data services](wire/v1/applet-services.md) through the installed OS provider.
