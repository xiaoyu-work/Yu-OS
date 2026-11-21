# Local decision models

Decision is a separate model task, not a text-provider fallback. It answers
`choice`, `noul` (yes/no probability), and `score` questions over a shared
state in one request. It never creates tool calls or grants authority.

Configure **Decision model** independently of the conversation, speech, image,
and embedding models in **Settings → Agent**, in desktop first-run setup, or
with `yu agent setup decision`. First-run setup can save the selection before
the first owner signs in. It is optional and does not start a model as the
greeter. The OS installer does not download a decision model.

Run a compatible System One server as the signed-in owner on the private
Unix socket `/run/user/<uid>/yu-decision.sock`. For example, an
operator who has installed [compatible llama-server and H2O weights](https://huggingface.co/h2oai/h2o-lightning-4b-gguf)
can serve the text model with:

```bash
llama-server --host "$XDG_RUNTIME_DIR/yu-decision.sock" \
  --model h2o-lightning-4b-Q8_0.gguf --alias h2oai/h2o-lightning-4b -np 1
```

```bash
yu agent setup decision apply --provider system-one-local --model h2oai/h2o-lightning-4b
yu agent setup decision --status
yu agent setup decision test
```

**Test connection** in Settings and `yu agent setup decision test` send one
synthetic question over the local socket. Reset in Settings, or
`yu agent setup decision --reset`, clears only `decision`; the other models
are unaffected. The configuration still lives in `decision.model` in the
owner's `~/.config/yuos/config.json`. Empty or missing configuration leaves
the previous Agent behavior unchanged.

Yu OS doesn't install, launch, or download that server or its weights. The
client only uses the private socket and has no HTTP-network fallback. It
requires an owner-only runtime directory and an owner-owned socket, checks the
reported model and the exact set of answers. `decision.timeout_ms` defaults
to 5000 and accepts values from 100 through 30000.
`yu model status` reports configuration and socket presence, not model health.
To verify inference, pipe a request to `yu model decide`:

```json
{
  "state": {"subject": "Tomorrow's team agenda"},
  "questions": {
    "category": {
      "type": "choice",
      "instructions": "Which category matches the subject?",
      "criteria": {"meeting": "Meeting or agenda", "other": "Unrelated"}
    }
  }
}
```

Configured system Agent sessions can use `yu_decide` for the same request.
This is not an App AI permission or a new credential or provider API for
Apps. No decision request is added to ordinary chat unless a tool is called.

## Helium

`yu_browser_next` is available to an attended local Agent with the matching
verified Browser App and access to both underlying App tools. Given the
selected tab and an explicit navigation goal, it reads one compact `links`
snapshot, asks for one action-and-target choice within two seconds, and can
follow one sufficiently clear same-origin link. Link labels and the goal
reach the local model, not the page body, URLs, current form values or
browser reference tokens. Uncertain choices or a model failure select no
action. The Agent may then use its existing browser workflow.

The snapshot and navigation are separate authenticated App calls with the
existing tool hooks and exact `browser.dom.read` and `browser.nav` checks.
A hook cannot retarget a link selected for a different document or input.
The extension rechecks the active tab, page and link without invoking page
click handlers.
It returns a verified navigation only after seeing the target URL loaded;
an uncertain execution must be inspected, not replayed. The fast path does
not submit forms, evaluate JavaScript, or use desktop pixels.
