# Quickstart: Call a model from your App

This App summarizes text. The App calls `ai.chat`; Yu OS sends the request to
the model API the user configured in Settings. The App gets no API key or
access to the system Agent, its memory, or its tools.

With an up-to-date Yu OS Agent package and `yuosd` running, save these two
files in `hello-ai/`. The Python SDK is included.

**`main.py`**

```python
from yuos_sdk import ai

def run(command, args):
    reply = ai.chat(args[0], origin="user-input", system="Summarize in one short sentence.")
    return {"summary": reply.text}
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

`app.json` asks for permission to use AI chat. `main.py` sends the text to summarize.
The user chooses the model and budget in Settings. The App specifies neither.

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
[Separate path: Agent calls your App (MCP)](../docs/app-development.md#quickstart-an-mcp-only-app)
