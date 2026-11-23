# Quickstart: Call a model from your App

Build a text summarizer with `ai.chat` and the user's configured model.
Save these two files in `hello-ai/`:

**`main.py`**

`@command("summarize")` implements the command-line entry (`yu app hello-ai summarize`).
`@desktop` implements the desktop entry (`yu app hello-ai --gui`). If you only
implement one of the two, Yu OS returns a built-in unavailable response when the
other is launched.

```python
from yuos_sdk.ai import chat
from yuos_sdk.app import command, desktop

@command("summarize")
def summarize(args):
    reply = chat(args[0], system="Summarize in one short sentence.")
    return {"summary": reply.output.text}

@desktop
def open_window(files):
    reply = chat("Give a one-sentence welcome.")
    print(reply.output.text)  # Or open your window with any GUI toolkit.
```

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
[Model permissions in Settings](../docs/app-development.md#model-permissions-in-settings) ·
[Expose App tools over MCP](../docs/app-development.md#quickstart-an-mcp-only-app)
