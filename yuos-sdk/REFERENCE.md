# Yu OS SDK Reference

New to the SDK? Start with the [Quickstart](QUICKSTART.md).

## The model

The built-in System Agent discovers and calls enabled App MCP tools.
Owners can use an App from its UI or CLI. Each App runs with its own
capabilities.

AI and catalog calls use `yu --wire=1`. The OS authenticates the registered
workload and checks its capabilities. App MCP tools run over private stdio
managed by the App Host. Read caller identity from the Gateway's call context.
See [App, Agent, and AI integration](../docs/app-ai-integration.md).

Manifest helpers validate declarations such as `owner-path`.
[Named GUI entries and optional needs](../docs/app-development.md#multiple-gui-entries-under-one-app-identity)
retain one App identity. The OS resolves scopes, grants capabilities, and
mounts resources.

For other capabilities, see [App events](wire/v1/app-events.md),
[managed channels](wire/v1/channel-ingress.md), and
[App data services](wire/v1/applet-services.md). Each requires the applicable
OS service and grant.

## Desktop defaults

Apps get a private data directory. Ordinary signed Apps launched by the owner
get a [public-only outbound TCP proxy](../docs/app-development.md#10-what-an-operation-can-reach).

In a signed GUI, the [system Open dialog](../docs/app-development.md#open-a-user-selected-file)
grants the requesting window read access to the exact file the user selects.
The selection is the user's approval.

## App object references

Every binding exports the generated `ObjectRef` data contract and pure
format/parse helpers for canonical references such as
`app://notes/note?id=draft%2F1&revision=7`. A reference identifies an App,
object type, opaque object ID, and optional revision. Resolve it through the
declared App operation or MCP tool; the OS checks capabilities at dispatch.

See [the normative object-reference contract](wire/v1/object-references.md)
for byte limits, strict URI spelling, generated validation, shared golden
vectors, and the optional manifest `objects` declaration.

## App-reported file change plans

The generated `FileChangePlan` value and validators describe a bounded,
App-owned staged proposal, including its public diff and lifecycle state.
The App owns the plan's contents and lifecycle. A review fingerprint binds the
proposal data. Applying the plan still requires authorization and a current-file
check; another writer can change the file after that check.
See [the file change plan contract](wire/v1/file-change-plans.md) for exact
fields, nullable values, reference semantics, and validation limits.

## App effect declarations

An operation or MCP tool may declare up to 16 effects for a metadata-only
preview. Undeclared effects are unknown. Recovery text gives App-authored
guidance.

See [the effect declaration and preview contract](wire/v1/operation-effects.md)
for fields, label limits, target bindings, unresolved runtime arguments, and
generated binding compatibility.

## AI support

The App sends input and receives a result from the user's configured model.
Yu OS manages model selection and provider credentials. The optional `system`
field in a chat request carries App-authored instructions.

Add the matching capability to `needs` in `app.json`, for example
`{"verb": "ai.chat"}`. Declare input origins and any safety profile in `ai`.

In **Settings → AI Budget**, the owner sets one limit across all Apps and
model slots for the UTC month. `0` means unlimited. Yu OS tracks usage
and notifies the owner near or at the limit.

| SDK call | Model input | App capability | `output` |
|---|---|---|---|
| `chat` | Prompt, optional images and App functions | `ai.chat` or `ai.chat.untrusted` | `text`, optional function calls |
| `embed` | Text | `ai.embed` | `vector` |
| `tts` | Text | `ai.audio.tts` | `format`, `audio_base64`, optional `sample_rate_hz` |
| `stt` | `audio_base64` and `format` | `ai.audio.stt` | `text` |
| `imagegen` | Prompt | `ai.image.generate` | `format`, `image_base64` |
| `decide` | `state` and [questions](../docs/decision-model.md) | `ai.decide` | `answers` |

Each call returns `provider`, `model`, `units`, and `output`. Import the
function you want:

```python
from yuos_sdk.ai import chat, embed

reply = chat("Summarize this sentence.", origin="user-input")
vector = embed("Text to index.", origin="user-input").output.vector
print(reply.output.text)
```

[Node](node/README.md#ai-and-catalog-tools), [Go](go/README.md#ai-support),
and [Rust](rust/README.md#ai-support) expose the same six calls.

Every call requires explicit origin: `trusted` for
developer-authored text, `user-input` for text typed by the user, and
`external-content` for files, web pages, email, or other outside content.
For chat, `external-content` selects `ai.chat.untrusted`. Safety policy
controls redaction: `strict` and `standard` redact prompt secrets;
`minimal` preserves the original text.

For an optional GUI feature, put the verb in
[`optional_needs`](../docs/app-development.md#model-permissions-in-settings).
On a model permission error, show an App-written message and let the user
open Settings → Applications.

Pass the MCP call's `signal` to Node AI and catalog calls to propagate the
deadline and cancellation. In Go, pass the
`*MCPCall` to `ChatContext`, `CallToolContext`, `BudgetContext`, or
`CatalogContext`. The Python and Rust `chat` helpers are synchronous.
The cancellable clients stop their CLI child. Check the outcome of an accepted
mutation before retrying it.

Use `max_units` to limit an individual request. On `BUDGET_EXCEEDED`, tell
the user the request exceeded its budget. The standalone usage query in
Python, Node, and Go reports App ID, period, and units used.

### Image input

Use a chat model that supports images and declare `ai.chat.untrusted` with
`external-content` in `ai.origins`. Each image takes `format` (`png`, `jpg`,
or `webp`) and `image_base64`. Send up to four images, 512 KiB combined
before encoding.

```python
from base64 import b64encode
from yuos_sdk.ai import chat

def describe_image(png_bytes: bytes) -> str:
    reply = chat(
        "Describe this image.",
        origin="external-content",
        images=[{"format": "png", "image_base64": b64encode(png_bytes).decode("ascii")}],
    )
    return reply.output.text
```

### App-owned tools

With a chat model that supports function calling, pass the schemas for your
App's functions. The model proposes a call; **your App validates and runs
it** using its own permissions, then sends back the result.

```python
from dataclasses import asdict
from yuos_sdk.ai import chat

request = {
    "origin": "trusted",
    "prompt": "Look up the price of tea, then tell me the price.",
    "functions": [{
        "name": "lookup_price",
        "description": "Look up the App's price for an item.",
        "parameters": {
            "type": "object",
            "properties": {"item": {"type": "string", "enum": ["tea"]}},
            "required": ["item"],
        },
    }],
}

reply = chat(**request).output
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
    reply = chat(**request, tool_turns=[turn]).output
print(reply.text)
```

Keep prior `tool_turns` in order if the model asks for another call.
Pass one result per call ID and echo optional `provider_state` unchanged.
For results derived from files, email or web pages, use `external-content`.
Each request counts toward the App AI budget.

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
  and shape of each response.
- **SDK packages:** Python, Node, Rust, and Go share the `yuos-sdk-v<version>`
  release and target the same generated CLI/MCP contract. Python derives its
  package metadata from the source `yuos_sdk.__version__`.
- **App runtime ABI:** the separate [App development platform](../docs/app-platform.md)
  records its ABI, export inventory, and version.

## Compatibility

Use SDK `0.1.0` with `yu` and the Yu OS runtime built from the same source
revision as a baseline. Live App calls need the installed runtime and running
broker; GUI and App data clients also need their OS services. Use pure helpers
and business logic tests off-system. `yu --wire=1 --version` checks the CLI
and envelope. Other OS builds must provide wire v1 and the operations the App uses.

Wire v1 supports additive changes within extensible shapes and preserves
existing behavior. Breaking changes need a versioned contract.

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

SDK packages are distributed through GitHub Releases.
[Publish SDK Release](../.github/workflows/publish-sdk-release.yml)
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
and signing tools against that release's `SHA256SUMS`. See the
[App development platform](../docs/app-platform.md) for development artifacts.

## Language support

| Binding | Development toolchain |
|---|---|
| [Rust](rust/README.md) | Stable Cargo/Rust used by CI |
| [Python](python/README.md) | Python 3.10+ |
| [Node](node/README.md) | Node.js 18+ |
| [Go](go/README.md) | Go 1.21+ |

All four bindings support model and catalog calls, manifest-bound MCP with
progress and cancellation, GUI bootstrap, object references, manifest
helpers, events, and channels. MCP tool inputs follow the
[manifest argument model](../docs/app-development.md#3-the-minimum-viable-app).

Python, Node, and Go support standalone AI usage queries. Rust provides
[App data services](wire/v1/applet-services.md) through the installed OS provider.
