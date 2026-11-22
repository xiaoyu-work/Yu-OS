# AI Gate Module

## Purpose

`ai/` is the policy, consent, budget, and provider-facing gate used by apps and
system callers that request AI work outside the interactive agent loop.

## Responsibilities

- Authorize AI verbs/scopes and app identity.
- Apply consent and budget decisions.
- Enforce the owner's monthly App inference limit across all six slots.
  The UTC ledger counts settled usage and in-flight reservations even
  when unlimited. Settings displays settled usage; provider dispatch
  uses both. Legacy manifest budgets are not enforcement inputs.
- Notify the owner at 80% and when the limit is reached or a reservation
  cannot fit, at most once per threshold and UTC month. Notification
  delivery does not determine budget enforcement.
- Call the owner's configured model without Agent prompts, history, memory,
  system tools, or tool execution. The App supplies custom function schemas
  and returns the results; the gate rejects undeclared proposals.
- Record usage and deny direct provider ownership by apps.
- Serve `chat` through the authenticated `yuosd` `ai.chat` route. The gate uses
  the live App capability, verified manifest, user configuration and consent.
  It validates bounded image data and places it with the App's prompt in a
  user message. Images use `ai.chat.untrusted` via `external-content` origin
  and go to the configured chat model alongside text.
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
| `budget.rs`, `user_budget.rs` | Shared App ledger, atomic owner setting, and threshold alerts |
| `consent.rs` | Persisted consent decisions |
| `chat.rs` | AI chat request surface |
| `app_tools.rs` | Bounded App-owned function schemas, matched result replay, and proposal projection |
| `inference.rs`, `inference/` | Closed App model input, configured provider dispatch, and owner-local worker |

## Dependencies

The gate consumes capability/session identity and the LLM registry. Apps call
the gate through SDK/bridge surfaces; they never receive provider credentials.
All model-visible input and usage remain auditable.

## Tests

```bash
cargo test -p yu ai:: -- --test-threads=1
```
