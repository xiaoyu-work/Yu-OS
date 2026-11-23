# AI Model Service

## Purpose

`ai/` dispatches model calls for the System Agent and for Apps using the SDK.

## Responsibilities

- Authorize AI verbs/scopes and app identity.
- Apply consent and budget decisions.
- Enforce the owner's monthly App inference limit across all six slots.
  The UTC ledger counts settled usage and in-flight reservations even
  when unlimited. Settings displays settled usage; provider dispatch
  uses both. Legacy manifest budgets are not enforcement inputs.
- Charge System Agent inference to `system.agent`, with its own configured
  cap and the same reservation and settlement mechanism.
- Notify the owner at 80% and when the limit is reached or a reservation
  cannot fit, at most once per threshold and UTC month. Notification
  delivery does not determine budget enforcement.
- Call the owner's configured model without Agent prompts, history, memory,
  system tools, or tool execution. The App supplies custom function schemas
  and returns the results; the gate rejects undeclared proposals.
- Record usage and deny direct provider ownership by apps.
- Serve `chat` through the authenticated `yuosd` `ai.chat` route. The gate uses
  the live App capability, verified manifest, user configuration and consent.
  It detects PNG, JPEG, or WebP from bounded image bytes and places them with
  the App's prompt in a user message. Text and images use the same `ai.chat`
  capability.
- Serve SDK `yu ai infer` through the same App authority and budget. The owner
  configures the destination; binary data is bounded stdin/reply data. STT
  detects WAV, MP3, or Ogg from the audio bytes.
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
| `model_service.rs` | Metered model calls, shared configured provider selection, and streaming settlement |
| `gate.rs` | Chat input, App policy and consent, and System Agent chat provider chain |
| `budget.rs`, `user_budget.rs` | Separate Agent and App accounts in the owner ledger, App settings and alerts |
| `consent.rs` | Persisted consent decisions |
| `chat.rs` | AI chat request surface |
| `app_tools.rs` | Bounded App-owned function schemas, matched result replay, and proposal projection |
| `inference.rs`, `inference/` | Typed App input and result projection for embeddings, speech, images, and decisions |

## Dependencies

The gate consumes capability/session identity and the LLM registry. Apps call
the gate through SDK/bridge surfaces; they never receive provider credentials.
All model-visible input and usage remain auditable.

## Tests

```bash
cargo test -p yu ai:: -- --test-threads=1
```
