# Bundled App Runtime Module

## Purpose

`yuos-runtime/` provides internal runtime and policy helpers for apps bundled
with Yu OS.

## Responsibilities

- Perform capability policy checks against the hidden core bridge.
- Carry bounded source-scoped memory requests to OS authority without an
  App-local memory store or provider.
- Provide internal runtime/session helpers consistently across languages.
- Carry bounded bundled-App requests to daemon-owned browser and network
  providers without exposing privileged host sockets.
- Bind bundled Python App business handlers to manifest-authoritative MCP tools.
- Transport bounded file replacement requests on stdin through the broker,
  without direct-write fallbacks or parent-directory mount grants.
- Own the typed, bounded Ask Yu OS context and desktop overlay launch contract.
- Carry bundled attached-browser requests over stdin to the typed daemon
  provider without exposing the browser socket to App sandboxes.
- Keep bundled-app conveniences separate from the public SDK.

## Key Files

| Path | Role |
| --- | --- |
| `python/src/yuos_runtime/` | Python policy/runtime helpers |
| `python/src/yuos_runtime/memory.py` | Bundled `remember`/`forget` transport; OS-owned storage and exact memory scopes |
| `python/src/yuos_runtime/_broker_bridge.py` | Shared bounded stdin transport for typed private `yu` bridges |
| `python/src/yuos_runtime/network_diagnostics.py` | Typed client for daemon-owned host-network diagnostics |
| `python/src/yuos_runtime/mcp.py` | Strict bundled-App MCP operation binding |
| `python/src/yuos_runtime/browser_bridge.py` | Private bounded stdin bridge to `system.browser.control`; sensitive values never enter argv |
| `python/src/yuos_runtime/file_changes.py` | Literal-path validation, stdin-only guarded replacement with per-call session selection, and conservative failure reporting |
| `rust/` | Rust internal runtime crate |
| `rust/src/fs.rs` | Typed filesystem CLI client; writes send bounded JSON business arguments via `--args-stdin`; binary reads reject truncation |
| `rust/src/filesystem.rs` | App-safe controlled text-file provider client; complete reads and atomic unique replacements, no App intercall |
| `rust/src/desktop.rs` | Fixed native editor activation through the controlled desktop provider |
| `rust/src/capture.rs` | Typed non-interactive screenshot client; SDK stdin, exact destination confirmation, no App invocation or desktop transport |
| `rust/src/ask_yu.rs` | Typed context serialization, authenticated/readiness-gated Unix sockets, process isolation, and asynchronous child supervision |
| `README.md` | Boundary and usage |

## Dependencies

Only bundled/trusted apps depend on `yuos-runtime`. Third-party apps use
`yuos-sdk`. Policy helper errors are surfaced; missing `yu` or a denied
decision never silently becomes allow.

Bundled clients can be source-owned by `yuos-app`. DB depends on the
`yuos_runtime.policy.require` export and wire-v1 decision contract, not private
runtime helpers or providers. The exact platform source pin is reproducible
but is not an independently published runtime compatibility guarantee; see
the [bundled-client contract](README.md#bundled-client-contract).
Summarize consumes the declared `memory.remember` export and public SDK AI,
not private dispatcher/provider code. Its `self:summarize` memory requests
remain separate from other identities, and failures must propagate.

Inherited desktop apps keep app-specific `Serialize` context structs in their
small `claw_glue` modules and call `yuos_runtime::ask_yu::launch`. They do not
name the Agent UI executable, construct activation flags, or assemble JSON.
The Agent UI consumes the same runtime-owned activation type; context-bearing
launches remain transient while context-free activation may use D-Bus.

Context payloads are capped at 32 KiB and serialized inside a typed activation.
The runtime directly spawns a transient UI and waits for readiness on an
inherited Unix socketpair before writing a bounded frame. Public SDKs use the
packaged helper's abstract listener, authenticated to the direct parent with
`SO_PEERCRED`. No payload crosses argv, pipes, D-Bus,
audit, registry, environment, or filesystem boundaries. The handoff requires
Yama ptrace isolation and marks parent and child non-dumpable; startup failure
or timeout kills and reaps the exact child, while the launcher thread reaps a
successfully activated UI when it exits.

The production executable is fixed at `/usr/local/bin/yuos-agent-ui` and must
pass regular-file, root-owner, executable, and non-writable checks. Test
injection is private to the runtime's `cfg(test)` module.

The attached-browser helper invokes the fixed hidden `yu __browser` bridge
with only protocol selectors in argv. Its bounded JSON request travels over
stdin and is accepted only inside an authenticated App session; `yuosd`, not
the helper, derives browser capabilities and browser-socket authority.

Capture's native human CLI can preserve its connected stderr terminal through
the SDK while sending the same bounded request on stdin. Headless and MCP calls
still require an existing authenticated session; no bootstrap permission or
desktop socket is added by the runtime.

## Tests

Rust runtime unit tests live under `rust/test/unit/`.

```bash
PYTHONPATH=yuos-sdk/python/src:yuos-runtime/python/src \
  python3 -m pytest -q yuos-runtime/python/src
cargo test -p yuos-runtime
```
