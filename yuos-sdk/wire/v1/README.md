# Wire protocol v1

The wire protocol is the **stable contract** between every `yuos-sdk`
language binding and the `yu` kernel binary. It is what every SDK
ultimately speaks; what users call from their app code is a
language-idiomatic wrapper over this protocol.

## Stable AI surface

Chat uses `ai.chat`. The
`yu ai infer --request-stdin` operation accepts
[`ai_inference_request.schema.json`](ai_inference_request.schema.json) and
returns [`ai_inference.schema.json`](ai_inference.schema.json) in the same
wire-v1 envelope. It uses the owner's embedding, TTS, STT, image generation,
or local decision slot.

[`chat --request-stdin`](ai_chat_request.schema.json) accepts up to 720 KiB
of JSON, with up to four images, 16 App-owned functions and 16 prior tool
rounds. Send each image as standard padded base64 of its PNG, JPEG, or WebP
bytes; Yu OS detects the format from the bytes. Images may total 512 KiB decoded.
The App executes proposed calls. Include the same functions on follow-up calls, one result
for every prior call ID, and optional `provider_state` from the reply
unchanged.

The named SDK calls supply `origin: "app"` in the wire request. The
effective App policy controls redaction of text prompts.

`yu --wire=1 ai chat --request-stdin --stream` accepts the same request and
opts into newline-delimited JSON on stdout. Zero or more
[`text_delta`](ai_chat_delta.schema.json) records precede one ordinary chat
envelope. Text is provisional until the terminal success envelope and exit
status 0. No function proposals or provider state are released as deltas;
they are validated in the final reply. A missing terminal envelope is an
error. Each delta is at most 16 KiB of UTF-8 and each stdout line at most
8 MiB including its newline. Output length is not capped unless the App sets
`max_units`, and the SDKs set no total deadline; Yu OS stops a provider that
stays silent for the configured request timeout.

Inference stdin accepts up to 720 KiB of JSON. `embed` text is limited to
16 KiB, TTS and image prompts to 4 KiB, STT input bytes to 512 KiB, and
decision requests to the local decision model's 16 KiB request envelope.
STT `audio_base64` is standard padded base64 of WAV, MP3, or Ogg bytes; Yu OS
detects the format from the bytes.
An output is one vector (at most 8192 values), one transcript (at most
64 KiB), one image or audio result (less than 3 MiB), or one decision reply.
Image and audio results are standard padded base64 with their `format`; the
named SDK calls return the decoded bytes.
Oversized replies fail; the broker does not silently truncate them.

All slots count toward the owner's monthly App inference limit. Chat charges
the usage its provider reports. Non-chat calls charge operation units after
they succeed: sanitized UTF-8 bytes for embedding, at least 50
units or one per sanitized text byte for TTS (whichever is higher), 50 units
per started KiB of STT audio, 1000 per image, and one unit per serialized byte
of decision input. These are policy weights, not a provider invoice. Optional
`max_units` can lower the per-call ceiling. Calls are admitted while recorded
usage is below the limit, so concurrent calls can finish slightly over it. A
call that ends without reported usage records none.

The v1 chat `budget` snapshot is App-scoped. `units_cap` is zero unless a
legacy owner-set per-App override applies. The owner aggregate and its limit
are only shown in system Settings, not returned to Apps.

A GUI model choice that is off returns `PERMISSION_DENIED` with
`detail.reason = "model_permission_required"`, the `app_id` and `verb`, and
`settings_page = "applications"`. `yu ai settings` opens that page for a GUI
with the separate `desktop.launch:com.yuos.Settings` grant. It cannot grant
permission or retry the model request.

## Transport

**v1 uses a subprocess transport.** A request is encoded as
`yu --wire=1` plus command-line arguments and, optionally, stdin JSON. The
reply is exactly one JSON **envelope** on stdout unless chat streaming is
explicitly requested. Exit code 0 accompanies a
success envelope; non-zero accompanies an error envelope. SDKs reject flat
command output, JSON on stderr, malformed envelopes, unsupported versions, and
an exit-status/`ok` mismatch.

The OS authenticates the workload using its registered session and process
ancestry. CLI arguments and environment variables do not establish that
identity. The subprocess adapter is not a standalone runtime: live calls
still need the installed Yu OS services.

## Envelope

Every reply is one JSON object that conforms to `envelope.schema.json`.

Successful reply:
```json
{
  "ok": true,
  "data": {"value": 1},
  "wire_version": 1
}
```

Error reply:
```json
{
  "ok": false,
  "error": "permission denied",
  "code": "PERMISSION_DENIED",
  "wire_version": 1
}
```

`data` has a request-specific schema. `audit_id` is optional, as is the
error envelope's `detail` object. See [error codes](error_codes.md);
new codes may be added in v1 without changing existing meanings.

## Request families

| Family   | Route                                   | Schema                  |
|----------|-----------------------------------------|-------------------------|
| `policy` | OS-internal capability check; no public SDK API | `perms.schema.json` |
| `ai`     | `yu ai chat --app <id> [...] `         | `ai.schema.json`        |
| `tool`   | `yu ai tool <name> --app <id> --args <json>` | `tool.schema.json` |
| catalog  | `yu ai tools`                         | `tool_catalog.schema.json` |
| AI usage | `yu agent budget show <app>`          | `budget_show.schema.json` |
| `app`    | `yu app <id> <verb> [...]`             | `app.schema.json`       |

The `policy` wire family is consumed only by the OS-internal
`yuos-runtime` package. It is not an importable API in any public SDK;
third-party SDK calls are capability-checked by the `yu` kernel.

App manifests (`app.json`) are validated against `manifest.schema.json`.

## App data service helper

The separate [App data service v1](applet-services.md) uses private Linux pipes
to a fixed OS helper, not this CLI envelope or an App-to-App route. Its public
Rust client and [shared conformance cases](applet-services.cases.json) cover
Calendar, task and telemetry records plus a history-permission preflight.
The generated CLI/MCP decoder set is unchanged; installing this interface
does not grant App or desktop-resource authority.

## Private App MCP calls

MCP-first Apps serve their manifest-declared tools over private stdio owned
by the Yu OS App Host. For every `tools/call`, the Gateway replaces any
caller-supplied Yu OS metadata and injects a value conforming to
`mcp_call_context.schema.json` under
`_meta["yuos.dev/call-context"]`. It binds the authenticated workload
principal to the call/trace correlation, owner, task/session, and
deadline. This context is descriptive, not authority; the App Host retains
the transient target capability grant.

Caller kinds are `system-agent`, `cli` and `app`. External agents, caller
`app_id`, `parent_call_id`, and `depth` are rejected, not translated. An
`app` caller exists only for a controlled App→App call: the caller declares
the exact `<app-id>/<tool>` in `app_calls`, the target tool lists it (or
`"*"`) in `app_access.callers`, and `caller.id` is the signed caller App id.
Only `app` callers may carry `assertions`, bounded caller-supplied strings
whose names the target lists in `app_access.assertions`. The target is
authorized only from its own `needs[]`. See ARCHITECTURE.md. The legacy `access.system_agent: false` keeps
the service CLI-only, and `access.external_agents` has no effect.

The Python runtime exposes that value through `yuos_sdk.mcp.current_context()`
and supports MCP progress tokens plus cooperative
`notifications/cancelled`. MCP-first runtimes reject calls without a valid
Gateway context.

## App effect metadata

Optional ordinary-operation and MCP-tool `effects` declarations support
[truthful metadata-only previews](operation-effects.md). These are App claims,
not authority or confirmed outcomes. Missing declarations mean unknown
effects, and recovery guidance is not proof that an inverse exists.

## App object data contract

[`object_ref.schema.json`](object_ref.schema.json) describes a portable
identifier, not another request family or transport. The pure helpers across
all four SDKs use [one canonical URI contract](object-references.md), with
shared byte limits and golden vectors. App manifests can optionally declare
`objects` with ordinary operation/MCP command resolvers. References carry no owner, grant,
data payload, or nested `wire_version`.

## App-reported file change plans

[`file_change_plan.schema.json`](file_change_plan.schema.json) defines the
public staged-plan value, including a bounded App-reported diff. See
[file change plans](file-change-plans.md) for the closed shape, semantic
validation, existing App object reference, and external-writer race
limitations. Plan IDs and review fingerprints are data, never authority or
OS-confirmed mutation.

## Error codes

See the [shared error codes](error_codes.md). A failure to start `yu` or
decode its reply is a client transport failure, not a wire error envelope.

## Versioning

The protocol version is announced in three places:

1. The SDK requests a version explicitly with `yu --wire=1`.
2. `wire_version` in every reply must equal the requested version.
3. `yu --wire=1 --version` can manually verify the envelope; SDKs do not
   spawn a startup handshake, and the probe does not establish feature availability.

Breaking changes to the envelope or another wire contract require an
explicitly versioned interface. Bug fixes preserving existing wire semantics
do not. The closed envelope and closed payload schemas reject unfamiliar fields.
