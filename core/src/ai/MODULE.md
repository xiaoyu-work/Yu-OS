# AI Gate Module

## Purpose

`ai/` is the policy, consent, budget, and provider-facing gate used by apps and
system callers that request AI work outside the interactive agent loop.

## Responsibilities

- Authorize AI verbs/scopes and app identity.
- Apply consent and budget decisions.
- Call the owner's configured model without Agent prompts, history, memory,
  or tool execution. Text chat rejects tool lists and unsolicited tool replies.
- Record usage and deny direct provider ownership by apps.
- Serve SDK `yu ai chat` through the authenticated `yuosd` `ai.chat` route.
  Workers receive neither the session registry, provider credentials nor network
  access. The gate uses the live exact App capability, owner configuration,
  verified manifest, consent, budget and safety policy; no local fallback exists.
- Serve SDK `yu ai infer` through the same App authority and budget. The owner
  configures the destination; binary data is bounded stdin/reply data.
  Configured local embeddings and decisions run as the owner in a fixed worker.
- Resolve App AI declarations without `scope` from the authenticated owner's
  settings. The same rule applies to operations, MCP tools, and GUI entries.
  Optional GUI choices use protected Settings and do not admit Agent-to-App calls.
- Persist safety-filtered provider requests in the owner's AI budget database
  (`ai_inputs`), correlated to the authenticated App session. This history is
  not App or agent memory. Cancellation retains the dispatched reservation's
  conservative estimate; completed calls settle actual usage.

## Key Files

| Path | Role |
| --- | --- |
| `gate.rs` | AI request validation, authorization, provider call |
| `consent.rs` | Persisted consent decisions |
| `chat.rs` | AI chat request surface |
| `inference.rs`, `inference/` | Closed App model input, configured provider dispatch, and owner-local worker |

## Dependencies

The gate consumes capability/session identity and the LLM registry. Apps call
the gate through SDK/bridge surfaces; they never receive provider credentials.
All model-visible input and usage remain auditable.

## Tests

```bash
cargo test -p yu ai:: -- --test-threads=1
```
