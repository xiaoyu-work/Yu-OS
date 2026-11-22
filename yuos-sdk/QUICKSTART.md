# Quickstart: Call a model from your App

Build a text summarizer with `ai.chat` and the user's configured model.

Use the current Yu OS Agent package with `yuosd` running and a chat model
configured in Settings. Save these two files in `hello-ai/`:

**`main.py`**

`summarize(text)` calls the SDK. `run` passes the CLI argument to it.

```python
from yuos_sdk.ai import chat

def summarize(text):
    reply = chat(text, system="Summarize in one short sentence.")
    return reply.output.text

def run(_command, args):
    return {"summary": summarize(args[0])}
```

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
[Handle AI errors](REFERENCE.md#handle-ai-errors) ·
[Give the model your own tools](REFERENCE.md#app-owned-tools) ·
[Model permissions in Settings](../docs/app-development.md#model-permissions-in-settings) ·
[Expose App tools over MCP](../docs/app-development.md#quickstart-an-mcp-only-app)
