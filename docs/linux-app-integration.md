# Integrating an Existing Linux App with Yu OS

Turn an existing Linux application (compiled C/C++/Rust/Go binary, Python/Qt/GTK
desktop app, or Node/Electron app) into a Yu OS App without rewriting its UI or
business logic.

A Yu OS App uses standard Linux UI toolkits (Qt, GTK, QML, iced, Electron) and
standard libraries. Adding `app.json` and `yuos-sdk` gives the App:

1. **Sandboxed private storage** (`app.data_dir()`) and **macOS/Windows-style permissions** (`needs`).
2. **System AI access** (`ai.chat`, `ai.chat_stream`, `ai.embed`, `ai.tts`, `ai.stt`, `ai.imagegen`, `ai.decide`).
3. **Desktop helpers** (`gui.open_file()`, `app.notify()`, and the "Ask Yu OS" agent overlay).
4. **System Agent integration** (`mcp` tools callable by the built-in Yu OS Agent).

---

## Path 1: Wrap an existing C/C++ Linux binary with Python or Rust SDK

If you have an existing C/C++ Linux program (for example `bin/my-cpp-tool`), you
do not need to rewrite its C++ source. Keep the binary in your App folder and
wrap it with `yuos-sdk` in Python or Rust to add Yu OS AI and permissions:

<details open>
<summary><b>Python wrapper around a C++ binary</b> (<code>main.py</code>)</summary>

```python
import subprocess
from yuos_sdk.ai import chat
from yuos_sdk.app import command, desktop

def run_cpp_and_summarize(input_file: str) -> str:
    # 1. Run your existing C/C++ binary:
    raw_output = subprocess.check_output(["./bin/my-cpp-tool", input_file], text=True)
    # 2. Enhance its output with Yu OS AI:
    return chat(raw_output, system="Summarize the tool output in one sentence.").output.text

@command
def analyze(args):
    return {"summary": run_cpp_and_summarize(args[0])}

@desktop
def main():
    subprocess.run(["./bin/my-cpp-tool", "--gui"], check=True)
```

</details>

<details>
<summary><b>Rust wrapper around a C++ binary</b> (<code>src/main.rs</code>)</summary>

```rust
use std::process::Command;
use yuos_sdk::{ai, app};

fn main() -> Result<(), app::AppError> {
    app::App::new()
        .command("analyze", |args| {
            let out = Command::new("./bin/my-cpp-tool").arg(&args[0]).output()?;
            let text = String::from_utf8_lossy(&out.stdout);
            let reply = ai::chat(&text, ai::ChatOpts::new().system("Summarize in one sentence."))?;
            Ok(serde_json::json!({ "summary": reply.output.text }))
        })
        .desktop(|| {
            Command::new("./bin/my-cpp-tool").arg("--gui").status()?;
            Ok(())
        })
        .run()
}
```

</details>

Or if the C/C++ binary needs no AI wrapper at all, point `app.json` directly at
the binary (`"runtime": "binary"`):

```json
{
  "id": "my-viewer",
  "version": "1.0.0",
  "name": {"en": "My Viewer"},
  "runtime": "binary",
  "entry": "bin/my-viewer",
  "operations": {
    "inspect": {
      "label": {"en": "Inspect a file"},
      "args": [{"name": "file", "kind": "path", "required": true}]
    }
  }
}
```

Test, package, and submit:

```sh
yu app dev ./my-viewer inspect ./sample.pdf
yu app dev ./my-viewer --gui
yu app pack ./my-viewer
yu app submit yuos-app-my-viewer_1.0.0_all.deb
```

---

## Path 2: Integrate `yuos-sdk` into an existing Python / Qt / GTK App

Suppose you have an existing Python desktop or CLI tool:

```python
# Original Linux code
def summarize_document(text: str) -> str:
    return text[:200]
```

### Step 1: Add `@command` and `@desktop` entries and call `yuos_sdk.ai`

Keep your existing UI toolkit (PyQt6, PySide6, GTK4, Tkinter) and wrap the
entrypoints with `@command` and `@desktop`:

```python
from pathlib import Path
from yuos_sdk import gui
from yuos_sdk.ai import chat_stream
from yuos_sdk.app import command, data_dir, desktop, notify


def summarize_text(text: str) -> str:
    chunks = list(chat_stream(text, system="Summarize in one short sentence."))
    summary = "".join(chunks)
    # Save history in the App's private data directory:
    (data_dir() / "last_summary.txt").write_text(summary, encoding="utf-8")
    return summary


@command
def summarize(args):
    return {"summary": summarize_text(args[0])}


@desktop
def main():
    # Use your existing Qt / GTK window here.
    # Open a user-selected file through the system dialog:
    selected = gui.open_file("Select a document to summarize")
    if not selected:
        return
    summary = summarize_text(Path(selected).read_text(encoding="utf-8"))
    notify("Summary complete", summary)
```

### Step 2: Add `app.json`

Declare `operations` once; both command-line and desktop launches share it:

```json
{
  "id": "doc-summarizer",
  "version": "0.1.0",
  "name": {"en": "Doc Summarizer"},
  "ai": {},
  "operations": {
    "summarize": {
      "label": {"en": "Summarize text"},
      "args": [{"name": "text", "kind": "text", "required": true}],
      "needs": [
        "ai.chat",
        {
          "verb": "ui.notify",
          "scope": {"kind": "fixed", "scope": {"kind": "wild"}},
          "why": {"en": "Notify when a summary finishes"}
        }
      ]
    }
  }
}
```

### Step 3: Test locally, package, and submit

```sh
export YUOS_DEV_AI_URL="http://localhost:11434/v1"
yu app dev ./doc-summarizer summarize "Long document text..."
yu app pack ./doc-summarizer
yu app submit yuos-app-doc-summarizer_0.1.0_all.deb
```

---

## Path 3: Integrate a Node.js, Go, or Rust Linux App

The same pattern applies in Node.js, Go, and Rust:

### Node.js (`@yuos/sdk`)

```ts
import { ai, app, gui } from "@yuos/sdk";

app.command(async function summarize(args) {
  let out = "";
  for await (const chunk of ai.chatStream(args[0], { system: "Summarize briefly." })) {
    out += chunk;
  }
  return { summary: out, dataDir: app.dataDir() };
});

app.desktop(async () => {
  const path = gui.openFile("Select a file");
  if (path) app.notify("Selected file", path);
});

app.run();
```

### Go (`yuossdk`)

```go
package main

import (
	"context"
	"log"
	yuossdk "github.com/xiaoyu-work/Yu-OS/yuos-sdk/go"
)

func main() {
	yuossdk.HandleCommand("summarize", func(args []string) (any, error) {
		resp, err := yuossdk.Chat(args[0], yuossdk.ChatOptions{System: "Summarize briefly."})
		if err != nil {
			return nil, err
		}
		return map[string]any{"summary": resp.Output.Text}, nil
	})
	yuossdk.HandleDesktop(func() error {
		path, err := yuossdk.OpenFile("Select a file")
		if err == nil && path != "" {
			_ = yuossdk.Notify("Selected file", path)
		}
		return nil
	})
	if err := yuossdk.RunApp(); err != nil {
		log.Fatal(err)
	}
}
```

### Rust (`yuos_sdk`)

```rust
use yuos_sdk::{ai, app, gui};

fn main() -> Result<(), app::AppError> {
    app::App::new()
        .command("summarize", |args| {
            let reply = ai::chat(&args[0], ai::ChatOpts::new().system("Summarize briefly."))?;
            Ok(serde_json::json!({ "summary": reply.output.text }))
        })
        .desktop(|| {
            if let Ok(Some(path)) = gui::open_file("Select a file") {
                let _ = app::notify("Selected file", &path);
            }
            Ok(())
        })
        .run()
}
```

---

## Path 4: Expose App features to the Yu OS System Agent (MCP)

To let the built-in Yu OS Agent call your Linux App's functions in user tasks,
add an `mcp` block to `app.json` (`"schema_version": 2`) and bind handlers in
`server.py`:

```json
{
  "id": "doc-summarizer",
  "version": "0.1.0",
  "schema_version": 2,
  "name": {"en": "Doc Summarizer"},
  "ai": {},
  "mcp": {
    "entry": "server.py",
    "tools": [{
      "name": "doc-summarizer.summarize",
      "summary": {"en": "Summarize text in one sentence."},
      "args": [{"name": "text", "kind": "text", "required": true}],
      "needs": ["ai.chat"]
    }]
  }
}
```

```python
from yuos_sdk.ai import chat
from yuos_sdk.mcp import App, ToolResult

app = App.from_manifest()

@app.tool("doc-summarizer.summarize")
def summarize(text: str) -> ToolResult:
    reply = chat(text, system="Summarize in one short sentence.")
    return ToolResult.text(reply.output.text)

if __name__ == "__main__":
    app.serve()
```

---

## Migration Checklist for Traditional Linux Apps

| Traditional Linux pattern | Yu OS App equivalent |
|---|---|
| Hardcoded `~/.config/<app>` or `~/.local/share/<app>` | `app.data_dir()` (automatically isolated per App) |
| Direct `openai` / `anthropic` SDK calls with user API keys | `yuos_sdk.ai.chat()` / `chat_stream()` with `"needs": ["ai.chat"]` |
| Unscoped reads of arbitrary user folders | `gui.open_file()` for interactive selection, or `fs.read` in `needs` |
| Custom `notify-send` subprocesses | `app.notify(summary, body)` with `ui.notify` in `needs` |
| Custom `debian/postinst` root scripts | `yu app pack ./<dir>` (declarative `app.json`, no root scripts) |
