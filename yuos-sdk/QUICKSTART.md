# Quickstart

Build, test, package, and submit your first AI-powered Yu OS App in under five minutes.

## 1. Install the SDK

On Yu OS, the SDK and `yu` CLI are preinstalled. When developing on macOS,
Windows, or another Linux distribution, install the SDK for your language:

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

---

## 2. Create the App

Scaffold a new App directory with `yu app create hello-ai --kind both`, or
create `hello-ai/` with the two files below:

### Code (`main.py` / `main.ts` / `main.go` / `src/main.rs`)

`@command` marks a command-line entry by function name (`yu app hello-ai summarize`).
`@desktop` marks the desktop entry (`yu app hello-ai --gui`). Both call the same
`summarize_text` function. If you only implement one of the two, Yu OS returns a
built-in unavailable response when the other is launched.

<details open>
<summary><b>Python</b> (<code>main.py</code>)</summary>

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
    # Open your window with any GUI toolkit and call summarize_text(...) from the UI.
    print(summarize_text("We moved the launch to Friday because the tests aren't ready."))
```

</details>

<details>
<summary><b>Node.js / TypeScript</b> (<code>main.ts</code>)</summary>

```ts
import { ai, app } from "@yuos/sdk";

async function summarizeText(text: string): Promise<string> {
  const reply = await ai.chat(text, { system: "Summarize in one short sentence." });
  return reply.output.text;
}

app.command(async function summarize(args) {
  return { summary: await summarizeText(args[0]) };
});

app.desktop(async () => {
  console.log(await summarizeText("We moved the launch to Friday because the tests aren't ready."));
});

app.run();
```

</details>

<details>
<summary><b>Go</b> (<code>main.go</code>)</summary>

```go
package main

import (
	"log"
	yuossdk "github.com/xiaoyu-work/Yu-OS/yuos-sdk/go"
)

func summarizeText(text string) (string, error) {
	reply, err := yuossdk.Chat(text, yuossdk.ChatOptions{System: "Summarize in one short sentence."})
	if err != nil {
		return "", err
	}
	return reply.Output.Text, nil
}

func main() {
	yuossdk.HandleCommand("summarize", func(args []string) (any, error) {
		summary, err := summarizeText(args[0])
		return map[string]any{"summary": summary}, err
	})
	yuossdk.HandleDesktop(func() error {
		_, err := summarizeText("We moved the launch to Friday because the tests aren't ready.")
		return err
	})
	if err := yuossdk.RunApp(); err != nil {
		log.Fatal(err)
	}
}
```

</details>

<details>
<summary><b>Rust</b> (<code>src/main.rs</code>)</summary>

```rust
use yuos_sdk::{ai, app};

fn summarize_text(text: &str) -> Result<String, ai::AiError> {
    let opts = ai::ChatOpts::new().system("Summarize in one short sentence.");
    Ok(ai::chat(text, opts)?.output.text)
}

fn main() -> Result<(), app::AppError> {
    app::App::new()
        .command("summarize", |args| {
            Ok(serde_json::json!({ "summary": summarize_text(&args[0])? }))
        })
        .desktop(|| {
            println!("{}", summarize_text("We moved the launch to Friday because the tests aren't ready.")?);
            Ok(())
        })
        .run()
}
```

</details>

### Manifest (`app.json`)

Declare `operations` once; command-line and desktop launches share it:

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

---

## 3. Run and test locally

Point `YUOS_DEV_AI_URL` at a local or OpenAI-compatible model endpoint (such as
Ollama at `http://localhost:11434/v1`, with optional `YUOS_DEV_AI_MODEL` and
`YUOS_DEV_AI_KEY`) and run the command-line or desktop entry:

<details open>
<summary><b>With the <code>yu</code> CLI</b></summary>

```sh
export YUOS_DEV_AI_URL="http://localhost:11434/v1"
yu app dev ./hello-ai summarize "We moved the launch to Friday because the tests aren't ready."
yu app dev ./hello-ai --gui
```

</details>

<details>
<summary><b>Direct language runner (without the <code>yu</code> CLI)</b></summary>

```sh
export YUOS_DEV_AI_URL="http://localhost:11434/v1"

# Python
python3 -m yuos_sdk.app ./hello-ai/main.py summarize "We moved the launch to Friday because the tests aren't ready."

# Node.js
node ./hello-ai/dist/main.js summarize "We moved the launch to Friday because the tests aren't ready."

# Go
go run ./hello-ai summarize "We moved the launch to Friday because the tests aren't ready."

# Rust
cargo run --manifest-path ./hello-ai/Cargo.toml -- summarize "We moved the launch to Friday because the tests aren't ready."
```

</details>

Output:

```json
{"summary": "The launch moved to Friday because testing isn't finished."}
```

---

## 4. Package

Build `yuos-app-hello-ai_0.1.0_all.deb`:

```sh
yu app pack ./hello-ai
```

---

## 5. Submit

Submit the `.deb` to [`xiaoyu-work/yuos-app`](https://github.com/xiaoyu-work/yuos-app)
for Store review:

```sh
yu app submit yuos-app-hello-ai_0.1.0_all.deb
```

After review, the Store publishes it to the Yu OS APT repository.

---

## Next steps

- [SDK API Reference](REFERENCE.md) — complete module, function, manifest, and error reference.
- [Integrate an Existing Linux App](../docs/linux-app-integration.md) — wrap C/C++ binaries or adapt Qt/GTK/Electron apps.
- [Expose App Tools over MCP](REFERENCE.md#mcp-api-yuos_sdkmcp) — let the Yu OS System Agent invoke your App.
