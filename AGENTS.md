# Repository Instructions for Coding Agents

This file is the shortest reliable path into the Yu OS repository. Use it to
choose the right subsystem before reading implementation files.

## Repository Documents

Read only the documents relevant to the task:

1. [`ARCHITECTURE.md`](ARCHITECTURE.md) — system components, dependency
   direction, entry points, and end-to-end data flows.
2. [`CONTRIBUTING.md`](CONTRIBUTING.md) — build commands and architecture rules.
3. [`docs/app-development.md`](docs/app-development.md) — Python app manifest,
   capability, SDK, lint, and install contracts.
   [`docs/extension-provenance.md`](docs/extension-provenance.md) — the
   signing/trust/install contract every App, Skill and MCP package must
   satisfy before it is trusted.
4. [`docs/image-architecture.md`](docs/image-architecture.md) — rootfs features,
   image profiles, and target identity.
5. [`packaging/README.md`](packaging/README.md) and
   [`docs/updating.md`](docs/updating.md) — Debian packages, APT publication,
   and installed-system updates.
6. The nearest component README, especially
   [`desktop/README.md`](desktop/README.md) before desktop work.
7. The nearest maintained `MODULE.md` for local responsibilities, key files,
   dependencies, and tests. Guides cover the major core/agent subsystems,
   adapters, SDK/runtime/crates/extensions, rootfs/targets/packaging,
   scripts, and workflows.

Source code and manifests are authoritative when prose is stale.

## Task Routing

Start with the smallest row matching the task. Read callers and tests before
editing additional surfaces.

| Task | Start here | Commonly coupled files |
| --- | --- | --- |
| `yu` CLI command or primitive | `core/src/main.rs`, `core/src/router.rs` | The primitive module, `core/src/yuosd/`, inline Rust tests |
| Model-visible content or prompt-injection containment | `core/src/agent/trust/` | `agent/prompt/`, `agent/safety/untrusted.rs`, the ingestion adapter, `test/unit/agent/trust/` |
| Activity goal/lifecycle or presentation | `core/src/activities/`, `core/src/yuosd/activities.rs`, `docs/activities.md` | `core/src/activity.rs`, task/session associations, Agent Web, `desktop/agent/`; one shared backend for headless and desktop |
| Activity execution limits or attempt accounting | `core/src/activities/execution_limits.rs`, `core/src/agent/service/execution_limits.rs`, `docs/activity-execution-limits.md` | Root claim reservations, actual turn ceiling, supervisor/standalone expiry and revocation, CLI/Web/native; constraints never grant capabilities |
| Activity monetary budget or model-turn accounting | `core/src/activities/monetary_budget.rs`, `core/src/agent/runtime/monetary_budget.rs`, `docs/activity-monetary-budgets.md` | SQLite policy/ledger, shared turn reservation/output cap/settlement, private worker exchange, owner-scoped broker and terminal controls; policy prices are not provider invoices or authority |
| Activity execution priority or shared queue admission | `core/src/activities/scheduling_policy.rs`, `core/src/agent/service.rs`, `docs/activities.md` | Owner-scoped SQLite CAS policy, authenticated broker and terminal controls, dynamic `Store::claim_one` ordering and bounded aging; never a Job field, authority grant, preemption or model decision |
| Activity capability or confirmation boundaries | `core/src/caps/activity_boundary.rs`, `core/src/yuosd/activity_capability_policy.rs`, `docs/activity-capability-policies.md` | Shared policy storage, root Job/lease identity, capability/approval gates, App grant bindings, live revocation and terminal/Web/native controls; policy never supplies permission |
| Activity event-triggered work | `core/src/triggers.rs`, `core/src/yuosd/scheduler.rs`, `docs/activities.md` | Owner/lifecycle/finite-limit preflight before consent, unattended task/session associations, durable delivery correlation and existing Activity result views; no model polling or serialized grants |
| Activity long-running automatic work | `core/src/activities/work.rs`, `core/src/triggers/work.rs`, `docs/activity-work.md` | Explicit scheduler-authorized enablement, same-owner event/time/completion wakeups, exact reserved Job/generation, caller-reported handoffs and conservative holds; shared CLI/TUI/Web/native state, never a new daemon or portable authority |
| Activity attention or decision summary | `core/src/yuosd/activity_attention.rs`, `core/src/notifications/`, `docs/activities.md` | Protected approval reads, associated Job issues/counts, task-linked notification projection, terminal/Web/native presentation; acknowledgement is never consent |
| App object contract or Activity object reference | `core/src/objects/`, `core/src/caps/manifest/objects.rs`, `docs/app-objects.md` | SDK wire/types/helpers, `yuosd/activity_objects.rs`, ordinary App dispatch, Web/native presentations |
| Activity object observations, relations or corrections | `core/src/activities/object_state.rs`, `core/src/yuosd/activity_object_state.rs`, `docs/object-state.md` | Shared SQLite history, owner/receipt/reference binding, CLI/Web/native presentations and recorded untrusted task context; no App data copy or inferred authority |
| App effect declaration or operation preview | `core/src/operations/`, `core/src/caps/manifest/effects.rs`, `docs/operation-previews.md` | Shared broker preview routes, SDK manifest schema, Activity Web/native views; no execution or authority |
| Activity execution receipt | `core/src/activities/receipts.rs`, `core/src/yuosd/activity_receipts.rs`, `docs/execution-receipts.md` | Normal App execution, grant-bound `agentd` reporting, immutable owner-scoped reports, redaction, terminal/Web/native views |
| Staged file plan or guarded replacement | `docs/file-change-plans.md`, `core/src/yuosd/file_changes.rs`, `packaging/apps.lock.json` | Exact session capabilities, authenticated App calls, indeterminate journal handling, external App-owned plans and shared receipts; never restore an OS-local App copy |
| App execution from a leased Agent task | `core/src/extension_host/`, `core/src/yuosd/app_services.rs`, `core/src/agentd/` | Isolated task/service Hosts, exact per-call authority, broker admission, runtime provenance, cancellation and Activity receipts; no dynamic App code in the model worker |
| Agent ask/chat loop | `core/src/agent/runtime/loop_.rs`, `core/src/agent/runtime/turn.rs` | `prompt/`, `tools/`, `memory/`, `llm/` |
| Agent terminal frontend | `core/src/agent/terminal/`, `docs/agent-terminal.md` | Yu OS-owned ratatui renderer, canonical conversations/tasks/approvals/Activities, `/workspace` context domains and `/directory` filesystem cwd, plain chat compatibility; external TUI binaries and compatibility protocols are not runtime dependencies |
| Request context, memory selection, or context budget | `core/src/agent/context/`, `core/src/agent/runtime/context.rs` | `prompt/`, memory tools, continuation/compaction recording; the model chooses additional reads through guarded tools |
| Agent worker process / broker isolation | `core/src/agentd/`, `core/src/bin/yuos-agentd.rs` | `yuosd/server.rs`, `agent/service.rs`, `yuosd.service`, `packaging/deb/build-debs.sh` |
| Dynamic App/MCP extension isolation | `core/src/extension_host/`, `core/src/bin/yuos-extension-host.rs` | `agentd/`, `yuosd/server.rs`, App/MCP tool integration, `yuosd.service`, packaging |
| Out-of-process Agent extension ABI | `core/src/agent_extensions/`, `core/src/extension_host/abi.rs` | `extension_host/agent_extension.rs`, agent runtime hooks/tools, provenance, audit, packaging |
| Extension package provenance | `core/src/provenance/`, `docs/extension-provenance.md` | Agent extension registry/host, package roots and adversarial process tests |
| LLM provider or model setup | `core/src/agent/llm/providers/`, `core/src/agent/llm/registry.rs`, `core/src/agent/setup.rs` | `types.rs`, `accumulate.rs`, streaming and non-streaming tests |
| Tool, guardrail, or approval | `core/src/agent/tools/registry.rs`, `core/src/agent/runtime/turn.rs` | `guardrails.rs`, hooks, capability checks, audit |
| Memory, recall, or sessions | `core/src/agent/memory/`, `core/src/session/` | runtime recording, prompt injection, audit/session CLI |
| Workspace context domains or file/folder sharing tags | `core/src/workspaces/MODULE.md`, `docs/workspaces.md`, `crates/yuosd-client/src/workspaces.rs` | Shared Activity database, owner-only broker, scoped Agent context and four presentations; tags are read-only data sharing, never App or execution grants |
| Personal-file vectors or semantic index lifecycle | `crates/MODULE.md`, `crates/yuos-semantic/src/indexer.rs`, `crates/yuos-embed/src/local.rs` | Fixed local Core inference pipe, managed Qwen3/ORT/GenAI bundle, atomic file revisions, live-root reconciliation and broker-side scoped retrieval; no cloud or stub fallback |
| Owner Memory center or learned-source edit/forget | `core/src/agent/memory/inspection.rs`, `core/src/yuosd/memory/inspection.rs` | `memory.inspect/read/update/forget` wire/client inventory, Web/native presentations, exact revisions, original-source provenance and preserved conversation evidence |
| Session journal or mutation bracketing | `core/src/session/journal/`, `core/src/yuosd/journal.rs` | `core/src/yuosd/server.rs` dispatch, `core/src/agentd/supervisor.rs`, authority audit, packaging modes |
| Proactive notification | `core/src/notifications/`, `core/src/yuosd/notifications.rs` | cron/triggers/heartbeat/nudges, Agent task lifecycle, Agent Web UI, desktop Agent bridge |
| Native notification popup, panel descriptor or delivery presentation | `core/src/yuosd/notifications/gui.rs`, `core/src/bin/yuos-notification-panel.rs` | Explicit signed-GUI `notifications.presentation` IPC resource, single Root desktop consumer, original external App UI, OS shell descriptor and checked GUI retirement |
| Nearby/AirDrop file sharing | `docs/nearby-sharing.md`, `core/src/yuosd/nearby/MODULE.md`, `crates/yuos-nearby/MODULE.md` | One owner-scoped service for CLI/TUI/Web/native, exact offer decisions, isolated non-root transport, checked Root radio retirement and signed package components; no Agent prerequisite or automatic file execution |
| `yuosd` RPC or privileged operation | `core/src/bin/yuosd.rs`, `core/src/yuosd/server.rs` | client RPC, caps, audit, the owning `yuosd` module |
| Broker wire protocol or a new broker route | `core/src/yuosd/routes.rs`, `core/src/yuosd/wire/`, `core/src/yuosd/transport/` | `client.rs`, every in-repo client, `audit_policy.rs`, `core/tests/yuosd_broker_socket.rs` |
| MCP client/server integration | `core/src/agent/tools/mcp/`, `core/src/config.rs` | tool registry and agent lifecycle attachment |
| Extension package provenance (App/Skill/MCP signing, trust roots, revocation) | `core/src/provenance/`, `docs/extension-provenance.md` | `core/src/apps.rs`, `core/src/agent/skills/loader.rs`, `core/src/agent/tools/mcp/discover.rs`, `packaging/deb/build-debs.sh` |
| App client/business/MCP operation | External `yuos-app` guide and owning `products/<group>/package.json` or `capabilities/<group>/package.json` | Declared App manifest, entrypoint and tests; versioned SDK/runtime; automatic App-main build selection and package delivery |
| App installation or requested-permission disclosure | `core/src/apps/installation.rs`, `core/src/apps/permission_review.rs` | Shared lint, CLI review, exact verified package, protected review controller and separate owner App policy; confirmation alone grants no runtime authority |
| OS preinstalled App defaults | `scripts/app_packages.py`, `core/src/yuosd/preinstalled.rs` | Exact signed release selection, separate owner denials, scoped App grants; never infer permission from a publisher or App ID |
| OS permission review UI or decision | `core/src/yuosd/system_review.rs`, `crates/yuosd-client/MODULE.md` | Shared App/capability DTO, protected display revisions, terminal and native renderers, bounded polkit helper, owner-only cancellation and existing approval authority |
| Authenticated display login or GUI App lifetime | `core/src/display_session/MODULE.md`, `core/src/yuosd/gui/MODULE.md` | `crates/yuos-display-{control,login}/`, `core/src/worker/gui_transport/`, compositor/session/PAM startup, checked permission retirement and `docs/updating.md` |
| Desktop sign-in or system updates | `desktop/hyprland-yuos/MODULE.md`, `docs/updating.md` | Quickshell first-run setup, OS-started Settings GUI in `yuos-app`, greetd/PAM and the shared offline APT unit |
| Bundled App product boundary or source fork | `docs/app-product-redesign.md`, external `yuos-app` guides, `packaging/MODULE.md` | Product UI/backend, App-owned common support, manifests, provenance/licenses, native launchers, account/state migration, compatibility packaging |
| Adapter | `adapters/<id>/app.json`, `adapters/<id>/main.py` | adapter tests and external binary dependency |
| App/SDK wire contract | `yuos-sdk/wire/`, language SDK package | generated bindings, conformance tests, `publish-sdk-release.yml` |
| App chat streaming or typed MCP bindings | `core/src/ai/`, `core/src/yuosd/ai_stream.rs`, `yuos-sdk/node/src/mcp.ts`, `yuos-sdk/rust/src/mcp/manifest.rs` | Shared App AI gate and broker transports, generated wire delta types, SDK type generators and handler adapters |
| Rootfs composition | `scripts/lib/image-profiles.sh`, `rootfs/build.sh`, `rootfs/features/` | target build script and package contents |
| WSL or Docker image | `targets/wsl/`, `targets/docker/`, `CONTRIBUTING.md` | local builds from the shared rootfs profile; not part of publication CI |
| Debian/APT package | `packaging/deb/`, `packaging/apt-repo/` | `release.yml` publishes Agent/Base/Desktop together, `scripts/ci/`, rootfs package-install features |
| Update downgrade protection | `core/src/update/`, `packaging/release-security/policy.json` | `packaging/deb/common/`, maintainer scripts, `packaging/apt-repo/verify-release-security.sh`, `docs/updating.md` |
| Website | `web/index.html`, `web/install/index.html`, `web/MODULE.md` | `web/stage.sh`, `assets/brand/`, `.github/workflows/publish-website.yml`, `scripts/ci/pages_files.py` |
| Desktop component | `desktop/README.md`, `desktop/PROVENANCE.md`, component README | `desktop/hyprland-yuos/` native staging and login; OS-owned `desktop/desktop-shell/`; standalone native Apps are built in `yuos-app` |
| Hyprland/Quickshell desktop replacement | `desktop/hyprland-yuos/MODULE.md`, `core/src/display_session/MODULE.md` | Full native compositor and OS Shell source, existing Root display/GUI authority, `core/src/update/system_desktop.rs`, `core/src/yuosd/desktop/windows.rs`; source presence never authorizes activation |
| Migrated App / shared-capability source | [`yuos-app`](https://github.com/xiaoyu-work/yuos-app), `packaging/apps.lock.json` | Product UI/business/MCP/native source and explicitly kinded capability clients live there; `scripts/app_sources.py` resolves main and stages one immutable build snapshot |
| CI workflow | `.github/workflows/` | scripts invoked by the workflow; only `test.yml` runs on pull requests, while publication workflows are manually dispatched or reusable |

## Development

Use Linux or WSL2. On Windows, Rust and Python commands should run inside WSL.
Clone into the Linux filesystem for rootfs/image builds; `/mnt/c` cannot
represent all device nodes, hardlinks, permissions, and case-sensitive paths
needed by the build.

Core development:

```bash
cargo check -p yu --lib
cd core && cargo build
```

Image and package entry points:

```bash
sudo ./build.sh wsl
./build.sh docker
ARCH=amd64 ./packaging/deb/build-debs.sh
```

Do not edit generated output under `build/` or `target/`.
`yuos-sdk/python/src/yuos_sdk/generated.py` is generated from the wire
schema; regenerate it from `yuos-sdk/` with:

```bash
python3 wire/codegen.py
```

## Testing

Use the narrowest existing test first. Core tests that mutate process-wide
environment variables must run serially when combined.

Tests that exercise a migrated product use a resolved immutable App source
snapshot. Run `python3 scripts/app_sources.py` first to fetch current App `main`
and record its exact SHA (CI does this explicitly). Do not maintain a commit
pin in the version-2 repository selection. Multi-step package builds retain
one generated snapshot through `YUOS_APP_SOURCE_LOCK`; SDK artifact pins,
runtime package verification and installed update authority are unchanged.
Cargo never downloads product fixtures or substitutes local App source for a
declared external App. The lock's optional `capabilities` list resolves only
`capabilities/<name>` with `kind: "shared-capability-client"`; `products` and
native preparation retain business-product ownership. Never recreate a local
`apps/doc`, `apps/db`, `apps/kv`, `apps/net` or `apps/summarize` fallback or count a shared-capability
group as a business product. Cross-repository runtime fixtures stage complete payloads
and invoke manifest-selected entrypoints over public MCP, not private App modules.
The 61 retained App sources belong to `yuos-app`: 23 business groups and
four shared-capability groups, with 50 Agent and 11 desktop identities.
Eleven messaging connectors, `gateway-email` and `widget-rail` are retired;
do not restore OS-local copies or source fallbacks.
App-owned helpers live in external `shared/python`, with tests/vectors in
`tests/shared`; no OS `apps/` source tree remains. Fixtures stage their common
support through the pinned public CLI into `usr/lib/yuos/python`, separately
from OS SDK/runtime and App payloads. Agent still delivers this support through
existing compatibility package assembly; installed ownership has not moved to
independent App APT packages.
Summarize fixtures use isolated synthetic public AI/policy/memory wire, never
paid/live models. The OS still owns AI consent, budgets, providers and memory
authority; source completion is not backend/state/identity or release independence.

Rust unit-test bodies live outside production source trees under each crate's
`test/unit/` directory, mirroring the `src/` path. Production modules contain
only a small `include!` declaration so unit tests retain private access.

During initial task discovery, do **not** scan `test/` or `tests/`. Read the
production entry point, types, and callers first. Open only the matching unit
test file after selecting the implementation, when confirming existing
behavior, adding a regression, or diagnosing a failure.

```bash
# Extension provenance (format, trust, verify, install, CLI)
cargo test -p yu --lib provenance:: -- --test-threads=1
cargo test -p yu --test extension_provenance_process -- --test-threads=1

# One Rust test or module
cargo test -p yu <test-filter> -- --test-threads=1

# Complete core suite
(cd core && cargo test -- --test-threads=1)

# Exact CI clippy command
(cd core && cargo clippy -- -D warnings)

# OS capture helpers: private PNG/interactive portal and checked Close, no desktop.
GITHUB_WORKSPACE="$PWD" RUNNER_TEMP="$PWD/build" \
  bash scripts/ci/test-boundaries.sh capture

# Browser broker and typed provider contract
cargo test -p yu --lib yuosd::browser -- --test-threads=1

# OS Applet providers (independent workspace; no App source preparation)
cargo test --manifest-path desktop/services/yuos-applet-services/Cargo.toml --lib --locked -- --test-threads=1
cargo test --manifest-path desktop/services/yuos-applet-services/Cargo.toml --features provider --locked -- --test-threads=1

# OS-owned Python suite; external App tests run in yuos-app
PYTHONPATH=yuos-sdk/python/src \
  python3 -m pytest -q adapters yuos-sdk/python/src \
    packaging/deb/tests/test_app_sources.py
```

Documentation-only changes do not require code tests.

## Conventions

- Commit and push this repository's code directly to `main`; do not create or
  push feature or temporary CI branches. Dispatch manual CI on `main`.
- Preserve unrelated dirty-worktree changes. Stage explicit paths rather than
  using `git add -A`.
- All public and bundled App SDK modules live in `yuos-sdk`.
- Apps declare operations/MCP tools and scopes in `app.json`; implementation
  belongs in the declared entrypoint and its modules. MCP-only Apps such as KV
  need neither `main.py` nor legacy operations. Schema inspection must not
  execute app entrypoint code.
- Apps never call model-provider SDKs directly. AI access goes through the
  Yu OS SDK/agent gate so consent, budgets, logging, and provider ownership
  remain centralized. Use AI verb-only needs; the owner chooses the model and
  monthly budget in Settings, not the App manifest.
  Use the named [SDK model calls](yuos-sdk/api.html#python-mod-ai) in examples.
- Ordinary Apps can use their own data, owner-selected files and default
  public egress as described in the [App guide](docs/app-development.md).
  Do not add App-to-App permissions. The built-in System Agent calls enabled
  App MCP tools through the authenticated App Host.
- Privileged or model-visible behavior must remain capability-gated and
  reconstructable from session/audit logs.
- Reuse the service-definition/provider/consumer seam documented in
  `CONTRIBUTING.md`; consumers must not reach around an interface to a concrete
  provider.
- Do not add broad catches, silent success fallbacks, or casts that bypass type
  safety.
- `desktop/` is a product fork with vendored origins. Preserve component
  licenses/provenance and avoid repository-wide reformatting.
- `crates/obscura-*` are vendored browser internals. Keep changes scoped and
  preserve their workspace boundary.
- `desktop/icons-tela/links/` contains case-sensitive symlink names. A Windows
  checkout reports false modifications for case-colliding paths; never stage
  those phantom changes.

## Cross-Surface Change Contracts

### New or changed app operation

Make client changes in the owning `yuos-app` source package, not an OS-local
copy or ignored composition cache. Compatible changes need no OS implementation
edit; new builds automatically consume App `main`, while installed changes
still require signed package delivery. Coordinate an OS change only when its public SDK/runtime,
protocol, authorization or other platform contract actually changes.

1. Update `app.json` operation args and `needs`.
2. Implement or change the declared entrypoint's handler.
3. Validate untrusted args before the policy check.
4. Update the App's behavior tests (`test_main.py` or explicitly declared tests).
5. Run `yu app lint <id>` when the binary is available.

Permission-related changes must also remain visible in the OS-generated
installation review, including conditional needs, argument constraints, AI
input policy and System Agent MCP availability. A review digest is comparison
data, never an authorization token. Ordinary installation or selecting a
default handler grants no requested capabilities. Preinstalled App defaults
come only from the OS package's verified release selection.

OS review presentation changes must cover both terminal and native renderers,
the root controller and the bounded helper. Decisions use the revision actually
displayed. App confirmations never contain permission choices; the same
screen may submit a distinct, exact owner App-policy decision. Renderers
advertise only enforced choices. Neither a new
renderer nor a returned JSON object is a new source of consent.

### New or changed capability

Trace the full chain: catalog/scope definition → enforcement/provider →
consumer/tool or app manifest. Update policy-facing tests and audit behavior in
the same change.

### New model-visible content source

1. Add the `SourceKind` variant in `core/src/agent/trust/source.rs`, giving it an
   ordinal, an `ALL` entry, and a profile (class, persistence, projection,
   audit). Omitting any of these fails to compile or fails the registry test.
2. Label the bytes at the ingestion adapter, not at the prompt boundary.
3. Let `PromptProjection` choose the channel. Only `SystemPolicy` reaches
   `system`/`developer`; everything else is a fenced user data message.
4. Add an adversarial test in `core/test/unit/agent/trust/adversarial.rs`
   asserting the label cannot rise and the fence cannot be escaped.
5. Never let a label reach `caps`, `policy`, `yuosd`, the approvals store, or
   the guarded tool registry.

### New or changed extension surface

An App, Skill or MCP/adapter package is authenticated before its
manifest, operations, capability needs, tool schemas or model-visible
text are trusted. When adding a discovery root, an install path or a
new package kind:

1. Verify through `crate::provenance::verify` — never re-read a mutable
   path after verification; read from the `VerifiedPackage` snapshot.
2. Quarantine failures with an actionable diagnostic; never drop them
   silently and never fall back to unverified content.
3. Keep trust roots compiled in. No environment variable, manifest
   field or model-reachable input may add a trust root or disable
   verification.
4. Add adversarial coverage under `core/test/unit/provenance/` and, for
   filesystem/process behaviour,
   `core/tests/extension_provenance_process.rs`.

### New or changed notification

Trace the full path: deterministic producer -> durable Notification Service ->
owner-scoped broker route -> enabled delivery adapters. Keep notification state
separate from audit and context-event journals, publish only after the source
state transition is durable, and cover deduplication, DND, retries, owner
isolation, and acknowledgement in tests. Background delivery must not depend
on an LLM choosing to invoke a notification tool.

### New or changed Activity behavior

Activities have one desktop-independent core service and owner-scoped broker
contract. Terminal, Web and native desktop clients are presentations, never
separate lifecycle or persistence authorities. Keep optional task/session
associations backward compatible; an execution completing never proves that
the Activity goal was achieved. Preserve explicit completion confirmation and
the existing capability, approval, task-cancellation and audit boundaries.
Update [`docs/activities.md`](docs/activities.md) with user-facing changes.

Automatic work is explicit opt-in through the existing scheduler delegation
boundary, not a consequence of `active`. Persist the exact dispatch identity
before publication and apply handoffs only after durable Job success and
checked containment retirement. Keep generated next-step text untrusted,
preserve finite limits and budget checks, and hold uncertain outcomes without
automatic replay. Work state/history and scheduler authority are never portable.

Portable continuity changes must keep one shared backend and a closed,
versioned, bounded document. Carry only semantic intent/references and rules
proven non-authoritative; never serialize Activity database rows, owner
identity, capabilities/approvals/consent, monetary state, Jobs/Sessions,
execution evidence, receipts/effects/results, journals/notifications,
object-state history or local paths. Imports require an explicit closed
placement, create paused Activities, preserve stable continuity lineage, and
apply safe rules atomically without starting work.

Monetary budgets remain separate from execution limits and authority. Reserve
before every Activity model dispatch, bind the durable UUID to Root-derived
owner/Activity/Job/session/turn identity, clamp outgoing output, and settle
only from provider-reported usage under the reservation's policy-rate snapshot.
Missing usage, errors and ambiguous retry/fallback accounting must not silently
refund. Document policy pricing as a conservative upper bound, never a
provider invoice.

Capability policies must constrain both standing permissions and approval
escalation. Deny precedes any consent side effect; required confirmation is
exact and single-use even when a capability is already held. App registration
and calls settle their whole approval set at root, not in a consuming local
preflight. Carry the policy binding through grant attenuation and recheck it
before broker effects. Policy edits stop old attempts through the existing
worker/App cleanup path, without undoing already-admitted effects.

App object declarations must be authenticated before labels or resolvers are
trusted. Keep canonical reference semantics shared through the public SDK;
describing or attaching a reference must not execute an App, copy its data,
or create authority. Explicit resolution reuses the normal App invocation
pipeline. See [`docs/app-objects.md`](docs/app-objects.md).

### New or changed Workspace behavior

Workspace context domains sit above Activities and remain separate from the
existing filesystem `Job.workspace` cwd. Use the shared owner database and
Root-derived optional conversation/Job associations; no frontend owns a
second lifecycle, context store or permission authority.

Keep shared context bounded and sourced, never a concatenation of complete
conversation histories. Filter memory sessions and file eligibility before
limits, counts, source pages or semantic ranking. File/folder tags are explicit
read-only sharing: recheck live tags, owner permissions, pinned no-symlink
paths and current file revision before returning bytes. Global personal-file
indexing is independent of tags and never falls back to cloud or fake vectors.

Temporary cross-Workspace reads select exact same-owner notes/files for one
Job or canonical conversation, with finite expiry, live revocation and no
transitive sharing. Imported bytes do not become owner notes, shared findings,
handoffs or policy. Visibility changes fence stale replay; preserve original
owner-visible evidence rather than silently rewriting a conversation.

Deletion removes shared bytes/tags and disables associated automatic work,
pauses preserved goals and requests ordinary standalone/Activity Job
cancellation. Unconfirmed retirement stays pending. Cover CLI, Yu OS TUI,
authenticated Agent Web and native Agent presentations at exact displayed
revisions, and update [`docs/workspaces.md`](docs/workspaces.md).

### LLM provider change

Cover configuration/setup, credential resolution, non-streaming probes,
streaming events, text/tool/reasoning round-trips, usage/error classification,
and provider-chain/pool behavior. A successful text-only request is not enough.

### GUI App or display lifetime change

GUI lifetime changes must preserve the Root PAM/kernel-login binding, private
display epoch and instance-scoped transport. Permission revocation must await
checked resource retirement without undoing durable denial on failure. A
process-local `gui::retire_app` call from standalone `yu` is not a broker
retirement acknowledgement; App install/update/rollback coordination remains a
separate Root-owned contract. Headless operation must not require a display.

### Rootfs or package change

Decide separately whether the change belongs in:

- a reusable rootfs feature (`rootfs/features/`);
- an installed Debian package (`packaging/deb/`);
- a target profile (`scripts/lib/image-profiles.sh`); or
- target-only packaging (`targets/`).

If installed-system update behavior changes, update `docs/updating.md`.

### Release-security change

`packaging/release-security/policy.json` and the constants in
`core/src/update/mod.rs` are one contract; a unit test fails when they diverge.
Raising the security epoch or the ABI generation is a coordinated change:
update the policy, the compiled constants, the package `control` dependencies
that encode the ABI, and `docs/updating.md`. Never lower an epoch, and never
weaken a gate to make a release install — publish a newer one, or use the
documented recovery authorization.

## Agent Workflow

Before editing:

1. Select the subsystem from the task-routing table.
2. Read its entry point, direct callers, data types, and nearest tests.
3. Search for the same concept before adding helpers or configuration.
4. Check `git status` and preserve concurrent user work.

While editing:

1. Keep changes within the verified dependency direction.
2. Cover every coupled surface in the relevant change contract.
3. Prefer a natural module extraction over adding more responsibility to an
   already oversized dispatcher.
4. Run targeted validation after each coherent behavior change.

Before completion:

1. Re-run the exact requirement, not a proxy.
2. Run `git diff --check`.
3. Confirm only intended paths are staged.
4. Update architecture/navigation docs when boundaries, entry points, commands,
   or cross-surface contracts changed.

## Documentation Updates

| Change | Documentation to check |
| --- | --- |
| Build/test command or contributor workflow | `AGENTS.md`, `CONTRIBUTING.md` |
| Component, dependency direction, entry point, or data flow | `ARCHITECTURE.md` |
| App/manifest/SDK contract | `docs/app-development.md`, SDK README |
| Image feature/profile/identity | `docs/image-architecture.md`, `rootfs/features/README.md` |
| Package or installed update behavior | `packaging/README.md`, `docs/updating.md` |
| Extension signing, trust roots, or package verification | `docs/extension-provenance.md`, `packaging/deb/yuos-agent/trust/publishers.d/README.md` |
| Desktop boundary or provenance | `desktop/README.md`, `desktop/PROVENANCE.md` |
