# yuos-sdk (Go)

Official **Go** SDK for [Yu OS](https://github.com/xiaoyu-work/Yu-OS).

Use this module to call models and catalog tools, expose MCP tools, and
bootstrap a desktop GUI. Outbound calls use `yu` and wire protocol v1.
Yu OS handles capability checks, input policy, budgets, and auditing.

## Install

From your App's Go module (Go 1.21 or later):

```sh
go get github.com/xiaoyu-work/Yu-OS/yuos-sdk/go@v0.1.0
```

```go
import yuossdk "github.com/xiaoyu-work/Yu-OS/yuos-sdk/go"
```

Live calls require the [Yu OS runtime](../quickstart.html#install).

## Surface

| Area          | Functions                                                          | Equivalent CLI                    |
|---------------|--------------------------------------------------------------------|-----------------------------------|
| App           | `HandleCommand`, `HandleDesktop`, `RunApp`                         | `yu app <id> <command>` / `--gui` |
| AI            | `Chat`, `ChatContext`, `Infer`; `Budget` and `BudgetContext` (usage only) | `yu ai chat` / `yu ai infer` |
| Tools         | `CallTool`, `Catalog`, their `Context` variants, and `ForChat`     | `yu ai tool <name> --app <id>`   |
| MCP service   | `LoadMCPApp`, `(*MCPApp).Bind`, `Serve`, `ServeStdio`               | App Host private stdio transport  |
| GUI           | `Context`, `(*GuiContext).OpenAgentOverlay`                        | launched via `yu app <id> --gui` |
| Objects       | `FormatReference`, `ParseReference`                               | Pure identifiers; no transport |
| Manifest, events, and channels | Manifest path helpers, `PublishEvent`, and channel controls | —                   |
| Transport     | `YuBinary`, error types (advanced)                                | —                                 |
| (generated)   | Typed structs from `wire/v1/*.schema.json` (`generated.go`)        | —                                 |

## Usage

```go
package main

import (
	"context"

	yuossdk "github.com/xiaoyu-work/Yu-OS/yuos-sdk/go"
)

func summarize(ctx context.Context, body string) (string, error) {
	res, err := yuossdk.ChatContext(ctx, body, yuossdk.ChatOptions{})
	if err != nil {
		return "", err
	}
	return res.Output.Text, nil
}
```

## Command line and desktop

Set `"runtime": "binary"` and register the function for each version the App
declares in `app.json`:

```go
package main

import (
	"log"

	yuossdk "github.com/xiaoyu-work/Yu-OS/yuos-sdk/go"
)

func main() {
	yuossdk.HandleCommand("summarize", func(args []string) (any, error) {
		reply, err := yuossdk.Chat(args[0], yuossdk.ChatOptions{})
		if err != nil {
			return nil, err
		}
		return map[string]string{"summary": reply.Output.Text}, nil
	})
	yuossdk.HandleDesktop(func() error {
		return nil // Open your window with any GUI toolkit.
	})
	if err := yuossdk.RunApp(); err != nil {
		log.Fatal(err)
	}
}
```

See [Command line and desktop](../api.html#go-mod-app).

## MCP App service

Use the [MCP example manifest](../../docs/app-development.md#quickstart-an-mcp-only-app) with `"runtime": "binary"`
and `mcp.entry` set to `server`. Build and include the Linux executable
before signing. `LoadMCPApp("")` loads
`$YUOS_APP_MANIFEST`, or `./app.json` during direct development.
`Bind` only accepts declared names, and `Serve` refuses to start until every
declared tool has exactly one handler.

```go
package main

import (
	"log"

	yuossdk "github.com/xiaoyu-work/Yu-OS/yuos-sdk/go"
)

func main() {
	app, err := yuossdk.LoadMCPApp("")
	if err != nil {
		log.Fatal(err)
	}
	err = app.Bind("hello-mcp.say", func(args map[string]any, call *yuossdk.MCPCall) (any, error) {
		if err := call.CheckCancelled(); err != nil {
			return nil, err
		}
		return args["message"], nil
	})
	if err != nil {
		log.Fatal(err)
	}
	if err := app.ServeStdio(); err != nil {
		log.Fatal(err)
	}
}
```

`MCPCall` implements `context.Context`, so handlers can select on
`call.Done()`, inspect deadlines, and pass the call directly to
`ChatContext(call, prompt, opts)` or `CallToolContext(call, name, args, appID)`.
The SDK propagates cancellation and deadlines without undoing accepted effects.
Authenticated caller and lineage data comes only from the Gateway-injected
`McpCallContext`; never derive identity from tool arguments.

Handlers may return ordinary values or explicit results:

- `TextMCPResult(text)` for successful text.
- `ErrorMCPResult(message)` for an MCP tool error.
- `StructuredMCPResult(object, text)` for `structuredContent` plus text.

`Serve(reader, writer)` is available for embedding and tests. It speaks
newline-delimited MCP JSON-RPC 2.0, serializes writes, supports progress,
cancellation, and authenticated deadlines, and returns fatal transport errors.

## App object references

```go
func objectReferenceExample() error {
	ref := yuossdk.ObjectRef{
		AppId: "notes", ObjectType: "note", ObjectId: "draft/1",
	}
	uri, err := yuossdk.FormatReference(ref)
	if err != nil {
		return err
	}
	_, err = yuossdk.ParseReference(uri) // app://notes/note?id=draft%2F1
	return err
}
```

`ObjectRef` is generated; `Revision` is `*string` so absence differs from a
present empty revision. Invalid references return `*ObjectRefError`.
Formatting and parsing preserve opaque IDs, reject invalid UTF-8 and
noncanonical URIs, and perform no discovery, filesystem access, or dispatch.
A reference does not assert existence, freshness, or permission.

Use generated `ValidateObjectRef` on decoded JSON before materializing the
struct. See [the shared contract](../wire/v1/object-references.md) for bounds
and the optional App manifest `objects` resolver declaration.

## AI support

Use `Chat(prompt, ChatOptions{})` or pass an MCP call as the context to
`ChatContext`. See the [model capabilities](../api.html#go-mod-ai).

Use `Embed`, `TTS`, `STT`, `ImageGen`, or `Decide` with `ModelOptions{}`. Every
model reply exposes `Provider`, `Model`, `Units`, and `Output`.
`STT` takes audio bytes; `Speech.Audio` and `Image.Image` hold the returned
bytes. `ChatOptions.Images` (image bytes) and `ChatOptions.Functions` extend a
chat request.
`OpenAiPermissionSettings(ctx)` opens the [model permission page](../../docs/app-development.md#model-permissions-in-settings)
after a user action and with the separate desktop launcher permission.

## Errors

Each domain returns typed errors you can switch on:

- `*AiDeniedError`, `*AiBudgetExceededError`, `*AiInvalidArgsError`,
  `*AiSafetyViolationError`, and `*AiUnavailableError` correspond to the
  [AI error codes](../reference.html#handle-ai-errors). `Payload` carries
  the structured error from Yu OS when present.
- `*ToolUnavailableError` — transport failure.
- `*ToolDeniedError` — capability / unknown-tool / arg-shape refusal.
- `*ObjectRefError` — pure object-reference validation; no operation was invoked.

## Binary resolution

The SDK runs `yu` from `$PATH`. Override with `YUOS_KERNEL_BIN` for
tests and dev setups.

## Develop

```sh
go vet ./...
go test -count=1 ./...  # fake yu; also reloads shared wire vectors outside this module
```

Generated types in `generated.go` are produced by
`python3 ../wire/codegen.py` — do not hand-edit them.
