# Workflows Module

## Purpose

`.github/workflows/` defines the pull-request test pipeline plus manually
dispatched and reusable test/publication pipelines.

## Responsibilities

- Run core/browser/Python validation, including resolved App-main source staging and
  preservation of the package-owned runtime during rootfs registration.
- Fetch current App main and prepare an immutable snapshot before cross-repository Rust tests,
  retaining real App/broker coverage without a second local implementation.
- Run OS adapter/SDK/runtime and pinned staging/MCP fixtures, not the removed
  OS `apps/` tests. App helper tests/vectors belong to App `tests/shared`;
  process fixtures receive separately staged common support, the required
  `python3-idna` runtime dependency and OpenSSL for private HTTPS certificates.
- Exercise the public App data codec/client, independent OS provider library and
  executable, exact export/ELF guards, and private real-kernel fixtures whose
  clients remain non-Root. No GUI/Clipboard admission or release gate is relaxed.
- Build the internal display control/PAM crates with `libpam0g-dev`, and run
  the actual Root-authenticated GUI/session fixture with a private `dbus-daemon`.
  Private namespace cases cover startup-failure exit and deferred retirement
  reclamation as well as normal lifecycle. Agent packaging builds the PAM
  library for GNU/glibc even when core uses musl.
- Keep actual Editor, Files and Terminal native acceptance visible as separate
  display-matrix modes. Terminal uses the original published-platform build,
  empty GUI authority and the common Host's existing private PTY/sandbox;
  unsupported default-shell resources are failures, not injected privileges.
- Keep Settings startup visible as its own native display-matrix mode, using
  original resources and public XKB data with empty mandatory capabilities
  and every optional right ungranted. This is not system-settings mutation or
  hardware acceptance.
- Run egress retirement cases against private listener queues and stalled DNS,
  with synthetic NSS configuration confined to a private mount/network namespace.
  Real lookup children must be reaped before the retirement acknowledgement.
- Exercise App-service data views with real sandboxed MCP/SQLite writes across
  distinct execution UIDs and mount lifetimes, retaining owner file identity
  and refusing foreign roots, aliases, submounts and replaced bindings.
- Exercise task-host ordinary App registration and persistent SQLite writes
  through authenticated private broker messages, across Host namespace/UID
  replacement, without exposing caller-selected data roots.
- Exercise checked App-service retirement with a real Host/cgroup, retained
  mount failures, cancellation and owner/identity/capacity fencing; cleanup
  failure must not become a successful stop or replacement.
- Exercise ordinary App bind-gated startup with the real runner and sandbox,
  including refused/delayed binding, exact stdin limits, raw bytes and EOF.
- Exercise real staged Calendar MCP writes and public SDK reads through the
  owner-scoped broker and standalone reader, including live WAL visibility,
  wrong scopes, single-use grants and physical read-only mounts.
- Exercise the real OS capture helper as an unprivileged owner against a private
  portal, retaining Root dispatch authentication, bounded PNG and cancellation
  without an App executable or live display.
- Exercise the independent nearby transport's actual TLS loopback, private
  D-Bus lease/failed-retirement cases and Root-only temporary-file/sandbox
  boundaries without acquiring wireless hardware; see `scripts/ci/test-boundaries.sh nearby`.
- Build and verify Agent, Base and Desktop packages for one signed APT release.
- Publish marketing website changes independently while preserving the signed
  OS APT files on the shared Pages site.

## Key Files

| Path | Role |
| --- | --- |
| `test.yml` | Pull-request, manual, and reusable test/clippy workflow |
| `release.yml` | Manual OS release on `main`; publishes the signed Agent, Base and Desktop updates together |
| `publish-website.yml` | Manual static website publication on `main`, independent of App and OS builds |
| `publish-sdk-release.yml` | GitHub SDK artifacts and synchronized language tags |
| `publish-app-platform.yml` | Manually published versioned SDK/runtime/toolkit development artifact for independent App builds |
| `../actionlint.yaml` | Compatibility with GitHub's queued concurrency |
| `../../scripts/ci/` | Release planning, signing-key import, package checks and installed Agent/privileged test scripts |

`release.yml` freezes one
[date-based package version](../../packaging/README.md#independent-publication)
and one App-main snapshot for the whole run. It calls `test.yml` with that
snapshot before building the packages. Desktop builds the Shell from OS source
inside its rootfs build and checks the resulting signed tree inventory.
Its package jobs use native GitHub-hosted
amd64/arm64 runners and remove only
unused Android, .NET and GHC toolchains from those disposable machines before
the existing disk-space preflight.

The Agent matrix installs and reinstalls the exact Agent/Base pair in one
APT transaction and verifies the installed floor before uploading the packages.
Before Agent upload, the amd64 runner also installs the actual official
signed App Store KV/DB bundle for a fresh synthetic owner through protected
displayed-revision review, then runs the existing real product browser/MCP
driver against the installed Web server. This is public-origin delivery
acceptance, not a source-fixture or physical-device claim.
The publisher compares every package's declared protocols with both the new
pool and authenticated previous manifests before Pages deployment.

Release runs queue without cancelling one another. All packages are merged
with the authenticated existing pool and signed. Both workflows share the
`yuos-apt-pages` deployment lock, and each preserves the other publisher's
live files from its inventory. Website-only publication verifies the signed
APT inventory with the OS archive key and does not renew or sign any files.
See [Website publication](../../web/MODULE.md#publication) for first use.
Non-expiring indexes need no scheduled refresh. The OS release does not
build installer, Docker or WSL images.

SDK publication also builds tested portable `yu` signing executables for amd64
and arm64 from that same commit, after its full Root validation succeeds.
For an initial App publication, validate the exact `main` commit with
`test.yml`, publish the App platform and SDK artifacts, then publish the
signed App APT repository. `release.yml` consumes that repository; it cannot
create the upstream App index.
The SDK Release binds those bounded assets, dependency source pins and upstream
notices to its checksums. This lets the first App channel authenticate snapshots
before the coordinated Agent ownership cutover, without a circular dependency
on an already-published Agent APT package. App publication pins an individual
immutable executable URL and digest, never OS source or a mutable CI artifact.
Publishing a signing tool does not install or activate an Agent or App.

## Dependencies

Workflow commands call repository scripts that remain the implementation source
of truth. Long privileged test steps live in `scripts/ci/test-boundaries.sh`;
the workflow keeps their original names and order, and the packaging contract
checks both the invocation and the extracted implementation.
Secrets are referenced by name only. Package publication requires the
signing key and never produces an unsigned fallback. Trigger documentation
must match each `on:` block.

## Tests

Run `actionlint` when available, parse YAML, and run the exact changed shell
commands in the narrowest safe environment.
