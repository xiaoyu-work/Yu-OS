# Quickstart: Call a model from your App

Build a text summarizer with `ai.chat` and the user's configured model.
Save these two files in `hello-ai/`:

**`main.py`**

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

**`app.json`**

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

### 1. Test locally

Point `YUOS_DEV_AI_URL` at a local or OpenAI-compatible model endpoint (such as
Ollama at `http://localhost:11434/v1`, with optional `YUOS_DEV_AI_MODEL` and
`YUOS_DEV_AI_KEY`) and run the command-line or desktop entry:

```sh
export YUOS_DEV_AI_URL="http://localhost:11434/v1"
yu app dev ./hello-ai summarize "We moved the launch to Friday because the tests aren't ready."
yu app dev ./hello-ai --gui
```

Or run `main.py` directly through the SDK:

```sh
YUOS_DEV_AI_URL="http://localhost:11434/v1" \
  python3 -m yuos_sdk.app ./hello-ai/main.py summarize "We moved the launch to Friday because the tests aren't ready."
```

```json
{"summary": "The launch moved to Friday because testing isn't finished."}
```

### 2. Package

Build `yuos-app-hello-ai_0.1.0_all.deb`:

```sh
yu app pack ./hello-ai
```

### 3. Submit

Submit the `.deb` to [`xiaoyu-work/yuos-app`](https://github.com/xiaoyu-work/yuos-app)
for Store review:

```sh
yu app submit yuos-app-hello-ai_0.1.0_all.deb
```

After review, the Store publishes it to the Yu OS APT repository.

[All model slots](REFERENCE.md#ai-support) ·
[Handle AI errors](REFERENCE.md#handle-ai-errors) ·
[Give the model your own tools](REFERENCE.md#app-owned-tools) ·
[Integrate an existing Linux App](../docs/linux-app-integration.md) ·
[Expose App tools over MCP](../docs/app-development.md#quickstart-an-mcp-only-app)
