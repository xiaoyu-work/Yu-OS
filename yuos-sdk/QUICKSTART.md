# Quickstart: Use system AI in your App

This App takes a sentence from the terminal and asks Yu OS to summarize it.
The user configures the text model with `yu agent setup text`. Yu OS owns the
provider credentials and checks permission, consent, and budget before calling
the model. The App just calls `ai.chat`.

On Yu OS or Ubuntu with the Yu OS Agent package and `yuosd` running, save these
two files in `hello-ai/`. The Python SDK is included.

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
  "ai": {"origins": ["user-input"], "budget": {"monthly_units": 5000}},
  "operations": {
    "summarize": {
      "label": {"en": "Summarize text"},
      "args": [{"name": "text", "kind": "text", "required": true}],
      "needs": [{
        "verb": "ai.chat",
        "scope": {"kind": "fixed", "scope": {"kind": "name", "value": "$configured-ai-model"}}
      }]
    }
  }
}
```

`$configured-ai-model` means "use the model the user configured." Keep it as written.

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
[Model permissions in Settings](../docs/app-development.md#model-permissions-in-settings) ·
[Separate path: Agent calls your App (MCP)](../docs/app-development.md#quickstart-an-mcp-only-app)
