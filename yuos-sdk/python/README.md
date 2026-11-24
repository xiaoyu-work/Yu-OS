# yuos-sdk (Python)

The official Python SDK for Yu OS. Import it as `yuos_sdk`.

## Install

```sh
python3 -m pip install \
  https://github.com/xiaoyu-work/Yu-OS/releases/download/yuos-sdk-v0.1.0.dev2026112401/yuos_sdk-0.1.0.dev2026112401-py3-none-any.whl
```

Requires Python 3.10 or later. The Yu OS Agent package already supplies
`yuos_sdk` at `/usr/lib/yuos/python/`. Live calls require the
[Yu OS runtime](../quickstart.html#install).

## Use

From an authenticated App with the [matching AI capability and consent](../../docs/app-development.md#7-ai-features):

```python
from yuos_sdk.ai import chat
from yuos_sdk.app import command

@command("summarize")
def summarize(args):
    result = chat(args[0])
    return {"summary": result.output.text, "units": result.units}
```

## What's in it

| Module                  | Purpose                                                                |
|-------------------------|------------------------------------------------------------------------|
| `yuos_sdk.app`       | Mark the [`@command` and `@desktop`](../api.html#python-mod-app) functions Yu OS starts. |
| `yuos_sdk.ai`        | `chat`, `embed`, `tts`, `stt`, `imagegen`, and `decide`. |
| `yuos_sdk.tools`     | `yu ai tool <name>` — explicitly call a catalog operation.           |
| `yuos_sdk.gui`       | Desktop GUI bootstrap and kernel-provided launch context.              |
| `yuos_sdk.mcp`       | Manifest-bound App MCP server, call context, progress, and cancellation. |
| `yuos_sdk.kernel`    | Explicit installed-CLI stdin transport with shared wire errors and cancellation. |
| `yuos_sdk.objects`   | Format and parse canonical App object identifiers. |
| `yuos_sdk.manifest`  | Validate or preview manifest path requests. |
| `yuos_sdk.events`, `yuos_sdk.channels` | Report App events and control authorized App-owned channels. |
| `yuos_sdk.yuos_session` | Read / observe `YUOS_SESSION` from inside an app.                 |
| `yuos_sdk.generated` | TypedDicts generated from `wire/v1/*.schema.json`.                     |

The OS checks capabilities when public SDK operations run.

## Expose App tools

See the [two-file MCP example](../../docs/app-development.md#quickstart-an-mcp-only-app)
for a complete manifest, handler, and install/run commands.

Declare MCP tools once in `app.json`, then bind only their handlers:

```python
from yuos_sdk.mcp import App, current_context

app = App.from_manifest()

@app.tool("mail.search")
def search(query: str) -> dict:
    context = current_context()
    context.raise_if_cancelled()
    context.report_progress(1, total=2, message="Searching")
    return {"matches": find_messages(query)}

app.serve()
```

The App Host sets `YUOS_APP_MANIFEST` to its verified manifest snapshot.
`App.from_manifest()` falls back to `./app.json` in direct development, which
does not authenticate an App or caller. MCP-first calls require the Gateway's
versioned call context; caller identity never comes from tool arguments.
The runtime validates arguments and applies manifest defaults before calling
the handler. `report_progress()` is a no-op when the caller did not request
progress.

## Controlled primitive transport

`kernel.call_json_with_stdin_binary(binary, args, data, *,
deadline_unix_ms, check_cancelled=None)` sends bounded bytes to an explicit
absolute CLI path (installed Yu OS uses `/usr/local/bin/yu`).
`args` is the complete primitive argv; the SDK adds `--wire=1`. Pass the
authenticated MCP context's `raise_if_cancelled` callback and no later than
its deadline. The inherited broker session supplies authority, never stdin,
caller labels or environment changes.

It returns a decoded object or raises `KernelDenied` with the original
structured `.payload`, or `KernelUnavailable` for transport/decode failures.
Cancellation and deadlines kill/reap the CLI, but cannot undo an already
accepted OS mutation. Do not automatically retry an indeterminate mutation.
This helper is not an App dispatcher or a capability grant.

## App object references

```python
from yuos_sdk import objects
from yuos_sdk.generated import ObjectRef

reference: ObjectRef = {
    "app_id": "notes",
    "object_type": "note",
    "object_id": "draft/1",
}
uri = objects.format_reference(reference)
assert uri == "app://notes/note?id=draft%2F1"
assert objects.parse_reference(uri) == reference
```

`objects.ObjectRefError` is a `ValueError`. These pure helpers enforce the
generated structural contract plus component/UTF-8 bounds and canonical
spelling. Omit an absent `revision`; do not supply `None` or an empty string.
They never trim opaque IDs, discover Apps, access files, or invoke an
operation. A reference does not assert existence, freshness, or permission.

See [the shared contract](../wire/v1/object-references.md) for byte limits and
the optional App manifest `objects` resolver declaration.

## AI support

`chat` takes optional image bytes and App functions, and `stt` takes audio
bytes. `tts` and `imagegen` return `audio` or `image` bytes. All six model
calls return `provider`, `model`, `units`, and `output`. See the
[model capabilities](../api.html#python-mod-ai) and
[typed errors](../reference.html#handle-ai-errors).

## Configuration

| Env var          | Effect                                                                |
|------------------|-----------------------------------------------------------------------|
| `YUOS_KERNEL_BIN`   | Path to the `yu` binary. Defaults to looking up `yu` in `$PATH`.    |
| `YUOS_APP_ID`     | App id, used by `ai.chat` / `tools.call`.                             |
| `YUOS_APP_MANIFEST` | Verified manifest snapshot used by `mcp.App.from_manifest()`.       |

## Wire protocol

This package implements wire protocol v1. See
[`../wire/v1/README.md`](../wire/v1/README.md) for the full spec.
Regenerate typed structs with:

```sh
python3 ../wire/codegen.py
```

## History

This package was previously known as `_lib` and lived under
`apps/_lib`. It has been promoted to the public, multi-language SDK
at `yuos-sdk/python` and renamed. There is no compatibility shim
— yuos is pre-1.0 and breaking changes are allowed.
