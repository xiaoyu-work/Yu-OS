# App development platform

The OS exports versioned development libraries; it does not supply App business
helpers. `yuos-app` owns its support libraries, product sources, builds and
independent Debian releases. Installed Apps communicate through the SDK and
capability-gated OS services.

## Development artifact

[`packaging/app-platform.json`](../packaging/app-platform.json) declares the
`yuos.app-platform/v1` contract and its release version. The
[publication workflow](../.github/workflows/publish-app-platform.yml) is manually
dispatched from `main`. It publishes:

```text
yuos-app-platform-v<VERSION>
  yuos-app-platform-<VERSION>.tar.gz
  SHA256SUMS
```

Consumers pin the version, HTTPS artifact URL and SHA-256, not an OS Git
checkout. The archive's `platform.json` declares its runtime ABI, named exports,
source revision and complete file inventory. A modified or incomplete cache is
an error, not permission to import a neighbouring source checkout.

| Export | Contract |
| --- | --- |
| `python-sdk`, `rust-sdk` | Public SDK and its wire-v1 clients |
| `python-runtime`, `rust-runtime` | First-party runtime clients; no privilege or policy bypass |
| `ui-toolkit` | Native rendering and widget development libraries |
| `launcher-client` | The existing launcher library/protocol dependency |

The version-1 archive retains the relative layout needed by those libraries'
Cargo manifests. Consumers resolve the named exports rather than selecting
arbitrary OS source paths. The archive excludes `core`, App products, credentials
and App support libraries. Only committed library bytes are published; Git LFS
objects must match their committed content hashes. Release output is
reproducible and an existing version cannot be replaced.

`yuos_runtime` remains first-party-only. Publishing its development client does
not authorize a third-party App to use a private service, add a trust root,
inherit another App's identity or bypass capability/consent enforcement.
Compatible OS implementation changes need no App rebuild. A changed service
contract needs a versioned interface and explicit package dependency.

The public Rust `applet` client and its
[versioned pipe contract](../yuos-sdk/wire/v1/applet-services.md) belong to
`rust-sdk`; `desktop/services/yuos-applet-services` and the installed provider
binary do not enter the archive. Source-cohort checks may canonically prepare
native workspaces against a separate, explicitly unpublished copy of these
public libraries. They must not alter a verified artifact or substitute for a
published pin. The initial Yu OS App platform version is `0.1.0`, with runtime
ABI 1. The App lock must use the checksum of the published archive, not a
locally built stand-in. Consumers still need the installed Applet provider and
OS admission.

From the repository root:

```bash
python3 -m pytest -q scripts/tests/test_app_platform.py
python3 scripts/app_platform.py --version 0.1.0
```

The build command only creates a local archive; it must not trigger publication.
An App platform release is neither an OS image nor an installed App update.
SDK artifacts are build inputs; installed App downloads and updates use the
separate signed App APT channel.

The Agent Debian package derives its versioned
`yuos-app-runtime-v1 (= 0.1.0)` Provides from `packaging/app-platform.json`.
Independent App packages require `yuos-app-runtime-v1 (>= 0.1.0)`. That
dependency supplies the runtime, not App authorization.
