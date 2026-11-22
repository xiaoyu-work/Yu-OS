# Yu OS SDK Reference

New to the SDK? Start with the [Quickstart](QUICKSTART.md).

## The model

Only the built-in System Agent can discover and call an App's enabled MCP
tools as an Agent. It needs no App-access grant. An owner can still use an
App directly from its UI or CLI. An App cannot invoke other Apps, directly
or through an App-owned AI agent. Apps may use gated AI, controlled system
services, and shared libraries; `tools.call` addresses a separate capability-
checked primitive catalog, not App MCP.

Outbound AI and catalog calls use a `yu --wire=1` subprocess. The OS verifies
authority against the registered workload; an App ID argument or environment
variable does not establish identity. Inbound App tools use private MCP stdio
owned by the App Host. The Gateway supplies caller context separately from
tool arguments. See [App, Agent, and AI integration](../docs/app-ai-integration.md).

Manifest helpers can validate requests such as `owner-path` without filesystem
access. [Named GUI entries and optional needs](../docs/app-development.md#multiple-gui-entries-under-one-app-identity)
retain one App identity. Only the OS resolves scopes, grants capabilities,
and mounts resources. [`yuos-runtime`](../yuos-runtime/) contains first-party
helpers; it is not a third-party SDK dependency.

For other capabilities, see [App events](wire/v1/app-events.md),
[managed channels](wire/v1/channel-ingress.md), and
[App data services](wire/v1/applet-services.md). Each requires the applicable
OS service and grant.

## App object references

Every binding exports the generated `ObjectRef` data contract and pure
format/parse helpers for canonical references such as
`app://notes/note?id=draft%2F1&revision=7`. References contain only an App ID,
object type, opaque object ID, and optional revision. They carry no owner,
grant, URI field, or data payload.

These helpers do not invoke `yu`, discover Apps, read data, or assert
existence, freshness, or permission. Terminal and desktop presentations share
the same identity semantics and backend. Actual resolution belongs to normal,
verified App operation/MCP command dispatch, with the existing capability and
audit gates. References introduce no legacy operation or App-local fallback.

See [the normative object-reference contract](wire/v1/object-references.md)
for byte limits, strict URI spelling, generated validation, shared golden
vectors, and the optional manifest `objects` declaration.

## App-reported file change plans

The generated `FileChangePlan` value and validators describe a bounded,
App-owned staged proposal, including its public diff and lifecycle state.
The App owns private before/proposed contents and the plan's lifecycle;
the SDK introduces no plan store, apply transport, or authority.

Review fingerprints bind proposal data, not permission. Normal authorization
and current-file checks still apply, and uncooperative writers may race after
the final check: this is not atomic compare-and-swap or universal rollback.
See [the file change plan contract](wire/v1/file-change-plans.md) for exact
fields, nullable values, reference semantics, and validation limits.

## App effect declarations

An ordinary operation or MCP tool may declare up to 16 effects in its manifest for a metadata-only
preview. These declarations do not grant authority, execute App code, inspect
object data or credentials, or promise file diffs. Missing effects mean
unknown effects, not purity; recovery is App-declared guidance, not proof that
an inverse exists.

See [the effect declaration and preview contract](wire/v1/operation-effects.md)
for fields, label limits, target bindings, unresolved runtime arguments, and
generated binding compatibility. No new SDK execution API is introduced.

## AI support

Yu OS calls the user's configured model API (or runs the configured local
model). It does not start the system Agent or attach its prompts, memory,
history, or tools. Yu OS keeps provider credentials; the App sends input
and receives the model result. The optional `system` text is the App's own
instruction to the model.

In `app.json`, add the capability you need, for example `{"verb": "ai.chat"}`.
Send the actual input in the SDK call, without a model ID. Exposing App tools
for the system Agent to call via MCP is a separate path.

Declare input origins (and, if needed, a safety profile) in the `ai` block.
Do not declare a budget. The owner sets one limit for App inference in
**Settings → AI Budget**. The limit covers all six model slots and all Apps
for the current UTC month; `0` means unlimited. Yu OS records usage even
without a limit and notifies the owner near or at the limit. Budget units
are not money, and the System Agent and Activity budgets are separate.

| Settings slot | SDK call or `infer` input | App capability | Result |
|---|---|---|---|
| `text` | `chat(prompt, …)` | `ai.chat` or `ai.chat.untrusted` | Text |
| `embed` | `{"kind":"embed","text":"..."}` | `ai.embed` | `vector` |
| `tts` | `{"kind":"tts","text":"..."}` | `ai.audio.tts` | `format`, `audio_base64`, and `sample_rate_hz` if known |
| `stt` | `{"kind":"stt","format":"wav","audio_base64":"..."}` | `ai.audio.stt` | `text` |
| `imagegen` | `{"kind":"imagegen","prompt":"..."}` | `ai.image.generate` | `format` and `image_base64` |
| `decision` | `{"kind":"decision","state":{},"questions":{…}}` | `ai.decide` | `answers` for the [System One questions](../docs/decision-model.md) |

Inference returns `{provider, model, units, output}`; `output.kind` matches
the requested kind. For example, Python:

```python
result = ai.infer({"origin": "user-input", "input": {"kind": "embed", "text": text}})
vector = result["output"]["vector"]
```

In Node, use `await ai.infer(request, {signal})`; Go uses `Infer(ctx, request)`;
Rust uses `ai::infer(&request)`. Input and output media cross as bytes encoded
in base64, not paths or URLs. A missing provider or runtime returns an error.

Both `infer` and `chat` require explicit origin: `trusted` for
developer-authored text, `user-input` for text typed by the user, and
`external-content` for files, web pages, email, or other outside content.
For chat, `external-content` selects `ai.chat.untrusted`. It does not
force a safety profile or prove provenance. The effective App and owner
policy controls redaction; `strict` and `standard` currently do the same
prompt-secret redaction, and `minimal` does not redact. No profile promises
prompt-injection detection.

For an optional GUI feature, put the verb in
[`optional_needs`](../docs/app-development.md#model-permissions-in-settings).
On a model permission error the App can show its own message and link to
Settings → Applications. The App cannot approve its own request.

Node AI and catalog calls return promises. Pass the MCP call's `signal` to
carry its cancellation and deadline through to `yu`. In Go, pass the
`*MCPCall` to `ChatContext`, `CallToolContext`, `BudgetContext`, or
`CatalogContext`. The Python and Rust `chat` helpers are synchronous;
checking an MCP call context in the handler does not interrupt those helpers.
Cancellation stops the cancellable client's CLI child; it cannot undo a
request the OS already accepted. Do not retry an indeterminate mutation
automatically.

The standalone budget query in Python, Node, and Go reports the App, period,
and units used. The CLI does not return a cap, so the query result has no
cap field. The chat reply retains its wire-v1 App usage snapshot.
`units_cap` is `0` unless an existing owner-set per-App override applies;
it never exposes the owner's total limit. Handle `BUDGET_EXCEEDED` without
automatically retrying or prompting for a new permission. `max_units` is
an optional ceiling for a particular request, not a manifest setting.

### App-owned tools

If the configured text model supports function calling, give it only
functions your App implements. The model proposes a call; **your App
validates and runs it**, then sends back the result. Yu OS does not run
these tools or add its own. No new manifest permission is needed for
the model request; any action your App takes still needs its own permissions.

```python
from dataclasses import asdict
from yuos_sdk import ai

request = {
    "origin": "trusted",
    "prompt": "Look up the price of tea, then tell me the price.",
    "tools": [{
        "name": "lookup_price",
        "description": "Look up the App's price for an item.",
        "parameters": {
            "type": "object",
            "properties": {"item": {"type": "string", "enum": ["tea"]}},
            "required": ["item"],
        },
    }],
}

reply = ai.chat_request(request)
if reply.tool_calls:
    results = []
    for call in reply.tool_calls:
        if call.name != "lookup_price" or call.input != {"item": "tea"}:
            results.append({"id": call.id, "content": "Unknown item", "is_error": True})
        else:
            results.append({"id": call.id, "content": "Tea costs $3."})
    turn = {"text": reply.text, "calls": [asdict(c) for c in reply.tool_calls], "results": results}
    if reply.provider_state:
        turn["provider_state"] = reply.provider_state
    reply = ai.chat_request({**request, "tool_turns": [turn]})
print(reply.text)
```

In Node, use `await ai.chatRequest(request, {signal})`; Go uses
`ChatRequest(ctx, request)`; Rust uses `ai::chat_request(&request)`.
Keep prior `tool_turns` in order if the model asks for another call.
Pass one result per call ID and echo optional `provider_state` unchanged.
For results derived from files, email or web pages, use `external-content`.
Each request is budgeted separately. Legacy `--tools` catalog names and
provider-hosted tool types are not accepted. This requires an updated runtime.

## SDK source and code generation

The [wire schemas](wire/v1/README.md) and [decoder contract](wire/v1/contract.json)
define generated types, validators, and MCP error constants. The language
directories own their transport and idiomatic API wrappers. See [MODULE.md](MODULE.md)
for the source and test layout.

Run codegen with:

```sh
cd yuos-sdk
python3 wire/codegen.py            # writes generated.* into each language tree
python3 wire/codegen.py --check    # verifies all generated outputs are current
```

Use `--sdk-only` with either command when regenerating a standalone SDK or
when another integrator owns the core MCP constants. It still generates all
four language bindings and the Rust SDK's MCP constants.

## Versioning

- **Wire protocol:** subprocess calls request `--wire=1` and check the version
  and shape of each response. They do not spawn a separate startup probe.
- **SDK packages:** Python, Node, Rust, and Go share the `yuos-sdk-v<version>`
  release and target the same generated CLI/MCP contract. Python derives its
  package metadata from the source `yuos_sdk.__version__`.
- **App runtime ABI:** the separate [App development platform](../docs/app-platform.md)
  records its ABI, export inventory, and version. SDK version, manifest
  `schema_version`, wire version, and installed runtime ABI are not interchangeable.

## Compatibility

Use SDK `0.1.0` with `yu` and the Yu OS runtime built from the same source
revision as a baseline. Live App calls need the installed runtime and running
broker; GUI and App data clients also need their OS services. You can use
pure helpers and test business logic off-system without those services.
`yu --wire=1 --version` checks the CLI and envelope, not feature availability
or broker readiness. A different OS build must implement wire v1 and the
specific operations or providers the App uses.

Wire v1 stays backward compatible: add operations or fields in shapes that
already allow them, without changing the existing behavior. A breaking change
to a closed shape or its meaning needs a versioned contract.

Generated decoders follow JSON Schema's mathematical-integer semantics:
finite `1`, `1.0`, and `1e0` values are equivalent integers. Bounds are
checked after type validation using lossless decimal-rational lexemes, and all
SDK adapters preserve accepted values. Node exposes u64-domain values as
`bigint` only when they cannot be represented safely as `number`.

Unrestricted JSON payloads use a stable lossless public model:

- Rust uses `serde_json::Value` and `serde_json::to_string`.
- Python uses ordinary JSON values plus `Decimal`/`WireDecimal`; use
  `generated.encode_wire_json`.
- Node uses ordinary JSON values plus `bigint` and `WireDecimal`; use
  `generated.stringifyWireJson`.
- Go uses `any` plus `json.Number` and the standard `encoding/json` encoder.

Fixed AI catalog tool arguments and model-proposed inputs can represent any
JSON value, including explicit `null`, scalars, and arrays. Each tool's own
schema still constrains what it accepts. Using the SDK serializer preserves
number precision. App MCP tools instead take an object of arguments declared
in the signed manifest.
Compact exponents that are unsafe to materialize remain `WireDecimal`
wrappers; serializers never expand them in memory. Node serialization rejects
non-finite and unsafe integer-valued native `number` values—use `bigint` or
`WireDecimal` instead.

## Releases

SDK packages are distributed through GitHub Releases, not PyPI, npm, or
crates.io. [Publish SDK Release](../.github/workflows/publish-sdk-release.yml)
is dispatched manually on `main` and requires successful Root validation
for the exact source commit. It checks version metadata and generated
bindings, tests all four languages, and produces:

- `yuos-sdk-v<version>` for the GitHub Release and Python/Node/Rust consumers;
- `yuos-sdk/go/v<version>` for Go module resolution;
- Python wheel and source distribution;
- Node package tarball;
- Rust `.crate` archive;
- Go and complete SDK source archives;
- portable Linux signing tools for amd64 and arm64, with notices;
- `SHA256SUMS`.

For distribution, pin a release artifact or tag. Verify downloaded archives
and signing tools against that release's `SHA256SUMS`. The signing tool does
not install the Yu OS runtime. The [App development platform](../docs/app-platform.md)
is a separate artifact.

## Language support

| SDK surface | [Rust](rust/README.md) | [Python](python/README.md) | [Node](node/README.md) | [Go](go/README.md) |
|---|---|---|---|---|
| Development toolchain | Stable Cargo/Rust used by CI | Python 3.10+ | Node.js 18+ | Go 1.21+ |
| Gated AI model calls and fixed catalog tools | Yes | Yes | Yes | Yes |
| Standalone AI usage query | — | Yes | Yes | Yes |
| Manifest-bound MCP services, progress, and cancellation | Yes | Yes | Yes | Yes |
| GUI bootstrap, object references, manifest helpers, events, and channels | Yes | Yes | Yes | Yes |
| App data services: Calendar, tasks, telemetry, and history preflight | Yes | — | — | — |

MCP App inputs use the [manifest argument model](../docs/app-development.md#3-the-minimum-viable-app),
not arbitrary nested JSON Schema. The separate [App data services protocol](wire/v1/applet-services.md)
is language-neutral, but only Rust has an SDK binding and it requires the
installed OS provider. A supported binding is not itself a capability grant.
