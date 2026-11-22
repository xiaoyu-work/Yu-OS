# yuos-sdk

Official SDK for building apps that run on Yu OS.

This directory is the **single source of truth** for the developer-facing
surface of Yu OS. Everything that's not in here is internal kernel
plumbing or vendored upstream code — third-party app developers should
need nothing more than what this folder exports.

Apps expose MCP services to the system Agent, permitted external agents, and
authenticated human CLI/desktop clients. They cannot call other Apps, directly
or through App-owned AI agents. Cross-App orchestration belongs to the built-in
system Agent. Apps may use gated AI, controlled system services, and shared
libraries; `tools.call` addresses the fixed AI primitive catalog, not App MCP.

Start with the [two-file MCP App quickstart](../docs/app-development.md#quickstart-an-mcp-only-app).

An App may declare separate signed GUI executables in optional
`desktop.entries`, without another installed identity or renderer unification.
`yu app <id> --gui:<name> [literal args...]` selects one declared entry;
each supplies its own localized name, literal selector and capability needs.
Optional `desktop.entry` and `desktop.needs` affect only the default GUI.
An explicit empty needs array means none, while omission preserves the legacy
operation-only needs behavior. See
[the manifest contract](../docs/app-development.md#multiple-gui-entries-under-one-app-identity).
The generated Rust/Python/Node/Go manifest types retain these additive fields;
they create no capability, resource mount or new runtime privilege.

The shared `scope: {"kind":"owner-path","path":"Documents/**"}` binding requests
only an authenticated owner's bounded Path scope. Public `manifest` helpers
validate and preview it without environment/filesystem access; Root independently
resolves identity/home, canonical containment, protected resources and approval.
High-risk deletion and `ui.panel` require their own ordinary owner approval;
neither new metadata nor a panel label supplies a default grant.

GUI manifests can place nonessential features in default/named
`optional_needs` arrays. Missing, denied or unapproved rights leave those
features unavailable without blocking startup or prompting for new authority.
Mandatory needs, especially required `ui.panel`, remain strict. Root uses only
existing parent or exact-owner reusable authorization; the arrays and purposes
remain part of review and deny inventory. Startup ungranted indexes are
presentation data, not a live grant or a new on-demand permission API.

## Layout

```
yuos-sdk/
├── wire/                Wire protocol — language-agnostic
│   ├── v1/
│   │   ├── README.md           protocol overview
│   │   ├── envelope.schema.json    common envelope shape
│   │   ├── perms.schema.json       capability decision envelope
│   │   ├── ai.schema.json          AI wire schema (kernel protocol)
│   │   ├── budget_show.schema.json yu agent budget show reply
│   │   ├── tool.schema.json        catalog tool invocation
│   │   ├── tool_catalog.schema.json catalog tool list
│   │   ├── mcp_call_context.schema.json authenticated App-call identity
│   │   ├── object_ref.schema.json portable App object identifier
│   │   ├── object-references.md    canonical URI and manifest semantics
│   │   ├── file_change_plan.schema.json bounded App-reported proposal
│   │   ├── contract.json           validators + stable error codes
│   │   ├── app.schema.json         yu app <id> <verb>
│   │   └── manifest.schema.json    app.json schema
│   └── codegen.py        reads wire/v1 schemas + contract → emits
│                         typed validators into rust/python/node/go
│                         and MCP error-code modules.
│
├── rust/                Rust SDK (cargo crate `yuos-sdk`)
│   ├── Cargo.toml
│   ├── README.md
│   ├── examples/
│   └── src/
│       ├── lib.rs               top-level re-exports + transport
│       ├── envelope.rs          common envelope parse / error
│       ├── ai.rs                gated chat API
│       ├── mcp/                 native App MCP server runtime
│       ├── tools.rs             tools::call / tools::catalog
│       ├── objects.rs           pure App object reference helpers
│       └── generated.rs         codegen output (envelope types)
│
├── python/              Python SDK (pip package `yuos-sdk`)
│   ├── pyproject.toml
│   ├── README.md
│   └── src/yuos_sdk/
│       ├── __init__.py
│       ├── ai.py, tools.py, mcp.py, kernel.py, objects.py, yuos_session.py
│       └── generated.py
│
├── node/                Node SDK (npm package `@yuos/sdk`)
│   ├── package.json
│   ├── README.md
│   └── src/
│       ├── index.ts             top-level re-exports
│       ├── transport.ts         subprocess transport
│       ├── ai.ts, tools.ts, objects.ts
│       └── generated.ts         codegen output
│
├── go/                  Go SDK (module github.com/xiaoyu-work/Yu-OS/yuos-sdk/go)
│   ├── go.mod
│   ├── README.md
│   ├── transport.go
│   ├── ai.go, tools.go, objects.go
│   └── generated.go
│
└── README.md
```

> The OS-internal `policy`, `snapshot`, and Rust `fs / exec / pkg /
> notify / net` helpers used by the bundled yuos apps live in the
> sibling [`yuos-runtime/`](../yuos-runtime/) tree, not here. That
> package is not published or supported for third-party use, and none
> of those helpers is exported by a public SDK. Public SDK operations
> are capability-checked by the `yu` kernel when they run.

## The model

Every SDK consumes the same **wire protocol v1**. Outbound AI and
system calls use the JSON envelope that `yu` writes to stdout.
Inbound App tools use private MCP stdio owned by the App Host, with a
Gateway-authenticated call context that is never taken from tool
arguments.

```
┌──────────────────────┐
│  your app (any lang) │
└──────────┬───────────┘
           │ language-native chat call (explicit prompt origin)
           ▼
┌──────────────────────────────────────────────────────────┐
│  yuos-sdk (rust / python / node / go)                 │
│  - typed structs (generated from wire/v1/*.schema.json)  │
│  - language-idiomatic API surface                        │
└──────────┬───────────────────────────────────────────────┘
           │ subprocess: yu ai chat --app <id> --prompt-file <0600 temp>
           ▼
┌──────────────────────────────────────────────────────────┐
│  yu                  (Rust binary, in /usr/local/bin)   │
│  - caps gate, audit, budget, safety, provider routing    │
└──────────────────────────────────────────────────────────┘
```

The reason for the subprocess transport: **identity, audit, and
session context are inherited from process ancestry** (kernel-spawned
parent → app process → yu child). A pure-library binding can't claim
"App X is making this call" without that lineage.

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

Across Rust, Python, Node, and Go, the hand-written AI surface is `chat` /
`chat-untrusted`. Chat requires an explicit origin: `trusted` for
developer-authored text, `user-input` for text typed by the user, and
`external-content` for files, web pages, email, or other outside content.
`external-content` selects the `ai.chat.untrusted` capability. It does not
force a safety profile or prove provenance. The effective App and owner
policy controls redaction; `strict` and `standard` currently do the same
prompt-secret redaction, and `minimal` does not redact. No profile promises
prompt-injection detection. Unsupported modality helpers are not published.

For an optional GUI feature, declare
[`$configured-ai-model` in the App manifest](../docs/app-development.md#optional-system-ai-at-installation)
to request the owner's configured model in the protected installation
review. This is a separate choice, not authority to select a provider or
access a document. The SDK chat call does not change.

Node AI and catalog calls return promises. Pass the MCP call's `signal` to
carry its cancellation and deadline through to `yu`. In Go, pass the
`*MCPCall` to `ChatContext`, `CallToolContext`, `BudgetContext`, or
`CatalogContext`. Cancellation stops the SDK's wait and its CLI child; it
cannot undo a request the OS already accepted. Do not retry an indeterminate
mutation automatically.

The standalone budget query in Python, Node, and Go reports the App, period,
and units used. The CLI does not return a cap, so the query result has no
cap field. The chat reply's separate budget snapshot still returns its
actual cap.

## What to generate vs hand-write

Codegen handles the **boring** part:

| Generated         | Hand-written                                |
|-------------------|---------------------------------------------|
| Request / reply struct types | The transport (how to spawn `yu`) |
| Error code enum   | The high-level wrappers (`ai.chat`, `tools.call`) |
| Manifest schema bindings | Examples, docs, language-idiomatic helpers |
| `ObjectRef` and structural validators | Pure canonical URI helpers and semantic bounds |

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
- **SDK packages:** Python, Node, Rust, and Go share the `sdk-v<version>`
  release and target the same generated CLI/MCP contract. Python derives its
  package metadata from the source `yuos_sdk.__version__`.
- **App runtime ABI:** the separate [App development platform](../docs/app-platform.md)
  records its ABI, export inventory, and version. SDK version, manifest
  `schema_version`, wire version, and installed runtime ABI are not interchangeable.

## Compatibility

Use SDK `0.1.0` with the matching Yu OS runtime and `yu` build from the same
source revision as a known baseline. `yu --wire=1 --version` is an optional
envelope check, not an installed-service or feature probe. A different OS
build must implement both wire v1 and the specific operations or providers the
App uses. A matching envelope version by itself does not guarantee newer
features. Released artifacts and pins are immutable; a source change does
not publish or replace an artifact.

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

Tool-call inputs may be any JSON value, including explicit `null`, scalars,
and arrays. Passing a proposed input directly to the matching tool invocation
preserves its exact wire representation.
Compact exponents that are unsafe to materialize remain `WireDecimal`
wrappers; serializers never expand them in memory. Node serialization rejects
non-finite and unsafe integer-valued native `number` values—use `bigint` or
`WireDecimal` instead.

## Releases

SDK releases are published through GitHub rather than PyPI, npm, or crates.io.
The manually dispatched **Publish SDK Release** workflow validates a single
SemVer across all language manifests, regenerates wire bindings, runs every
language test suite, and creates:

- `sdk-v<version>` for the GitHub Release and Python/Node/Rust consumers;
- `yuos-sdk/go/v<version>` for Go module resolution;
- Python wheel and source distribution;
- Node package tarball;
- Rust `.crate` archive;
- Go and complete SDK source archives;
- `SHA256SUMS`.

Install from an immutable release artifact or tag, never from `main`.

## Language support

| SDK surface | [Rust](rust/README.md) | [Python](python/README.md) | [Node](node/README.md) | [Go](go/README.md) |
|---|---|---|---|---|
| Gated AI chat and fixed catalog tools | Yes | Yes | Yes | Yes |
| Standalone AI usage query | — | Yes | Yes | Yes |
| Manifest-bound MCP services, progress, and cancellation | Yes | Yes | Yes | Yes |
| GUI bootstrap, object references, manifest helpers, events, and channels | Yes | Yes | Yes | Yes |
| App data services: Calendar, tasks, telemetry, and history preflight | Yes | — | — | — |

MCP App inputs use the [manifest argument model](../docs/app-development.md#quickstart-an-mcp-only-app),
not arbitrary nested JSON Schema. The separate [App data services protocol](wire/v1/applet-services.md)
is language-neutral, but only Rust has an SDK binding and it requires the
installed OS provider. A supported binding is not itself a capability grant.
