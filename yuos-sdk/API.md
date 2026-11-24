# Yu OS SDK — Complete API & Manifest Reference

Related guides: [Quickstart](QUICKSTART.md) · [Overview & Permissions](REFERENCE.md) · [Integrating Existing Linux Apps](../docs/linux-app-integration.md)

**Contents**
1. [`app.json` Manifest Reference](#1-appjson-manifest-reference)
   - [Top-level fields](#top-level-fields)
   - [Operation & MCP tool fields (`operations.<name>`)](#operation--mcp-tool-fields)
   - [Argument fields (`args[]`) and `kind` types](#argument-fields-args-and-kind-types)
   - [Permission declarations (`needs[]`)](#permission-declarations-needs)
2. [App Entry & System Helpers (`yuos_sdk.app`)](#2-app-entry--system-helpers-yuos_sdkapp)
   - [`command` / `@command`](#command--command)
   - [`desktop` / `@desktop`](#desktop--desktop)
   - [`run`](#run)
   - [`data_dir`](#data_dir)
   - [`notify`](#notify)
   - [`clipboard_read` & `clipboard_write`](#clipboard_read--clipboard_write)
   - [`secret`](#secret)
   - [`check_permission` & `require_permission`](#check_permission--require_permission)
3. [AI Models (`yuos_sdk.ai`)](#3-ai-models-yuos_sdkai)
   - [`chat`](#chat)
   - [`chat_stream`](#chat_stream)
   - [`embed`](#embed)
   - [`tts`](#tts)
   - [`stt`](#stt)
   - [`imagegen`](#imagegen)
   - [`decide`](#decide)
   - [`budget`](#budget)
4. [Desktop Dialogs & Window Context (`yuos_sdk.gui`)](#4-desktop-dialogs--window-context-yuos_sdkgui)
   - [`open_file`](#open_file)
   - [`save_file`](#save_file)
   - [`select_folder`](#select_folder)
   - [`context` & `open_agent_overlay`](#context--open_agent_overlay)
   - [`is_gui_launch`](#is_gui_launch)
5. [System Agent MCP Service (`yuos_sdk.mcp`)](#5-system-agent-mcp-service-yuos_sdkmcp)
6. [Brokered File Snapshots (`yuos_sdk.files`)](#6-brokered-file-snapshots-yuos_sdkfiles)
7. [Events & Object References (`events`, `objects`)](#7-events--object-references-events-objects)

---

## 1. `app.json` Manifest Reference

Every Yu OS App directory contains an `app.json` manifest at its root.

```json
{
  "id": "doc-helper",
  "version": "0.1.0",
  "name": {"en": "Doc Helper", "zh-CN": "文档助手"},
  "runtime": "python",
  "ai": {},
  "operations": {
    "summarize": {
      "label": {"en": "Summarize a document"},
      "args": [
        {"name": "file", "kind": "path", "required": true},
        {"name": "max_words", "kind": "integer", "default": 100},
        {"name": "verbose", "kind": "bool"}
      ],
      "needs": ["ai.chat"]
    }
  }
}
```

### Top-level fields

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `id` | `string` | **Yes** | — | Unique App ID matching `[a-z][a-z0-9_-]*` (e.g. `"my-app"`). |
| `version` | `string` | **Yes** | — | SemVer version string (e.g. `"0.1.0"`). |
| `name` | `{"en": string, ...}` | **Yes** | — | Localized display name (`"en"` is required; `"zh-CN"` etc. optional). |
| `summary` | `{"en": string, ...}` | No | — | Short one-line description shown in the Store and CLI. |
| `runtime` | `"python" \| "node" \| "binary" \| "shell"` | No | `"python"` | How Yu OS launches the App entry point. |
| `entry` | `string` | No | `"main.py"` / `"index.js"` / `"bin/<id>"` | Entry file relative to the App directory. |
| `ai` | `object` (`{}`) | When using `ai.*` | — | Enables system AI access for this App. |
| `operations` | `Record<string, Operation>` | For CLI / Desktop | `{}` | Commands shared by CLI (`yu app <id> <cmd>`) and Desktop (`--gui`). |
| `desktop` | `Desktop` | No | Auto-derived from `operations` | Optional overrides for `.desktop` metadata (`icon`, `categories`, `mime_types`, `single_instance`). |
| `mcp` | `McpService` | For Agent tools | — | Tools exposed to the built-in Yu OS System Agent (`schema_version: 2`). |

### Operation & MCP tool fields

Each entry under `operations.<name>` (or `mcp.tools[]`) accepts:

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `label` / `summary` | `{"en": string, ...}` | **Yes** | — | Human/Agent description (`operations` uses `label`; `mcp.tools` uses `summary`). |
| `args` | `Arg[]` | No | `[]` | Ordered list of input parameters. |
| `needs` | `(string \| Need)[]` | No | `[]` | Permissions required by this operation. |
| `stdin` | ` boolean` | No | `false` | Allow piping raw stdin via `yu app <id> <op> --stdin` (up to 16 MiB). |

### Argument fields (`args[]`) and `kind` types

Each item in `args` defines one input parameter:

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | `string` | **Yes** | — | Parameter name (`[a-z][a-z0-9_-]*`). |
| `kind` | `string` | **Yes** | — | Parameter data type (see table below). |
| `required` | `boolean` | No | `false` | Reject the call before launch if omitted. |
| `default` | `string \| number \| boolean` | No | — | Literal default value when omitted (cannot be combined with `required: true`). |
| `binding` | `"positional" \| "flag"` | No | `"flag"` for `bool`, `"positional"` for others | How the argument is passed on the CLI (`--name value` vs positional). |
| `choices` | `Array<string \| number \| boolean>` | No | `[]` | Restrict input to a fixed set of allowed values (enum). |
| `repeatable` | `boolean` | No | `false` | Accept multiple values (`list` / array); must be the last positional arg if positional. |
| `label` | `{"en": string, ...}` | No | — | Optional parameter description shown in `--schema` and help. |

#### Supported `kind` values

| `kind` | Value type | CLI behavior & OS integration |
|---|---|---|
| `"text"` | `string` | Free-form UTF-8 string. |
| `"path"` | `string` (file/directory path) | Expands `./` and `~/` to a canonical path and **automatically grants read access (`fs.read`)** to the user-supplied file in the sandbox. |
| `"integer"` | `int` | Whole number (`42`). Non-integers are rejected before launch. |
| `"number"` | `float` | Integer or decimal number (`3.14`). |
| `"bool"` | `boolean` | Boolean switch. Defaults to `binding: "flag"` (`--verbose` or `--verbose=false`). |
| `"host"` | `string` | Network `host` or `host:port` (can be bound to `net.dial` scopes). |
| `"name"` | `string` | Identifier token (can be bound to named scopes). |

### Permission declarations (`needs[]`)

1. **String shorthand (AI capabilities)**:
   ```json
   "needs": ["ai.chat", "ai.embed", "ai.audio.tts", "ai.audio.stt", "ai.image.generate", "ai.decide"]
   ```
2. **Scoped object (when requesting non-default file Write, Network, or Device access)**:
   ```json
   "needs": [
     {
       "verb": "fs.write",
       "scope": {"kind": "owner-path", "path": "Documents/**"},
       "why": {"en": "Save exported reports in your Documents folder"}
     }
   ]
   ```
   > **Note:** Reading a file passed by the user via a `"kind": "path"` argument or selected via `gui.open_file()` is authorized automatically by user intent and does **not** require a `needs` entry.

---

## 2. App Entry & System Helpers (`yuos_sdk.app`)

### `command` / `@command`

Register a command-line handler for `yu app <id> <name> [args...]`.

<details open>
<summary><b>Python</b></summary>

```python
from yuos_sdk.app import command

@command                   # Uses function name "summarize"
def summarize(args: list[str]) -> dict:
    return {"summary": args[0]}

@command("custom-name")    # Explicit command name
def my_handler(args: list[str]) -> dict:
    return {"ok": True}
```

</details>

<details>
<summary><b>Node.js / TypeScript</b></summary>

```ts
import { app } from "@yuos/sdk";

app.command(function summarize(args: string[]) {
  return { summary: args[0] };
});
```

</details>

<details>
<summary><b>Go</b></summary>

```go
yuossdk.HandleCommand("summarize", func(args []string) (any, error) {
	return map[string]any{"summary": args[0]}, nil
})
```

</details>

<details>
<summary><b>Rust</b></summary>

```rust
use yuos_sdk::app::App;

App::new().command("summarize", |args: Vec<String>| {
    Ok(serde_json::json!({ "summary": args[0] }))
});
```

</details>

| Parameter | Type | Required | Description |
|---|---|---|---|
| `name` | `string` | Optional in Python/Node (defaults to function name) | Operation name matching `operations.<name>` in `app.json`. |
| `handler` | `(args: string[]) => any` | **Yes** | Receives validated CLI arguments as strings; return value is serialized to stdout as JSON. |

---

### `desktop` / `@desktop`

Register the zero-argument entry point called when the App is launched from the desktop launcher or via `yu app <id> --gui`.

<details open>
<summary><b>Python</b></summary>

```python
from yuos_sdk.app import desktop

@desktop
def main() -> None:
    # Start your Qt / GTK / Tkinter window loop
    ...
```

</details>

<details>
<summary><b>Node.js / TypeScript</b></summary>

```ts
app.desktop(async () => {
  // Open your window
});
```

</details>

<details>
<summary><b>Go</b></summary>

```go
yuossdk.HandleDesktop(func() error {
	return nil
})
```

</details>

<details>
<summary><b>Rust</b></summary>

```rust
App::new().desktop(|| {
    Ok(())
});
```

</details>

> **Automatic fallback:** If an App only registers `@command` and a user runs `--gui` (or vice versa), the SDK automatically outputs `{"available": false, "surface": "desktop"|"cli", "message": "..."}` and exits cleanly.

---

### `run`

Dispatch the current process invocation to the matching `@command` or `@desktop` function.
- **Python:** Optional (`yuos-sdk` auto-dispatches when `main.py` finishes loading if `run()` was not called).
- **Node.js:** `await app.run()`
- **Go:** `err := yuossdk.RunApp()`
- **Rust:** `app.run()?`

---

### `data_dir`

Return the App's isolated private storage directory (`$YUOS_DATA_DIR` in production, or `~/.local/share/yuos-dev/apps/<id>` during local development), creating it if it does not exist. Requires no permissions in `app.json`.

| Language | Signature | Returns |
|---|---|---|
| **Python** | `app.data_dir()` | `pathlib.Path` |
| **Node.js** | `app.dataDir()` | `string` |
| **Go** | `yuossdk.DataDir()` | `(string, error)` |
| **Rust** | `app::data_dir()` | `std::io::Result<PathBuf>` |

---

### `notify`

Post a desktop/system notification (`ui.notify`).

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `summary` | `string` | **Yes** | — | Notification title (non-empty). |
| `body` | `string` | No | `""` | Notification body text. |

| Language | Signature |
|---|---|
| **Python** | `app.notify(summary: str, body: str = "") -> dict` |
| **Node.js** | `app.notify(summary: string, body?: string): Record<string, unknown>` |
| **Go** | `yuossdk.Notify(summary, body string) error` |
| **Rust** | `app::notify(summary: &str, body: &str) -> Result<Value, BridgeError>` |

---

### `clipboard_read` & `clipboard_write`

Read or write plain text on the system clipboard (`clipboard.read` / `clipboard.write`). Outside `YUOS_SESSION`, reads/writes an in-memory dev clipboard (or `$YUOS_DEV_CLIPBOARD`).

| Language | Read signature | Write signature |
|---|---|---|
| **Python** | `app.clipboard_read() -> str` | `app.clipboard_write(text: str) -> None` |
| **Node.js** | `app.clipboardRead(): string` | `app.clipboardWrite(text: string): void` |
| **Go** | `yuossdk.ClipboardRead() (string, error)` | `yuossdk.ClipboardWrite(text string) error` |
| **Rust** | `app::clipboard_read() -> Result<String, BridgeError>` | `app::clipboard_write(text: &str) -> Result<(), BridgeError>` |

---

### `secret`

Read an encrypted credential or API token from the Yu OS Credential Store (`secret.use`, equivalent to macOS Keychain / Windows Credential Manager). Outside `YUOS_SESSION`, reads `$YUOS_DEV_SECRET_<UPPER_NAME>` or `$YUOS_DEV_SECRET`.

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | `string` | **Yes** | — | Credential key name (e.g. `"GOOGLE_ACCESS_TOKEN"`). |
| `namespace` | `string` | No | `"default"` | Credential namespace. |

| Language | Signature |
|---|---|
| **Python** | `app.secret(name: str, namespace: str = "default") -> str` |
| **Node.js** | `app.secret(name: string, namespace = "default"): string` |
| **Go** | `yuossdk.Secret(name, namespace string) (string, error)` |
| **Rust** | `app::secret(name: &str, namespace: &str) -> Result<String, BridgeError>` |

---

### `check_permission` & `require_permission`

Check at runtime whether the current session holds `verb` for a dynamically computed scope (for example, checking `net.dial` on a redirect target host). Outside `YUOS_SESSION`, always returns `allow`.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `verb` | `string` | **Yes** | Capability verb (e.g. `"net.dial"`, `"fs.read"`). |
| `scope` | `path \| host \| name \| self_ref \| wild` | At most one | Dynamic scope to check against the active session. |

| Language | Check signature (returns decision dict) | Require signature (raises/errors on deny) |
|---|---|---|
| **Python** | `app.check_permission(verb, *, path=None, host=None, name=None, self_ref=None, wild=False) -> dict` | `app.require_permission(verb, *, path=None, host=None, name=None, self_ref=None, wild=False) -> None` |
| **Node.js** | `app.checkPermission(verb, { path?, host?, name?, selfRef?, wild? })` | `app.requirePermission(verb, { path?, host?, name?, selfRef?, wild? })` |
| **Go** | `yuossdk.CheckPermission(verb, PermissionScope{...})` | `yuossdk.RequirePermission(verb, PermissionScope{...})` |
| **Rust** | `app::check_permission(verb, PermissionScope)` | `app::require_permission(verb, PermissionScope)` |

---

## 3. AI Models (`yuos_sdk.ai`)

All AI functions automatically route through the owner's configured model in Yu OS Settings (or `$YUOS_DEV_AI_URL` during local development). Every non-streaming call returns a `ModelResult`:

| `ModelResult` field | Type | Description |
|---|---|---|
| `provider` | `string` | Active provider ID (e.g. `"openai"`, `"anthropic"`, `"local"`). |
| `model` | `string` | Active model ID. |
| `units` | `int` | Budget units consumed by this call. |
| `output` | Typed per call (see below) | Model output payload. |

### `chat`

Run a text or multimodal chat completion (`"needs": ["ai.chat"]`).

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `prompt` | `string` | **Yes** | — | User prompt text (non-empty). |
| `system` | `string` | No | `None` | System instruction for the model. |
| `images` | `list[bytes]` | No | `None` | Up to 4 raw image byte buffers (PNG, JPEG, or WebP auto-detected; max 512 KiB total). |
| `max_units` | `int` | No | `None` | Optional per-call budget ceiling. |
| `functions` | `list[dict]` | No | `None` | Optional JSON Schema function declarations for tool calling. |
| `tool_turns` | `list[dict]` | No | `None` | Prior tool call + result turns when continuing a function-calling loop. |

**Returns (`reply.output`):**
- `output.text` (`string`): Assistant reply text.
- `output.tool_calls` (`list[ToolCall]`): Proposed function calls (`id`, `name`, `input`).

### `chat_stream`

Stream text chunks from the chat model as they are generated (`"needs": ["ai.chat"]`). Accepts the same `prompt`, `system`, `images`, and `max_units` parameters as `chat`.

- **Python:** `for chunk in ai.chat_stream(prompt, system=...):` (yields `str`)
- **Node.js:** `for await (const chunk of ai.chatStream(prompt, { system }))` (yields `string`)
- **Go:** `yuossdk.ChatStream(ctx, prompt, opts, func(chunk string) error { ... })`
- **Rust:** `ai::chat_stream(prompt, opts, |chunk: &str| { ... })`

### `embed`

Compute a semantic embedding vector for text (`"needs": ["ai.embed"]`).

| Parameter | Type | Required | Description |
|---|---|---|---|
| `text` | `string` | **Yes** | Input text to embed. |
| `max_units` | `int` | No | Optional per-call unit ceiling. |

**Returns (`reply.output`):** `output.vector` (`list[float]` / `number[]` / `[]float64` / `Vec<f64>`).

### `tts`

Synthesize speech audio from text (`"needs": ["ai.audio.tts"]`).

| Parameter | Type | Required | Description |
|---|---|---|---|
| `text` | `string` | **Yes** | Text to speak. |
| `voice` | `string` | No | Optional voice name. |
| `max_units` | `int` | No | Optional per-call unit ceiling. |

**Returns (`reply.output`):**
- `output.audio` (`bytes` / `Buffer` / `[]byte` / `Vec<u8>`): Raw audio bytes.
- `output.format` (`string`): Audio format (`"wav"`, `"mp3"`, `"ogg"`, `"aac"`, `"flac"`, or `"pcm16"`).
- `output.sample_rate_hz` (`int | None`): Sample rate in Hz when reported.

### `stt`

Transcribe speech audio bytes to text (`"needs": ["ai.audio.stt"]`).

| Parameter | Type | Required | Description |
|---|---|---|---|
| `audio` | `bytes` / `Uint8Array` / `[]byte` | **Yes** | Raw WAV, MP3, or Ogg audio bytes (format auto-detected from magic bytes). |
| `language` | `string` | No | Optional BCP-47 language hint (e.g. `"zh"`, `"en"`). |
| `max_units` | `int` | No | Optional per-call unit ceiling. |

**Returns (`reply.output`):** `output.text` (`string`).

### `imagegen`

Generate an image from a text prompt (`"needs": ["ai.image.generate"]`).

| Parameter | Type | Required | Description |
|---|---|---|---|
| `prompt` | `string` | **Yes** | Image description. |
| `size` | `"square" \| "landscape" \| "portrait"` | No | Aspect ratio preset. |
| `max_units` | `int` | No | Optional per-call unit ceiling. |

**Returns (`reply.output`):**
- `output.image` (`bytes` / `Buffer` / `[]byte` / `Vec<u8>`): Raw image bytes.
- `output.format` (`string`): `"png"`, `"jpeg"`, or `"webp"`.

### `decide`

Evaluate structured state against a list of decision questions (`"needs": ["ai.decide"]`).

| Parameter | Type | Required | Description |
|---|---|---|---|
| `state` | `dict` / `object` | **Yes** | JSON object describing current state. |
| `questions` | `list[dict]` | **Yes** | Questions (`id`, `question`, `kind`: `"boolean" \| "choice" \| "number"`, optional `choices`). |

**Returns (`reply.output`):** `output.answers` (`list[DecisionAnswer]` with `id`, `value`, `confidence`).

### `budget`

Query the App's current monthly AI usage (`ai.budget()`). Returns `period`, `units_used`, and `units_cap`.

---

## 4. Desktop Dialogs & Window Context (`yuos_sdk.gui`)

### `open_file`

Open the native system file picker. Selecting a file automatically grants read access to that file for the running window (no `needs` entry required in `app.json`).

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `title` | `string` | No | `"Open file"` | Dialog window title. |

**Returns:** Selected file path (`str` / `string`), or `None` / `null` / `""` if the user cancelled.
*(Local dev override: `$YUOS_DEV_OPEN_FILE`)*

### `save_file`

Open the native system Save dialog (`"needs": [{"verb": "ui.prompt", "scope": {"kind": "wild"}, "why": {"en": "..."}}]`).

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `default_name` | `string` | **Yes** | — | Suggested filename (e.g. `"report.txt"`). |
| `title` | `string` | No | `"Save file"` | Dialog window title. |

**Returns:** Chosen destination path, or `None` / `null` / `""` if cancelled.
*(Local dev override: `$YUOS_DEV_SAVE_FILE`)*

### `select_folder`

Open the native system folder picker (`ui.prompt`).

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `title` | `string` | No | `"Select folder"` | Dialog window title. |

**Returns:** Chosen directory path, or `None` / `null` / `""` if cancelled.
*(Local dev override: `$YUOS_DEV_SELECT_FOLDER`)*

### `context` & `open_agent_overlay`

Read the desktop launch context (`app_id`, `files` opened via file association) and trigger the system-wide "Ask Yu OS" Agent overlay with optional context.

| Language | Context call | Overlay call |
|---|---|---|
| **Python** | `ctx = gui.context()` (`ctx.app_id`, `ctx.files`) | `ctx.open_agent_overlay(hint="...")` |
| **Node.js** | `const ctx = gui.context()` (`ctx.appId`, `ctx.files`) | `await gui.openAgentOverlay("...")` |
| **Go** | `ctx, err := yuossdk.Context(nil)` (`ctx.AppID`, `ctx.Files`) | `ctx.OpenAgentOverlay("...")` |
| **Rust** | `let ctx = gui::Context::from_env()?` (`ctx.app_id`, `ctx.files`) | `ctx.open_agent_overlay(Some("..."))?` |

### `is_gui_launch`

Returns `true` when the current process was started as a desktop GUI (`YUOS_APP_GUI=1`).

---

## 5. System Agent MCP Service (`yuos_sdk.mcp`)

Expose typed tools declared under `mcp.tools` in `app.json` so the built-in Yu OS System Agent can call your App.

| Method / Function | Python | Node.js | Go | Rust | Description |
|---|---|---|---|---|---|
| Load manifest | `app = App.from_manifest()` | `const app = mcp.App.fromManifest()` | `app, err := LoadMCPApp()` | `let mut app = mcp::App::from_environment()?` | Parse `app.json` and initialize the MCP server. |
| Bind tool | `@app.tool("id.tool")` | `app.tool("id.tool", fn)` | `app.Bind("id.tool", fn)` | `app.bind(MyTool)` | Bind a handler function to a declared tool name. |
| Start loop | `app.serve()` | `await app.serve()` | `app.ServeStdio()` | `app.serve_stdio().await` | Serve JSON-RPC requests over stdio. |
| Call context | `mcp.current_context()` | `ctx` parameter | `ctx` parameter | `ctx: &CallContext` | Inspect `caller`, `session_id`, `deadline_unix_ms`, `raise_if_cancelled()`, `report_progress()`. |

---

## 6. Brokered File Snapshots (`yuos_sdk.files`)

Read and atomically replace a single user file up to 32 MiB inside a `brokered-file` MCP tool without mounting the host directory.

| Function | Parameters | Returns | Description |
|---|---|---|---|
| `read_file` / `readFile` / `ReadFile` | `path: str` (+ deadline/cancellation options) | `FileSnapshot(content: bytes, revision: str)` | Read complete verified file bytes and an opaque revision token. |
| `replace_file` / `replaceFile` / `ReplaceFile` | `path: str, revision: str, content: bytes` | `None` / `void` / `error` | Atomically replace the file if its current state still matches `revision`. |

---

## 7. Events & Object References (`events`, `objects`)

- **`events.publish(event_type, idempotency_key, *, entity_id=None, payload=None)`**:
  Publish a structured domain event (e.g. `"calendar.event.created"`) under `ipc.publish` (`app:<id>`) so Activity triggers and automations can react.
- **`objects.format_reference({"app_id": ..., "object_type": ..., "object_id": ..., "revision": ...})`** & **`objects.parse_reference(uri)`**:
  Format and parse canonical `app://<app_id>/<object_type>?id=<object_id>` URIs.
