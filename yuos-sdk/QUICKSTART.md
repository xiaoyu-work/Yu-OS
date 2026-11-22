# Quickstart: Call a model from your App

Build a text summarizer with `ai.chat` and the user's configured model.

Use the current Yu OS Agent package with `yuosd` running and a chat model
configured in Settings. Save these two files in `hello-ai/`:

**`main.py`**

```python
from yuos_sdk.ai import chat

def run(command, args):
    reply = chat(args[0], origin="user-input", system="Summarize in one short sentence.")
    return {"summary": reply.output.text}
```

**`app.json`**

```json
{
  "id": "hello-ai",
  "version": "0.1.0",
  "name": {"en": "Hello AI"},
  "ai": {"origins": ["user-input"]},
  "operations": {
    "summarize": {
      "label": {"en": "Summarize text"},
      "args": [{"name": "text", "kind": "text", "required": true}],
      "needs": [{"verb": "ai.chat"}]
    }
  }
}
```

From the parent directory, sign with your
[system-trusted App key](../docs/extension-provenance.md#trust-roots).
Confirm the admin install, then grant AI consent and run as your normal user.

```sh
yu provenance sign --kind app --id hello-ai --version 0.1.0 \
    --path ./hello-ai --key "$HOME/.secrets/my-publisher.json" --entrypoint main.py
yu app install ./hello-ai --review
sudo yu app install ./hello-ai --no-consent
yu app consent grant hello-ai
yu app hello-ai summarize "We moved the launch to Friday because the tests aren't ready."
```

Approve any capability prompt. The model generates the text in `summary`, for example:

```json
{"summary": "The launch moved to Friday because testing isn't finished."}
```

[All model slots](REFERENCE.md#ai-support) ·
[Give the model your own tools](REFERENCE.md#app-owned-tools) ·
[Model permissions in Settings](../docs/app-development.md#model-permissions-in-settings) ·
[Expose App tools over MCP](../docs/app-development.md#quickstart-an-mcp-only-app)
