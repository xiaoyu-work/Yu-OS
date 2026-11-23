# Quickstart: Call a model from your App

Build a text summarizer with `ai.chat` and the user's configured model.

Use the current Yu OS Agent package with `yuosd` running and a chat model
configured in Settings. Save these two files in `hello-ai/`:

**`main.py`**

`@command("summarize")` marks the function that `yu app hello-ai summarize`
runs. `args` holds the text after the command name.

```python
from yuos_sdk.ai import chat
from yuos_sdk.app import command

@command("summarize")
def summarize(args):
    reply = chat(args[0], system="Summarize in one short sentence.")
    return {"summary": reply.output.text}
```

This App has a command-line version. Mark a function with `@desktop` to add
a [desktop version](REFERENCE.md#command-line-and-desktop).

**`app.json`**

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

### 1. Test locally (no signing required)

Point `YUOS_DEV_AI_URL` at any local or OpenAI-compatible model endpoint (such
as Ollama at `http://localhost:11434/v1`, with optional `YUOS_DEV_AI_MODEL` and
`YUOS_DEV_AI_KEY`) and run the App directory directly:

```sh
export YUOS_DEV_AI_URL="http://localhost:11434/v1"
yu app dev ./hello-ai summarize "We moved the launch to Friday because the tests aren't ready."
```

Without the `yu` CLI installed, you can also run `main.py` directly through the SDK:

```sh
YUOS_DEV_AI_URL="http://localhost:11434/v1" \
  python3 -m yuos_sdk.app ./hello-ai/main.py summarize "We moved the launch to Friday because the tests aren't ready."
```

```json
{"summary": "The launch moved to Friday because testing isn't finished."}
```

### 2. Package `.deb` for Store review

When ready to publish, pack the directory into a `.deb` archive (no signing key
needed) and submit it to [`xiaoyu-work/yuos-app`](https://github.com/xiaoyu-work/yuos-app)
(`submissions/incoming/`) for Store review:

```sh
yu app pack ./hello-ai
```

After the Store owner reviews the package, the Store signs and publishes it to
the official Yu OS APT repository.

[All model slots](REFERENCE.md#ai-support) ·
[Handle AI errors](REFERENCE.md#handle-ai-errors) ·
[Give the model your own tools](REFERENCE.md#app-owned-tools) ·
[Model permissions in Settings](../docs/app-development.md#model-permissions-in-settings) ·
[Expose App tools over MCP](../docs/app-development.md#quickstart-an-mcp-only-app)
