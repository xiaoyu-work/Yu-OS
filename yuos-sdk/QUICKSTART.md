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

Replace `YOUR_MODEL_ID` with the exact ID configured in `yu agent setup text`.
This limits the App's permission; it does not select the model.

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
        "scope": {"kind": "fixed", "scope": {"kind": "name", "value": "YOUR_MODEL_ID"}},
        "why": {"en": "Summarize the text you enter."}
      }]
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

[AI permissions and other input sources](../docs/app-development.md#7-ai-features) ·
[Let the system Agent call your App (MCP)](../docs/app-development.md#quickstart-an-mcp-only-app)
