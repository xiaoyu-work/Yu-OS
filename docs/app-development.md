# Writing Your First Yu OS App

Start with the [system AI Quickstart](../yuos-sdk/QUICKSTART.md) to call the
user's configured model from your App. To let the CLI and the system Agent
call your App, use the MCP-only example below.

For the architectural background see
[`app-ai-integration.md`](app-ai-integration.md) (AI gate, manifest,
audit) and [`app-ai-tool-catalog.md`](app-ai-tool-catalog.md) (the fixed
tool catalog available to Apps).

## Quickstart: an MCP-only App

This example exposes `hello-mcp.say` to the CLI and the system Agent. It
echoes text without calling a model. Save these two files in `hello-mcp/`.

**`app.json`**

```json
{
  "id": "hello-mcp",
  "version": "0.1.0",
  "schema_version": 2,
  "name": {"en": "Hello MCP"},
  "mcp": {
    "entry": "server.py",
    "tools": [{
      "name": "hello-mcp.say",
      "summary": {"en": "Echo a message."},
      "args": [{"name": "message", "kind": "text", "required": true}]
    }]
  }
}
```

**`server.py`**

```python
from yuos_sdk.mcp import App, ToolResult

app = App.from_manifest()

@app.tool("hello-mcp.say")
def say(message: str) -> ToolResult:
    return ToolResult.text(message)

if __name__ == "__main__":
    app.serve()
```

From the parent directory, [sign and install](#11-ship-it) the App:

```sh
yu provenance sign --kind app --id hello-mcp --version 0.1.0 \
    --path ./hello-mcp --key "$HOME/.secrets/my-publisher.json" --entrypoint server.py
sudo yu app install ./hello-mcp
yu app hello-mcp say "Hello from MCP"
```

Run the last command as your normal user and complete any invoke approval.
It prints `Hello from MCP`. No `main.py`, `operations`, or target capabilities
are needed. A local `python3 server.py` starts the stdio loop but does not
provide an authenticated Gateway call context.

## App integration

Agent-to-App integration uses `schema_version: 2` and one
top-level `mcp` service containing lifecycle, caller access, tools, and exact
capability needs. The removed `session` block is rejected. Optional
`operations` remain the human-facing `yu app <id> <operation>` CLI surface;
they are not a second Agent tool contract.

Apps do not call other Apps. Cross-App workflows are orchestrated by the
built-in system Agent. Apps may use gated AI, controlled kernel/system
services, and shared libraries. `mcp.access` accepts only `system_agent` and
`external_agents`; an App allowlist is not supported. App-owned AI agents
cannot access App services or mint system Agent tasks. The broker enforces
this using authenticated process/session ancestry, not environment flags.

`YUOS_DATA_DIR` names only the App's own data. Normal owner launches, service
Hosts and task-host ordinary operations use the same existing owner/App
backing partition. Isolated Hosts see a private mapped path, so Apps must use
the supplied data root rather than reconstruct another App's directory or
derive it from Host HOME. Task registration must supply its Root-owned binding;
missing bindings are errors, not empty temporary stores. See
[persistent data and update behavior](updating.md#app-data-moves-into-per-app-directories).

The public App data client's Calendar day/today operations read the
authenticated owner's Calendar store through the OS service, not through the
caller's local data directory. They require the existing named
`data.db.read:calendar` capability, independently of caller identity or language.
Write authority does not substitute for read authority, and the response grants
no filesystem access. Calendar's provider, overlap, timezone and sorting
semantics are unchanged; the public SDK request/response format is unchanged.

Bundled source ownership is independent of the installed App identity.
Migrated business products live under external `products/`; shared-capability
clients `doc`, `db`, `kv`, `net` and `summarize` live under external `capabilities/`,
explicitly selected
by the [OS source lock](../packaging/apps.lock.json). This source kind is not
a manifest runtime/capability kind and grants no authority. Doc consumes a
declared Files parser library, not a Files App. Its six operations and existing
AI/memory identity remain unchanged. DB retains its five direct MCP/CLI tools,
exact database read/write scopes and existing owner/App SQLite namespace,
separate from KV and Agent memory. KV keeps its own string-map JSON namespace,
exact-key get/set/delete grants and whole-store read requirement for list/dump.
It uses only its manifest-selected MCP `server.py`, not a fabricated `main.py`;
its App-owned tests are explicitly selected as `test_server.py`. See the
[source composition contract](../packaging/README.md).
AI Helpers owns the Summarize client, not the OS AI gate. It takes explicit
text and calls SDK AI under `summarize` consent/budget, then requests bounded
summary memory through the bundled `yuos_runtime.memory.remember` export under
`self:summarize`. Other products use SDK AI directly, not this App or its
consent. App sources and their common support are external.
App `shared/python` owns `_shared`, `gateway._shared` and `canonical_argv`;
`tests/shared` owns helper tests/vectors. There is no OS `apps/` source tree.
The pinned common staging CLI installs these libraries in
`/usr/lib/yuos/python`, alongside separately owned OS SDK/runtime. Existing
Agent package assembly still delivers those files until a coordinated
independent App-package ownership transition.
Compatible App changes use versioned public SDK/MCP/wire or declared bundled
exports. Full staged payload fixtures must select the manifest entrypoint,
not private App/SDK functions or a mutable sibling source layout.

Settings permission management does not delegate target-App capabilities.
Its manifest declares only `sys.permissions` with fixed scope `manage`;
the OS service accepts any explicitly authorized App holding that High-risk
capability for the authenticated owner. Settings is one client, not an identity
exception. Default Agent/local-launcher authority does not include this
capability. The App can inspect, request restoration and revoke supported
permissions, never approve its own requests or execute the target App.
Revocation attenuates declared brokered capabilities at launch and
live broker spending. Restoration is pending until the trusted human helper
decides it, and never bypasses the signed manifest or caller's ceiling.
Dynamic/direct-resource permissions are explicitly unsupported by this first
management surface; see [installed compatibility and limits](updating.md).

System package clients use `system.package.install` / `system.package.control`
with the existing **Critical** `sys.package` capability. Authorization names an
exact package; global index/upgrade actions require explicit wildcard authority.
The OS checks the authenticated owner and session rather than requiring the
caller to be the `pkg` App. Store, terminal and other clients do not inherit this
grant by identity, and default Agent/local-launcher ceilings still deny it.
Effects apply to the system and can affect all users; owner binding is not a
per-user package sandbox. Rollback remains tied to its recorded inverse and
fresh authority. No App-to-App invocation or installation approval is implied.

Media clients use `system.media-player.control` under their own authenticated
identity, without impersonating the `yuos-player` App. Status requires
`desktop.media.observe:yuos-player`; the six playback actions separately
require `desktop.media.control:yuos-player`. The scope identifies the fixed
target, not the caller. Neither capability is in the default System Agent
authority; existing authenticated local-launch policy is unchanged, and each
calling App's permission-denial policy remains authoritative. The provider
addresses only the owner's already-running, executable-verified native Player:
no other-player selection, media URL, program launch or session-bus access is
exposed to the caller.

A staged migration is moving ordinary App calls onto `mcp.tools`, while
explicit manifest operations remain available. The human
`yu app <id> <command>` selects a declared ordinary operation first. Otherwise
it resolves exactly one tool by the `<app-id>.<command>` convention and dispatches
through the authenticated MCP gateway. Adding a distinct operation, including
an opaque stdio host, does not hide the App's MCP commands. Execution errors
never trigger a fallback between these paths. To keep CLI syntax stable, an
MCP tool arg may carry the same optional one-shot CLI
`binding` (`positional`/`flag`) as an operation arg. `binding` is CLI-only
metadata: it never appears in the model-facing MCP `inputSchema`, and the
default binding is unchanged (`bool` → flag, everything else → positional).
MCP-only commands reject raw `--stdin`. Use `yu app <id> <command> --args-stdin`
to send one JSON object of manifest-declared business arguments instead of
ordinary tool arguments. The marker must be the only tool argument; malformed,
empty, non-object, unknown-field, and oversized input is rejected before a tool
can run. Defaults, required fields and choices use the same manifest validation
as argv. No caller/session/context authority comes from the object.
Input is at most 1008 KiB of serialized JSON (including escapes and base64), or
a lower `YUOS_APP_STDIN_MAX_BYTES`. This leaves 16 KiB for the unchanged 1 MiB
broker request envelope. Canonical arguments must also fit this budget.

## 1. What is a Yu OS app?

A Yu OS App ships `app.json` and the entrypoints for its declared surfaces.
An MCP-only App needs just its service entrypoint; a human-only one-shot
App does not need an MCP service. An App with both could look like this:

```
my-app/
├── app.json        ← required manifest
├── main.py         ← default Python entry point
├── server.py       ← authenticated MCP App service
└── assets/         ← optional app-owned files
```

The manifest's `runtime` selects how the entry point is launched. Current
values are `python`, `node`, `shell`, and `binary`; the default is `python`.

Apps are operation/tool packages, not general Agent lifecycle plugins. Use the
separate signed, out-of-process
[Agent extension ABI](extension-abi.md) for session/model/tool observations.
That ABI cannot mutate prompts or authorization and does not trust an
installed directory without [package provenance](extension-provenance.md).

The `yu` CLI recursively scans `$YUOS_APPS_DIR` (default
`/usr/lib/yuos/apps/`) for valid `app.json` files, then exposes each operation
as `yu app <id> <op>` ([`core/src/apps.rs`](../core/src/apps.rs)).

The manifest id must equal the app directory's **normalized path relative to
`$YUOS_APPS_DIR`**, with path segments joined by `-`. For example,
`gateway/discord/app.json` declares `id: "gateway-discord"`. An app whose
path and id disagree is skipped during discovery.

The id itself has to match `[a-z][a-z0-9_-]*`
([`manifest.rs:935`](../core/src/caps/manifest.rs)) — start with a
lowercase letter, then lowercase letters, digits, `_`, or `-`.

## 2. Developing against the SDK

The Yu OS Agent package supplies the Python SDK at `/usr/lib/yuos/python/`.
On a development machine, install the [Python wheel](../yuos-sdk/python/README.md#install),
or use `python3 -m pip install -e yuos-sdk/python/` from the root of a Yu OS
source checkout in your Python environment. You do not need the first-party
`yuos_runtime` package. Use the SDK's pure helpers and test business logic
locally; run live App calls on a system with the matching Yu OS runtime.

For Node, Go, and Rust, use the [language SDK guides](../yuos-sdk/REFERENCE.md#language-support).
Ship the dependencies or compiled binaries needed by your declared entries
before signing the payload.

## 3. The minimum viable app

Manifest for a human-only one-shot App (`runtime` defaults to `python`).
For Agent-callable tools, use the [MCP-only quickstart](#quickstart-an-mcp-only-app)
above instead of duplicating a command:

```jsonc
{
  "id": "hello",
  "version": "0.1.0",
  "name": { "en": "Hello" },
  "runtime": "python",
  "operations": {
    "say": {
      "label": { "en": "Say something" },
      "args": [
        {
          "name": "message",
          "kind": "text",
          "required": true
        }
      ]
    }
  }
}
```

Only a human-only App may stop at `id`, `version`, and `name.en`
([`manifest.rs:613-621`](../core/src/caps/manifest.rs)). Every
`name`/`label`/`summary`/`why` field is a localised map; English is
required, other locales (`zh-CN`, …) are optional fallbacks
([`i18n/text.rs:42-90`](../core/src/i18n/text.rs)).

Entry point:

```python
# main.py
def run(command, args):
    return {"command": command, "args": args, "msg": "hello"}
```

The bridge calls `mod.run(command, args)` exactly once per invocation
([`bridge.rs:64`](../core/src/bridge.rs)). Two crucial details:

* `command` is a **string** — the manifest-declared operation name, such as
  `"say"`. The kernel rejects undeclared operations before starting the app.
* `args` is a **list of strings**, not a dict — the bridge validates every
  declared positional and `--flag` value, rejects undeclared flags, and then
  passes the effective argv (including manifest defaults) to the handler. Apps
  parse the already validated values with App-owned
  [`canonical_argv`](https://github.com/xiaoyu-work/yuos-app/blob/main/shared/python/canonical_argv.py),
  preserving inline flag values and the `--` delimiter.

Both operation and MCP tool arguments use a manifest `kind`: `path`, `host`,
`name`, `text`, `number`, `integer`, or `bool`. They do not accept arbitrary
nested JSON Schema. `number` accepts decimals and `integer` rejects
fractional input. For CLI calls, `binding` is either `positional` or `flag`;
by default booleans use flags and other kinds are positional:

```jsonc
{
  "name": "timeout",
  "kind": "integer",
  "binding": "flag",
  "required": false,
  "default": 30
}
```

Use `--` to end flag parsing when a positional value itself begins with `--`.
Any supplied number, integer, or explicit boolean literal that does not match
its declared kind is rejected before launch. Missing required booleans are
rejected before optional booleans are materialized as `false`. A destructive
confirmation uses `"required": true, "choices": [true]`, so omission and
`--confirm=false` both fail before capability resolution.
For a mode-specific confirmation, use `required_when` with the same
`arg-present`, `arg-equals`, or `arg-not-equals` condition model as capability
needs. The argument is accepted exactly when the condition applies and is then
required. The condition must reference an earlier argument; conditionally
required arguments cannot be repeatable or also declare `required: true`,
or defaults. A conditional confirmation still declares
`choices: [true]`.

Use `choices` for a closed scalar enum. Set `repeatable: true` when every
occurrence is meaningful: the bound value becomes an ordered JSON array and a
flag is emitted once per item. Repeatable positional arguments must be the
last positional declaration. Repeatable booleans are rejected.

Every argument has one canonical name and one binding. `aliases`,
`positional_alias`, `default_from`, and `trusted_resolver` are invalid
manifest fields. Callers provide context-dependent values explicitly.

An optional argument may declare a non-null literal `default` matching its
`kind`. Omit `default` when there is no default; explicit JSON `null` is
invalid. Defaults are literals only; the runtime never derives one argument
from another argument, credentials, environment, or host configuration.
Defaulted arguments must be optional; defaulted positional arguments follow
all required positional arguments. Defaulted positionals cannot be mixed with
optional positional slots that have no default because argv cannot represent
those gaps consistently. The bridge resolves paths before capability
derivation and materializes defaults using their declared binding: positional
values remain positional, non-boolean flags become `--name value` (or
`--name=value` when the value starts with `--`), and a true boolean flag
becomes `--name`. An explicitly supplied false flag becomes `--name=false`;
only an omitted/default-false flag is omitted. Positional booleans are
serialized as `true` or `false`. Flag defaults are placed before an
end-of-options delimiter; positional defaults follow supplied positional
values. The handler must consume that canonical argv rather than recompute a
separate default.

MCP tools receive a JSON object rather than argv. Before capability
resolution and MCP forwarding, the kernel inserts every declared literal
default and omitted boolean into that object, validates choices and repeatable
arrays, and normalizes path values. One shared effective-call resolver feeds
the in-process gate, daemon gate, and forwarded tool arguments.

Provider, host, destination, and server selection are explicit typed
arguments. Conditional needs bind fixed or caller-supplied scopes only after
the selected provider/mode condition applies.

An operation may set `stdin: true` to receive explicitly forwarded caller
input. The top-level CLI opts in with `--stdin` only for operations declaring
that support. This switch is
recognized only in an App operation's pre-`--` option region, so command-owned
`--stdin` flags elsewhere remain untouched. The CLI streams at most 16 MiB
(configurable with `YUOS_APP_STDIN_MAX_BYTES`) and fails before launch on
overflow. The bridge never inherits or probes process stdin. Agent, MCP
service, and ordinary CLI calls therefore keep child stdin closed. Python list
handlers use App-owned common support's `canonical_argv` module from that
same installed Python root;
argparse and gateway parsers consume the same inline flags and `--` delimiter
directly.

An explicitly declared opaque host instead uses
`yu app stdio <id> <operation> [args...]`. It requires that operation's
`stdin: true` and a held, authenticated primary entry. This human/host-only,
non-model-callable frontend forwards incremental bytes rather than collecting
one-shot input; it does not parse or reformat stdout or append a newline.
Diagnostics and OS review presentation use stderr. App arguments resembling
format flags remain App arguments, and `--` ends metadata-option interpretation.
Actual streaming execution refuses `--wire=1`; help/schema output remains
ordinary CLI metadata. Cancellation and the existing operation deadline after
input EOF retire the worker and session.

The opaque host uses the existing authorization, owner review, App denial,
package-binding and sandbox mechanisms. It cannot choose an arbitrary program
or obtain another operation's capabilities. Ordinary captured Python operations
still require `main.py`; the declared-stdio path and MCP may use their own
verified entries. Do not add a captured-operation fallback for other primary
Python filenames.

### Filesystem MCP contract

The bundled `fs` App is MCP-only: all 14 commands are declared once under
[`products/files/apps/fs/app.json`](https://github.com/xiaoyu-work/yuos-app/blob/main/products/files/apps/fs/app.json)'s `mcp.tools`, with direct SDK
handlers in `server.py` and typed behavior in `main.py`. Human CLI calls use
the same tools:

```bash
yu app fs write /workspace/out.txt --content='hello'
yu app fs write /workspace/empty.txt --content=
yu app fs write_bytes /workspace/out.bin --content=AAEC
printf '%s' '{"path":"/workspace/out.txt","content":"hello\u0000world"}' | yu app fs write --args-stdin
yu app fs read /workspace/out.txt --offset=0 --limit=1000
yu app fs read /workspace/out.txt --start=1 --end=20
yu app fs tag /workspace/out.txt reviewed blue
```

`write` and `write_bytes` require explicit string `content`; neither reads
raw stdin, and `--stdin` is rejected. Empty content is valid. Binary content is
strict base64 (no ignored whitespace or invalid characters). Small human CLI
writes may use argv; use `--args-stdin` for larger content or embedded NUL.
Rust runtime writes always send `{path, content}` as bounded JSON over stdin,
never content in argv. The App still receives only explicit MCP arguments.

Byte offsets are nonnegative integers and limits are positive integers,
capped at 1,000,000 bytes for text and 4 MiB for binary reads. Line ranges are
inclusive, 1-based, and capped at 1,000,000 bytes of UTF-8 output while still counting
the whole file. `end` requires `start` and cannot precede it. Line mode cannot
be combined with a nonzero byte offset or a nondefault byte limit. `recent`
accepts a nonnegative count, skips hidden entries and symlinks, and defaults
to ten regular files under `/workspace`.
Binary and line caps are lower than the legacy operation limits so the
duplicated MCP text/structured response fits the 16 MiB MCP frame and the
text result fits the 8 MiB Host frame. Continue larger reads with byte offsets
or narrower line ranges.
The Rust `yuos_runtime::fs::read_bytes` convenience wrapper rejects a truncated
response rather than returning a prefix as a complete file. Text `read` retains
its explicit `ReadResult.truncated` field.

Paths are resolved before capability checks. Recursive copies reject
symlinks and special files rather than materializing out-of-scope targets.
Listings do not follow child symlinks when reporting directory status.
Tags belong to regular files and merge into the sibling `.yuos-meta.json`;
`stat` reads those tags. Because the scope vocabulary has no sibling transform,
both declare read authority on the parent directory, while `tag` also declares
parent write authority and file metadata authority. Missing sidecars mean no
tags; corrupt, unreadable or symlinked sidecars fail rather than being ignored
or overwritten. Search requires ripgrep: missing binaries, timeouts and
non-match-related failures are errors, not filename-only fallback success.
All filesystem failures propagate as MCP tool errors.

Mutations snapshot before changing their targets, including tag sidecars and
new directories. The server supplies the authenticated call's session id,
never the persistent service's environment or a tool argument; calls without
a session do not snapshot into an unrelated service session.

The return value (a dict, list, or scalar) is JSON-dumped to stdout.
Return `None` to print nothing.

### Agent-launched isolation

An App invoked from a daemon-backed Agent task does not run in root `yuosd` or
inside the `yuos-agentd` model/tool process. `yuosd` starts a task
`yuos-extension-host` under an exclusively leased execution UID, distinct from
the task owner. The worker uses its versioned, purpose-bound control socket;
MCP App calls are relayed to the root-managed owner/App service Host.

The host starts with no supplementary groups, `NoNewPrivs`, a `0077` umask,
sanitized environment and descriptors, finite resource limits, mandatory
cgroup containment and its own session/process group. App children receive a separate private broker proxy
path, never `/run/yuos/yuosd.sock`. The proxy accepts only routes for the
child's nearest registered App/MCP session, then re-enters the normal
capability authority and final provider checks. Session registration,
per-call transient scopes, approvals, and teardown therefore retain the same
manifest-derived policy as direct launches.

Do not depend on inherited file descriptors, arbitrary parent environment
variables, the broker socket pathname, or daemonized children. Calls and
frames are bounded. Task completion retires its task Host; shared App service
Hosts follow their separately authenticated lifecycle and per-call authority.
App stdout/stderr and returned values are treated as untrusted Agent input.
See the [Host contract](../core/src/extension_host/MODULE.md).

### App-owned object references

Apps may optionally declare an `objects` map that binds named object types to
public App commands. An explicit operation takes precedence; otherwise the
exact `<app-id>.<command>` MCP tool supplies the resolver's arguments.
The shared OS catalogue authenticates these declarations
without executing the App; explicit resolution still uses the ordinary App
permission and runtime path. Public SDK helpers format portable references,
and Activities can attach them without copying or reading the App's data.
See [App-owned objects](app-objects.md) for the manifest contract, canonical
URI format, examples, and the distinction between a verified declaration and
an existing, readable object.

### Expected operation effects

An operation or MCP tool may optionally declare bounded `effects` metadata. The shared OS
preview service displays authenticated declarations and requested targets
without executing the App, reading object data, or granting permissions.
Missing metadata remains unknown. See [operation previews](operation-previews.md)
for the declaration vocabulary and its limits.

## 4. The one-shot dev loop

Every one-shot `yu app <id> <op>` invocation launches the app entry point
through the runtime selected by its manifest. Python apps run in a fresh
subprocess and are imported from disk for every call, so there is no module
cache to restart. Do not edit a signed installed App in place. Sign and
install an updated package when changing its contents.

Use an App source directory for metadata discovery and static validation:

```sh
mkdir -p ~/my-apps/hello
$EDITOR ~/my-apps/hello/app.json    # paste the manifest from §3
$EDITOR ~/my-apps/hello/main.py     # paste the run() from §3

YUOS_APPS_DIR="$HOME/my-apps" yu app
YUOS_APPS_DIR="$HOME/my-apps" yu app lint hello
```

Discovery does not authorize execution. See [Ship it](#11-ship-it) for
the checked signing and installation flow.

Or work from a copy of a bundled app as a starting point:

```sh
cp -r /usr/lib/yuos/apps/notify ~/my-apps/my-notify
# rename the id in app.json AND rename the directory to match
```

## 5. Capability gating

Any operation that touches a controlled resource — files, network,
secrets, AI, the user's screen — has to declare it in the manifest
**and** check it at runtime.

Declare it in `operations.<op>.needs[]`:

```jsonc
"send": {
  "label": { "en": "Send a notification" },
  "needs": [
    {
      "verb": "ui.notify",
      "scope": { "kind": "wild" },
      "why": { "en": "Display a notification on your screen." }
    }
  ]
}
```

`scope.kind` is one of (kebab-case;
[`manifest.rs:462`](../core/src/caps/manifest.rs)):

* `"wild"` — anything matches. Use for verbs without natural scope
  (`ui.notify`, `time.delay`).
* `"fixed"` — hard-code a scope: `{"kind": "fixed", "scope": {...}}`.
  Useful for ops that always touch the same resource.
* `"from-arg"` — late binding: `{"kind": "from-arg", "arg": "path"}`
  reads the named op argument and constructs the scope from it. Without a
  transform it works for `path` / `host` / `name`. The optional safe
  `transform` is `parent` for a path's containing directory or `url-host`
  for the exact host and effective port parsed from a text URL. `url-host`
  resolves HTTP and HTTPS defaults to ports 80 and 443, preserves explicit
  ports and bracketed IPv6, and rejects schemes without a known or explicit
  port. App-side URL checks must derive the identical scope; HTTP clients use
  `_shared.safe_http.host_scope`, including for every redirect hop. The shared
  App-owned `tests/shared/vectors/url_host_scope_vectors.json` corpus locks Rust and Python behavior
  for UTS-46 ignored/mapped/contextual/rejected input, IDNA/punycode domains,
  legacy IPv4 forms, IPv6 compression, and ports. Production images provide
  `idna >= 3.3, < 4`; missing or unsupported versions fail closed. Core's
  `url 2.5.8` parser materializes caller URLs as canonical ASCII effective
  arguments before capability derivation and child dispatch. Python therefore
  never reinterprets caller Unicode on the initial request; it applies UTS-46
  itself only when canonicalizing redirect locations before requesting their
  authority. Rust browser/JavaScript consumers use
  `obscura_net::effective_host_scope` so every initial and redirect policy
  check includes the same effective port and bracketed IPv6 form.
* `"from-arg-map"` — map explicit argument values to predefined scopes:
  `{"kind": "from-arg-map", "arg": "mode", "values": {...}}`.
* `"from-arg-or-wild"` — derive a scope from an argument normally, but use a
  wildcard when the separately declared boolean argument named by `wild_when`
  is `true`:
  `{"kind": "from-arg-or-wild", "arg": "target", "wild_when": "all"}`.

Needs that apply only in one mode declare `when` explicitly:

```jsonc
{
  "verb": "fs.read",
  "scope": { "kind": "from-arg", "arg": "file" },
  "when": { "kind": "arg-present", "arg": "file" },
  "why": { "en": "Read the optional file when one was supplied." }
}
```

`arg-equals` gates provider/mode-specific fixed needs:
`{"kind":"arg-equals","arg":"provider","value":"google"}`. An inactive
condition omits only that declared need. Once active, missing arguments and
unmapped `from-arg-map` values remain errors. An optional scope argument needs
a condition checking that argument, or a `when` exactly matching the argument's
`required_when` (same kind, referenced argument and comparison value).
The matching condition makes the argument mandatory before capability
resolution; an inactive condition yields no capability. Conditional booleans
have no implicit `false` default. Unconditional optional bindings and mismatched
conditions are rejected at manifest load time. `arg-not-equals` provides the
inverse comparison when one mode must omit authority.

Check it at runtime by importing the internal runtime:

```python
from yuos_runtime import policy

def run(command, args):
    try:
        policy.require("ui.notify", wild=True)
    except policy.PermissionDenied as denied:
        return {"error": str(denied), "denial": denied.denial}
    except policy.PolicyUnavailable as exc:
        return {"error": f"capability check failed: {exc}"}
    ...
```

`policy.require(verb, *, path=..., host=..., name=..., self_ref=..., wild=False)`
uses the hidden kernel policy bridge and raises `PermissionDenied` on deny
([`yuos-runtime/python/src/yuos_runtime/policy.py:79-106`](../yuos-runtime/python/src/yuos_runtime/policy.py)).
Pass exactly one scope keyword (or `wild=True` for unscoped verbs).

Use `policy.check(...)` (same signature, returns the raw decision
envelope) when you want to surface "would-be-denied" without aborting.

### Agent-resumable authorization

A bundled App that needs Google or Microsoft user authorization should return
an `auth_required` error with a constrained Agent action:

```python
{
    "error": "Gmail authorization is required",
    "auth_required": True,
    "retryable": False,
    "setup": {
        "interactive_oauth_available": True,
        "agent_action": {
            "tool": "yu_oauth_login",
            "input": {"provider": "google"},
        },
        "login_command": "yu credential oauth-login google",
    },
}
```

Only `google` and `microsoft` are supported. In an attended local Agent session,
the system Agent starts the trusted browser flow in the default credential
namespace and retries the original operation after authorization. Keep the CLI
command as a human fallback, and never include client secrets, access tokens,
refresh tokens, authorization codes, or browser state in App output.

## 6. Manifest-derived schema and help

`yu app <id> --schema` and `yu app <id> <op> --schema` generate their
output **only from `app.json`**. Schema inspection never imports or executes
the app entry point, so listing an untrusted app remains side-effect free.

Describe every operation in the manifest:

```jsonc
"say": {
  "label": { "en": "Say something" },
  "summary": { "en": "Return the supplied message." },
  "args": [
    {
      "name": "message",
      "kind": "text",
      "required": true,
      "label": { "en": "Message to return." }
    }
  ]
}
```

The manifest is the only maintained operation and argument contract. The entry
point implements behavior for those declared operations; it must not define
`_schema()`, handle `__schema__`, or maintain a parallel parameter list.
Static app tests compare manifest operations with dispatcher branches, require
parser flags to use `binding: "flag"`, and compare argparse required/default/
integer/repeatable/choice behavior without importing entrypoints. Optional
positional arguments cannot precede required positionals, and repeatable
positionals must be last. Fixed path scopes use absolute paths or
`~/...`; `$HOME`, `$XDG_DATA_HOME`, and other environment placeholders are
rejected because capability matching does not expand them.

## 7. AI features

Use the SDK's AI gate rather than a model-provider SDK. From an App with
approved access to the supplied file and `ai.chat.untrusted`:

```python
from dataclasses import asdict
from pathlib import Path

from yuos_sdk import ai

def run(command, args):
    body = Path(args[0]).read_text(encoding="utf-8")
    resp = ai.chat(
        prompt=f"Summarise:\n{body}",
        origin="external-content",
        max_units=2000,
    )
    return {"summary": resp.text, "usage": asdict(resp.usage)}
```

Always pass `origin` to chat: `trusted` for developer-authored text only,
`user-input` for text typed by the user, and `external-content` for files,
email, web pages, or output from another agent. If inputs mix sources,
declare the outside content rather than labeling the whole prompt `trusted`.
`external-content` selects `ai.chat.untrusted`; the effective policy
determines redaction. The label does not prove the input is safe.

In `app.json`, list what your App needs to do, for example
`"needs": [{"verb": "ai.chat"}]`. That line requests chat permission.
In code, call `ai.chat` with the actual text. The user picks the model in
Yu OS settings, so the App does not need `scope`, `value`, or a model ID.
The [SDK model table](../yuos-sdk/REFERENCE.md#ai-support) lists the supported
verbs, including `ai.chat.untrusted` for `external-content`.

The same declaration works in an operation, MCP tool, or GUI entry. Keep the
`ai` policy in the manifest: consent and budget checks still apply. Older
manifests that declare an explicit scope remain supported. Other permissions
still need a scope to say which resource the App wants, such as a file or host.

| Calling surface | Owner consent |
|---|---|
| An operation, MCP tool, or GUI call declaring AI in required `needs` | The owner grants separate consent to the App's AI policy with `yu app consent grant <id>`. Changes to the policy require renewed consent. |
| An eligible GUI entry using [optional model permissions](#model-permissions-in-settings) | The owner explicitly allows the App to use that model in Settings → Applications. The Store can also offer optional text AI during installation. Neither path requires a second `consent grant` for the selected GUI entry. |

Both paths still require the matching capability and are subject to budgeting
and the effective policy. The optional GUI choice does not flow to other
surfaces. A direct Root directory install is not an ordinary user's Store
AI opt-in.

For the full AI integration model — `origin`, safety profiles, tool
catalog, budget envelope, audit surface — read
[`app-ai-integration.md`](app-ai-integration.md).

## 8. Developer commands

The `yu` CLI ships with everything you need to introspect, validate,
and ship an app
([`core/src/router.rs:107-187`](../core/src/router.rs)).

| Command | Effect |
|---|---|
| `yu app` | List every discovered app under `$YUOS_APPS_DIR`. |
| `yu app <id>` | Show ops + version for one app. |
| `yu app <id> <op> [args…]` | Run an op. |
| `yu app <id> --schema` | Full manifest-derived schema for the app. |
| `yu app <id> <op> --schema` | Schema for one op. |
| `yu app lint [<id>]` | Check declared MCP entrypoints and look for blocked model-provider imports in Python files. Run on every discovered App if no ID is given. |
| `yu app tool list [<id>]` | Show the manifest-declared MCP tools this App exposes through the private Gateway. |
| `yu app install <dir> --review` | Verify the package and return its requested permissions, scopes, conditions, AI policy and execution surfaces as JSON, without installing or granting anything. |
| `sudo yu app install <dir> [--force] [--no-consent] [--yes]` | Ask Root to publish a signed App into its configured App root after protected review. `--yes` cannot replace first review. `--no-consent` defers the separate legacy AI prompt. Installing an unsigned directory with `--dev-trust` is not supported. |
| `yu review pending` | List and present the current user's App reviews and existing capability requests using the shared OS review contract. |
| `yu review show <id>` | Display one owner-scoped review and its authenticated permission disclosure. |
| `yu review approve <id>` | Confirm an App package and choose any newly requested, supported GUI permissions, or decide a separate capability request through the interactive OS helper. |
| `yu review deny <id>` | Cancel the current user's pending review through the owner-scoped OS route, without a privilege prompt or capability grant. |
| `yu app consent list` | Which apps you have granted AI consent to. |
| `yu app consent show <id>` | Display the manifest's AI block. |
| `yu app consent grant <id> [--yes]` | Grant AI consent. |
| `yu app consent revoke <id>` | Revoke it. |
| `yu app consent path <id>` | Print the consent record path. |

### Installation permission review

The OS builds the review from the authenticated manifest, covering operations,
MCP tools and the declared GUI surfaces. Capability names, explanations and
risk levels come from the OS catalog; the App's explanation is shown separately
as its reason for requesting access. Conditional requests are disclosed even
when the corresponding feature is not currently used. Argument-derived scopes
remain unresolved until invocation: listing a file or host parameter is not
permission to access every file or host.

The review also lists the declared desktop surface, MCP lifecycle and external
Agent access, and the AI policy. Its `contract_digest` describes the
permission-related contract, including argument constraints, conditions and
execution surfaces. It excludes the App version and capability-purpose text,
so changing release numbers or explanations alone does not imply a permission
change. The digest is a comparison key, **not an approval token or a grant**.

The same install screen asks **Always allow** or **Don't allow** for each
new supported GUI permission. The choice is separate from the package
confirmation and remains saved until changed in Settings. An unchanged choice
for the same publisher is carried forward; broader scopes or new uses need
their own choice. Optional denied features stay unavailable, and a required
denial prevents that launch. Argument-bound access, selected file contents,
and Agent-run operations do not inherit standing GUI permission. The optional
system AI choice described below is bound to the exact package and model, not
carried forward like ordinary GUI choices.

Exact App builds selected by the OS release use its
[preinstalled defaults](../packaging/README.md#preinstalled-app-defaults)
on direct local launches. Apps cannot opt themselves in through a declaration.

Signed global directory installation requires Root and creates a protected
Root-owner OS request before publishing, including `--force`, `--yes`,
`--no-consent` and in-place installs. `--review` remains nonmutating and does
not require installation authority.
A terminal or native desktop presenter sends the displayed decision through
`yuos-approval-helper --system-review-json`, authenticated by polkit. Without an
interactive terminal or a previous matching OS decision, installation returns
an explicit pending-review error and does not replace the existing App.
`--yes` never substitutes for first review. Automation may reuse a previously
consumed decision only for the same owner, App, publisher, trust tier and
permission-related contract under the current revocation generation.

Review clients authenticate the Unix broker peer as root before sending
requests. A configured user-owned socket, returned JSON, request ID or contract
digest cannot become approval authority. The Root directory controller consumes
confirmation once against the freshly verified staged package, inside the same
per-App fence and checked ordinary/GUI/MCP cross-owner retirement used by the
official Store. A consumed confirmation records the user's
decision, not proof that a subsequent filesystem publication succeeded.
`permission_review.permissions_granted` remains `false`: disclosure does not
issue a grant. The direct GUI still receives a bounded runtime lease through
normal signature, capability, sandbox and policy checks.

The installer re-verifies the package after review and rejects a changed
package before replacing the old installation. Cancelling review leaves the
previous version untouched. Manifest disclosure is defined in
[`apps/permission_review.rs`](../core/src/apps/permission_review.rs);
the terminal and desktop share the typed
[`SystemReview` contract](../crates/yuosd-client/MODULE.md).

The reusable [directory-install backend](../core/src/apps/installation.rs)
owns source validation, private staging, in-place revalidation and filesystem
replacement/recovery. It and `yu app lint` use the same
[static lint implementation](../core/src/apps/lint.rs), without executing App
entrypoints. CLI flags, OS review presentation and AI consent remain in the
router; actual signed publication uses the closed Root-only
`app.directory_install` route. Root selects its configured App root and verified
App ID, retaining existing normalized nested layouts. A different CLI-local
`YUOS_APPS_DIR`, an unavailable daemon or unconfirmed cleanup is an explicit
error, never permission to write directly. The controller outlives client
disconnection and durably blocks App admission after an uncertain result.
Agent-only installations need no preinstalled App package: Root may initialize
the configured empty App-root leaf beneath its existing verified Root-owned
parent. This creates no App payload and never uses a caller-selected target.
Root records publication dispatch durably before any installation tree writes.
After dispatch or an indeterminate outcome, do not repeat installation, even
with the same package or `--force`. Run `yu app install --recover <app-id>` for
a read-only outcome check. It requires the exact installed package/manifest,
original destination, unchanged directory delivery ownership and no transaction
scratch before reopening admission. It does not need the original source and
never copies, renames or consumes confirmation. Unconfirmed bytes/scratch remain
blocked; this command does not repair or roll back them. Only cleanup known to
precede publication may retry checked retirement with current confirmation.
Directory installation refuses dpkg-owned
payloads: update those through APT. Official Store installation remains APT-only.

Mutating `--dev-trust` installs are currently blocked without writing files or
trust grants. A Root-owned unsigned tree in the installed vendor root, or one
matching an old developer grant, cannot yet be guaranteed quarantined before a
new independent trust decision. The existing standalone developer-trust
terminal/phrase/no-active-session gate is unchanged; no Root request flag
creates trust or runtime permission. Public signed App rollback reuses the same
Root directory controller and protected review, while trust-mutation
coordination remains separate work. See
[provenance rollback](extension-provenance.md#operator) for its retained-artifact,
installed-destination and APT restrictions.

Brokered operation/GUI/native registration and prepared MCP calls require
owner review before activation unless this owner already confirmed the package
or the OS admitted a built-in desktop entry. Confirmation by UID 0 for a manual
directory installation does not make choices for other owners; they review
the App on first use. Unreviewed background services are not warmed
automatically. Existing App permission denials remain in effect after
installation confirmation.

Both presenters use the same catalog labels, purposes, resource scopes,
argument constraints, supported choices and state transitions. The broker
assigns a protected monotonic revision to each displayed snapshot. Decisions
carry that exact revision, not a subsequently fetched one; the controller
rechecks it, the owner, the action and each selection before applying the
existing authority. A stale view or a competing decision cannot grant access.
Owner cancellation uses the same typed decision without a privilege prompt.
The helper reads at most 64 KiB, accepts no caller-supplied owner or grant in
the decision, and derives the authenticated user from polkit.

Package confirmation by itself carries no permission choices. When the OS
offers lasting GUI choices, the presenter submits them separately from the
confirmation for the same displayed review. Existing capability requests still
offer only durations their authority supports, with use/time limits disclosed.
Legacy App-policy restoration removes a deny gate but does not supply launcher
or manifest authority. Unsupported Ask policies and direct-resource controls
have an explanation rather than a working-looking switch.

Older `permission.*` clients and the legacy capability-helper mode remain
compatible; other Settings and Agent Web approval callers still need the same
presentation cutover. Store/package-channel integration, generic local-launch
cutover and complete per-permission allow/ask/deny resource enforcement also
remain separate rollout work. Native component builds do not establish
interactive Wayland or complete installed-system acceptance.

## 9. Environment variables

Set explicitly only when overriding defaults
([`bridge.rs:28-53`](../core/src/bridge.rs),
[`router.rs:25`](../core/src/router.rs),
[`yuos_sdk/ai.py:525`](../yuos-sdk/python/src/yuos_sdk/ai.py)).

| Variable | Purpose | Default |
|---|---|---|
| `YUOS_APPS_DIR` | Apps root the kernel scans. | `/usr/lib/yuos/apps` |
| `YUOS_DATA_DIR` | Where the current process persists Yu OS data. Inside a sandboxed operation this is the app's own partition, `<data-root>/apps/<app-id>` — never the owner's data root. | `$XDG_DATA_HOME/yuos`, normally `~/.local/share/yuos`; yuosd overrides this to `/var/lib/yuos` |
| `YUOS_OWNER_HOME` | Verified home path of the user the App service represents. Use it only to resolve a manifest-approved `~/...` resource; the path grants no access by itself, and `$HOME` remains private App state. | (auto) |
| `YUOS_APPLICATIONS_DIR` | Where `yu app install` writes generated desktop launchers. | `/usr/share/applications` |
| `YUOS_SDK_PYTHON_DIR` | SDK import search directory for the one-shot Python bridge. It must contain the package the entrypoint imports; third-party Apps only need `yuos_sdk/`. | unset → bridge checks its installed and development paths |
| `YUOS_APP_ID` | The id of the calling app. **Auto-set by the bridge** from `app.json`; do not override. | (auto) |
| `YUOS_KERNEL_BIN` | Override the public SDK's default CLI executable for development. Does not select caller identity; explicit installed-binary clients do not use it. | `yu` (from `$PATH`) |
| `YUOS_SESSION` | Session id for grouped multi-call audit. | unset |
| `YUOS_WORKER_SANDBOX` | Set to `1` inside every sandboxed operation. Read it to detect the sandbox; do not rely on it for security. | (auto) |
| `YUOS_EGRESS_SOCKET` | HTTP `CONNECT` proxy socket, present only when the operation was granted exact `net.dial` hosts. | unset |
| `YUOS_EGRESS_ENDPOINTS` | Comma-separated `host:port` list the egress broker admits. | unset |

Everything else is cleared. An operation starts from an empty
environment and receives only the variables above plus a fixed `PATH`,
`HOME`, `TMPDIR` and locale — never the launcher's environment, never a
provider key, never `YUOS_PROC_DATA_DIR`, `YUOS_CAPS_DATA_DIR`,
`SSH_AUTH_SOCK`, `DISPLAY`, `WAYLAND_DISPLAY` or a session bus address.

## 10. What an operation can reach

Operations run inside a namespace/seccomp/cgroup sandbox
([`core/src/worker/MODULE.md`](../core/src/worker/MODULE.md)). Plan for
this while writing the manifest:

* **Filesystem.** Only the app package (read-only), the app data
  directory (read-write), the `yu` binary and SDK trees (read-only),
  and the paths the operation's `needs[]` actually granted. A
  `fs.read` grant is a read-only mount, `fs.write` and `fs.delete` are
  read-write, and a `from-arg` scope is mounted at the same absolute
  path the argument named. A wildcard scope maps to no mount at all: the
  capability check passes and the path is still not there. Never assume
  `$HOME` is the user's home — inside the sandbox it is the app's own
  data directory.
* **App data.** `YUOS_DATA_DIR` is the app's own partition of the owner's
  data root, `<data-root>/apps/<app-id>`. Neither the root nor another
  app's partition is mounted, so two apps cannot read each other's
  files even though they run as the same user. State a bundled app
  wrote to the data root before isolation is moved into that partition
  once, automatically, before its first sandboxed run
  ([`docs/updating.md`](updating.md) covers the cases that stop and ask).
* **Agent memory.** `yuos_runtime.memory` still writes into the owner's
  cross-app agent memory; the database is not mounted, and the call is
  carried out by the launcher after checking `memory.write` scoped
  `self:<your-app-id>`. Writing to another app's source is refused, and
  there is no file to open directly. `show` and `forget --row` answer
  for another app's row exactly as they answer for a row that does not
  exist, so the row id space says nothing about what other apps store.
* **Network.** Denied by default, in a private network namespace, and
  the worker seccomp filter allows only `AF_UNIX` sockets — an
  `AF_INET` socket cannot even be created, so there is no direct dial
  to fall back to. An operation that declares `net.dial` with an exact
  `host[:port]` scope receives a Unix-domain egress socket at
  `YUOS_EGRESS_SOCKET`; `yuos_runtime.egress.create_connection()` opens a
  tunnel through it, and the broker admits only those endpoints, pins
  the address it resolved itself, and refuses loopback, link-local,
  cloud-metadata, private and CGNAT addresses. TLS is established by the
  caller over the returned stream, against the hostname it asked for. A
  wildcard host scope grants nothing, because there is no identity to
  check against. `net.probe` is deliberately different: it authorizes a
  bounded, daemon-mediated reachability check for one exact endpoint and
  never adds an egress socket or IP networking to the App sandbox.
* **Filesystem globs.** `*` matches one segment and is expanded entry
  by entry, so `Documents/*` exposes the entries in `Documents` and not
  their children. `**` covers a subtree and is refused outright when
  that subtree contains a credential store, so `$HOME/**` does not
  quietly hand over `~/.ssh`. A write scope must name an exact path or a
  `**` subtree.
* **Processes.** Bounded process count, open files, file size, memory,
  CPU, wall clock and captured output. A worker cannot daemonize: the
  whole process group and cgroup are killed when the operation ends, is
  cancelled or times out.
* **Authority.** `yuos_runtime.policy` and every `yu` call the app makes
  go through a private broker endpoint bound to that one launch, which
  relays them to `yuosd` under a launcher-held grant. `yuosd` still
  decodes, authorizes and spends the exact capability against the live
  App workload grant, so the endpoint can only narrow what the app may do. An
  app cannot register an identity, widen its own capabilities, answer
  its own consent prompts, or reach the real broker socket.
* **GUI.** Display and GPU transports are granted only to a
  `desktop.exec` launch, never to a headless operation.

### Multiple GUI entries under one App identity

The optional desktop contract can preserve separate native executables and
render graphs without creating another installed App identity:

```json
{
  "id": "workbench",
  "version": "1.0.0",
  "name": {"en": "Workbench"},
  "runtime": "binary",
  "entry": "bin/command",
  "desktop": {
    "exec": "--gui",
    "entry": "bin/window",
    "needs": [],
    "entries": {
      "applet": {
        "entry": "bin/widget",
        "exec": "open",
        "name": {"en": "Workbench widget"},
        "needs": []
      }
    }
  }
}
```

`yu app workbench --gui` selects the default GUI. Its exact existing
`desktop.exec` also remains a public selector, including panel selectors such
as `open`. `yu app workbench --gui:applet [literal user args...]` selects the
named GUI, passing its declared `exec` as **one literal argument**, followed by
unchanged user arguments. `exec` is not a shell command, command line or
executable. No caller-supplied path can select a GUI. Unknown or invalid names
are refused before execution.
Non-interactive named launchers without `YUOS_SESSION` use authenticated Root
registration without creating an Admin/bootstrap session. Root derives the
actual parent or bounded unregistered-launcher policy, not caller environment.

`desktop.entry` overrides only the default GUI's package-relative file. Without
it, top-level `entry` or the runtime default is used. Ordinary operations and
`mcp.entry` are unchanged. Each additional entry requires `entry`, `exec`,
localized `name` (including `en`) and an explicit `needs` array. Names match
`[a-z][a-z0-9_-]*`, at most 64 ASCII bytes, with at most 64 extra entries.
Optional `panel_applet` defaults to false independently on each extra surface;
it controls presentation environment only, not LayerShell or resource rights.

`desktop.needs`, when present, is the default GUI's exclusive declaration:
`[]` requests no capabilities. Omission retains the legacy constrained union
of fixed/wild `operations.*.needs`. An additional GUI always uses only its own
needs, never another GUI's, ordinary operations' or MCP tools' needs.
Explicit `null` is invalid, not omission. Fixed needs outside the launcher's
delegation require the existing exact-owner capability approval; wild needs
can inherit only bounded same-verb launcher authority. Argument-bound and
explicit conditional GUI needs are disclosed as unsupported/unbound and are
not granted at startup. The old operation-needs union retains its legacy
condition behavior. Literal GUI argv does not bind operation argument names
or authorize a later file-picker selection.

Both the default GUI and a named GUI may additionally declare an
`optional_needs` array using the same Need model:

```json
{
  "needs": [{
    "verb": "ui.panel",
    "scope": {"kind": "fixed", "scope": {"kind": "wild"}},
    "why": {"en": "Required panel display"}
  }],
  "optional_needs": [{
    "verb": "sys.identity",
    "scope": {"kind": "fixed", "scope": {"kind": "name", "value": "manage"}},
    "why": {"en": "Optional account management"}
  }]
}
```

Omission means `[]`; explicit `null` is invalid. The field belongs only to
`desktop` or `desktop.entries.<name>`, not ordinary operations or MCP tools.
It never borrows default/other GUI, operation or MCP permissions. Mandatory
`needs` still refuse startup when authorization is missing or denied. In
particular, required LayerShell authority must remain a mandatory `ui.panel`
need. Adding the same permission to `optional_needs` does not relax it.
For legacy default GUIs, authors must explicitly declare their required
`desktop.needs` subset before moving a feature out of the old operation union.

Optional missing, denied or ask-unapproved rights leave features unavailable
without blocking GUI startup or asking for new consent. Root may admit only
already authenticated parent rights or an existing exact owner/session/scope
reusable approval, spending one use while retaining its expiry. Once-grants,
restoration receipts and another session's approvals cannot supply optional
startup rights. Wild bindings keep the same bounded parent-only meaning.
Package, App-deny, Activity, Workspace and protected-resource/mount rules
still apply; unsupported optional resources are not mounted as a fallback.
Optional argument-bound/conditional needs remain unbound. AI needs still
require a valid AI policy and remain budget-gated.

#### Optional system AI at installation

To offer optional system AI at installation, list `{"verb": "ai.chat"}` or
`{"verb": "ai.chat.untrusted"}` in that GUI's `optional_needs`.
For document input use `ai.chat.untrusted`, with `external-content` in `ai.origins`,
strict or standard safety, and a positive `ai.budget.monthly_units`. For input
without external content use `ai.chat` with its corresponding origin. Only the
optional GUI entry uses the Store installation choice, which applies to the
provider and model the user has configured at installation.

Only verified publisher-signed Apps requesting this optional feature get this
choice, and only when a model is configured. The protected Store reviews show
the provider, model and AI policy; accepting installation alone does not enable
it. Explicit Allow takes effect only after verified publication. Normal owner
GUI launches may then call the same SDK `ai.chat` without a second AI prompt.
Turning it off in Settings → Applications retires the current grant. A declined
or unavailable choice leaves ordinary App use available. The installation
choice is for text AI; all supported models can be managed as described below.

Signed directory installation remains a direct Root publication and the
protected review belongs to UID 0. It must not enable an ordinary user's AI
session. The ordinary owner can make a separate choice in Settings.

#### Model permissions in Settings

Declare a model capability without a condition in the GUI entry's
`optional_needs` and set a positive App AI budget. For example, to offer
speech synthesis:

```json
{"verb": "ai.audio.tts"}
```

An eligible signed App starts without the right. When the user requests the
feature, catch the SDK's permission error. For `detail.reason ==
"model_permission_required"`, show App-owned copy such as “Enable speech
for this App in Settings → Applications.” Open Settings only when the user
selects that action:

```python
try:
    speech = ai.infer({"origin": "user-input", "input": {"kind": "tts", "text": text}})
except ai.AiDenied as error:
    if error.payload.get("detail", {}).get("reason") != "model_permission_required":
        raise
    # Show your prompt; use ai.open_permission_settings as the button handler.
```

The button uses the separate `desktop.launch` capability scoped to
`com.yuos.Settings`. If unavailable, keep the manual Settings instructions
visible. Opening Settings does not authorize or send the prompt. After the
owner enables the exact App/model choice, reopen the App and let the user
retry. Changing a choice retires the old App session. A changed package or
destination requires its own choice. This grants no MCP invoke, filesystem,
provider credentials, or model-selection authority.

The authenticated review lists required and optional purposes separately and
includes optional declarations in the permission-contract digest and deny
inventory. Root registration reports `optional_needs_ungranted` indexes, also
projected as `YUOS_APP_OPTIONAL_NEEDS_UNGRANTED_JSON` for the selected GUI.
This is an initial presentation snapshot, not permission authority or a promise
that a provider is ready. Apps must retain normal live policy/provider checks
and handle existing denial errors. No later approval silently widens the
running grant; expiry/revocation retains checked instance retirement.
This contract adds no on-demand widening or file-picker capability.

A GUI requiring a bounded owner-relative filesystem request can instead use
the common Need binding:

```json
{
  "verb": "fs.delete",
  "scope": {"kind": "owner-path", "path": "Documents/**"},
  "why": {"en": "Delete the owner's selected documents"}
}
```

Root resolves this only against authenticated `PathContext.home`, for example
`path:/home/alice/Documents/**`; neither manifest/request owner fields, `HOME`,
`YUOS_OWNER_HOME`, cwd nor SDK output select the authority. The same binding
works for operation and MCP needs through their Root-derived contexts.
Only Path-kind verbs accept it. Paths must be normalized relative paths:
no absolute, backslash, empty/dot/dotdot segment, colon, `$`, NUL/control or
other `*` pattern. A trailing `/**` selects a subtree; `**` alone selects the
authenticated home subtree. Literal spaces and Unicode are retained; path and
resolved path fit 4096 UTF-8 bytes and literal segments fit 255 bytes.
Root applies existing canonical Path-scope containment and rejects symlink
escapes from the owner's home. Normal protected-resource and descriptor/mount
checks remain mandatory, including refusal of `home/**` when it contains
credential roots. This declaration does not make that resource view available.

Owner-path needs request an exact bounded capability via normal authorization.
Existing low-risk owner scopes may cover them; a missing High-risk `fs.delete`
scope requires exact-owner approval. No default capability or Admin bootstrap
is added, and legacy Wild inheritance is unchanged. Protected review shows the
actual owner-resolved canonical scope while retaining the declared purpose.
SDK `manifest.resolve_owner_path` (`ResolveOwnerPath` in Go) is only a lexical
preview: it reads no home/environment/filesystem and grants or mounts nothing.
Root independently derives the owner and canonical target before authorization.

Panel hosting separately requires `ui.panel` with fixed canonical Wild scope,
for example `"scope":{"kind":"fixed","scope":{"kind":"wild"}}`. This High-risk
resourceless right is absent from System Agent and default/unregistered launcher
ceilings; normal exact-owner approval remains necessary. `ui.window`, a panel
label, `panel_applet` and clipboard rights do not imply it. Root projects
LayerShell only from the actual installed instance's live grant; selection
read/write, capture, virtual keyboard and global input remain independent.

Root retains the exact selected package, name, entry and selector throughout
registration and launch. Each target must be an authenticated regular file in
both the signed file tree and envelope entrypoint inventory, executable for a
binary runtime. The same sandbox, process identity, owner review, live grants,
App deny gates, renewal and checked retirement apply to every surface. The App
data/session/audit identity and cross-owner mutation fence remain App-scoped.
Protected review discloses every target and its exact needs/purposes; the new
fields enter its comparison digest but grant nothing.

The public core helpers `Manifest::gui_entry(name)`,
`gui_entry_for_cli(selector)` and `gui_entrypoints()` resolve manifest data
without authority. `AppLaunch::gui_entrypoint(name)` applies the generic signed
file gate. A signer appends the deduplicated GUI entrypoint inventory to its
existing ordinary/MCP entries: if `desktop` exists, use
`desktop.entry` or top-level `entry` or the runtime default, then each
`desktop.entries.*.entry`. Do not export arbitrary auxiliary binaries.
All four public SDK manifest types retain these optional fields; wire/ABI 1
is unchanged. This source contract does not publish an SDK/platform artifact,
adopt a pin, admit unsupported resources or complete installed GUI acceptance.

Declared needs and display transport availability are not native resource or
installed GUI acceptance. The authenticated Host must retain live grants,
independent selection/LayerShell checks and checked resource retirement.
Unsupported resources, private IPC and owner-home views remain explicit gates;
an App preflight is never a substitute for actual provider enforcement.

### MCP service lifetime and Activity work

An App's signed `mcp.entry` runs through the isolated owner/App service Host.
Its declared lifecycle controls service reuse; the existing owner/App data
partition persists independently of a task or leased execution UID. A service
never mounts the union of all its tools' host paths or receives lifetime
network access.

Each `tools/call` is checked against `mcp.tools[].args` and `needs`, receives
canonical arguments, and carries a single-use, action-bound authorization.
Caller invoke authority does not enter the App. Broker-mediated effects can
use a reusable child with no standing target authority; direct-resource calls
receive a freshly authorized call-scoped sandbox. Exact grants, RPC and
clearing retain their ordinary serialized lifecycle. There is no in-worker
or unsandboxed fallback.

Activity constraints belong to the individual invocation, not the shared
service Host. Root rechecks the pinned policy before broker effects and
records Activity-associated task results as
[caller-reported receipts](execution-receipts.md) to the same backend used by
terminal, Web and native desktop. Unknown call outcomes remain indeterminate;
receipt recording never repeats an App call.

## 11. Ship it

Once the app does what you want:

```sh
YUOS_APPS_DIR="$HOME/my-apps" yu app lint hello  # static check
yu provenance sign --kind app --id hello --version 0.1.0 \
    --path ~/my-apps/hello --key ~/.secrets/my-publisher.json \
    --entrypoint main.py             # bind the whole tree to your key
yu app install ~/my-apps/hello --review  # inspect verified requests without installing
sudo yu app install ~/my-apps/hello # confirm the signed directory install as Root
yu app hello say "Hello from CLI"  # run as your normal user
```

The [MCP example](#quickstart-an-mcp-only-app) uses `server.py` instead of `main.py`
for an MCP-only App. Sign every declared entrypoint. A signature authenticates
the package; it does not grant capabilities or AI consent. Directory installation
requires system publisher trust, Root and a running broker. Signing does not
enroll a publisher, and a per-user trust entry does not authorize a Root install.
Re-sign after changing package contents. For distribution, pack the signed
directory and give the administrator the public publisher entry separately.
Never distribute the private key. See [package provenance](extension-provenance.md)
for trust, revocation, and rollback.

### Troubleshooting

| Symptom | What to check |
|---|---|
| Publisher unknown or package no longer verifies | Check the [system trust store](extension-provenance.md#trust-roots) and verify that the signed directory has not changed. |
| App or tool not found | Run `yu app` and `yu app tool list <id>`. Check the declared entrypoint and use the `<app-id>.<command>` tool name for CLI resolution. |
| Missing MCP call context | Run the installed App through `yu app <id> <command>` or the Gateway, not by sending a bare `tools/call` request to its executable. |
| Permission denied | Check the caller's invoke approval and the tool's signed `needs` separately. For AI, also check the [consent path for the calling surface](#7-ai-features). |
| Install outcome is unknown | Follow the [directory-install recovery procedure](#installation-permission-review), not another install attempt. |

## See also

* [`extension-provenance.md`](extension-provenance.md) — package
  signing, trust roots, install pipeline, revocation, rollback.
* [`app-ai-integration.md`](app-ai-integration.md) — AI gate, manifest
  reference, lifecycle, audit surface.
* [`app-ai-tool-catalog.md`](app-ai-tool-catalog.md) — every
  agent-callable tool with its verb and scope.
* [`browser-attached-design.md`](browser-attached-design.md) —
  worked example of a non-trivial app with a Chromium extension and a
  native-messaging bridge.
* [`yuos-app/products/notifications/apps/notify/`](https://github.com/xiaoyu-work/yuos-app/tree/main/products/notifications/apps/notify),
  [`yuos-app/capabilities/storage-sdk/apps/kv/`](https://github.com/xiaoyu-work/yuos-app/tree/main/capabilities/storage-sdk/apps/kv),
  [`yuos-app/products/files/apps/fs/`](https://github.com/xiaoyu-work/yuos-app/tree/main/products/files/apps/fs) — small bundled apps that double as
  reference templates.
