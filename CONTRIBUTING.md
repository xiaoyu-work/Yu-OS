# Contributing to Yu OS

Coding agents and contributors should read [`AGENTS.md`](AGENTS.md) for task
routing and authoritative validation commands, then
[`ARCHITECTURE.md`](ARCHITECTURE.md) for component boundaries and data flows.

## Building from Source

### Prerequisites

- Linux (or WSL2 on Windows)
- Rust 1.94+
- Python 3
- Docker (for building the image)
- Root access (for rootfs bootstrap)
- `libpam0g-dev` on Debian/Ubuntu when building the OS display PAM module

### Build the Rust Core

```bash
cd core
cargo build --release
```

The Yu OS-owned terminal UI is compiled into `yu` by the ordinary core build.
It has no separately fetched frontend source or runtime binary.

### Build the OS display runtime

The unpublished OS display runtime has separate native build inputs. From the
repository root, with the PAM development library installed:

```bash
cargo build -p yu --bin yuos-display-host --bin yuos-display-session --bin yuos-gui-runner --locked
cargo build -p yuos-display-login --locked
cargo test -p yuos-display-control -p yuos-display-login --locked -- --test-threads=1
```

The PAM module must target GNU/glibc, including when the core package uses musl.
These commands do not install or restart a desktop. The
[private authenticated fixture](crates/yuos-display-login/MODULE.md) uses
isolated mounts and disposable login state; do not substitute a live login.

### Build the Rootfs + Docker Image

```bash
# Bootstrap Debian rootfs, install Node.js 24, apps, browser engine
sudo ./rootfs/build.sh

# Build the Docker image
./build.sh docker

# Or via yuos-ctl
./cli/yuos-ctl build
```

### Build a WSL2 Distribution

```bash
# Produces build/yuos-wsl-amd64.wsl
sudo ./build.sh wsl
```

On Windows, import and launch from PowerShell:

```powershell
wsl --install --from-file .\build\yuos-wsl-amd64.wsl `
    --name yuos --location C:\WSL\yuos --version 2

# Or run the helper:
.\targets\wsl\install.ps1
```

### Build a Live ISO

```bash
# Host requirements (Debian/Ubuntu):
sudo apt install squashfs-tools xorriso grub-pc-bin grub-efi-amd64-bin \
                 grub-common mtools

# Produces build/yuos-live-amd64.iso (hybrid BIOS+UEFI bootable)
sudo ./build.sh iso-live

# Boot in QEMU (BIOS):
qemu-system-x86_64 -m 2G -cdrom build/yuos-live-amd64.iso -nographic

# Boot in QEMU (UEFI, requires ovmf package):
qemu-system-x86_64 -m 2G -bios /usr/share/ovmf/OVMF.fd \
                   -cdrom build/yuos-live-amd64.iso
```

### Build a VM Disk Image

```bash
# Host requirements (Debian/Ubuntu):
sudo apt install qemu-utils parted dosfstools mtools rsync

# Produces build/yuos-vm-amd64.qcow2 (hybrid BIOS+UEFI bootable)
sudo ./build.sh vm

# Multiple formats in one build:
sudo FORMATS="qcow2 vmdk vhdx" ./build.sh vm

# VMware Fusion / Workstation build with VMware Tools guest integration
# for guest resize / clipboard (preset wrapper sets the desktop features):
sudo ./presets/desktop.sh
# ...or spell out the equivalent FEATURES manually:
sudo FEATURES=base,yuos-core,systemd,kernel,desktop,vmware,copilot-cli,grub-disk,vm,apt-source,local-user \
     FORMATS=vmdk ./build.sh vm

# Larger disk:
sudo SIZE=16G ./build.sh vm

# Boot in QEMU (BIOS):
qemu-system-x86_64 -m 2G -nographic \
    -drive file=build/yuos-vm-amd64.qcow2,format=qcow2,if=virtio

# Boot in Hyper-V Gen 2 (UEFI): disable Secure Boot first
#   Set-VMFirmware -VMName yuos -EnableSecureBoot Off
```

### Build an Installable ISO

```bash
# Host requirements (same as iso-live):
sudo apt install squashfs-tools xorriso grub-pc-bin grub-efi-amd64-bin \
                 grub-common mtools

# Produces build/yuos-installer-amd64.iso (hybrid BIOS+UEFI bootable).
# This ISO boots into a kiosk-mode Calamares installer that copies the
# live system to a real disk and configures GRUB on the target.
sudo ./build.sh iso-installer

# Test in QEMU with an empty target disk:
qemu-img create -f qcow2 build/yuos-target.qcow2 16G
qemu-system-x86_64 -m 4G \
    -cdrom build/yuos-installer-amd64.iso \
    -drive file=build/yuos-target.qcow2,format=qcow2,if=virtio \
    -boot d
```

The installed system has the Yu OS apt repository pre-configured, so
`sudo apt update && sudo apt full-upgrade` will pull newer Yu OS packages.
See [`docs/updating.md`](docs/updating.md) for the unified update path across
installed targets.

### `apt` repository

Run **Release Yu OS** (`release.yml`) manually on `main`. Every new run
automatically assigns a date-based version, runs the shared tests and publishes
Agent, Base and Desktop together to the signed APT source. No release target
or version needs to be selected. The image commands above are for local
builds, not CI publication. To update the public website without releasing
system packages, run [Publish Website](web/MODULE.md#publication).

Release runs queue without cancellation. The publisher verifies compatibility
with the authenticated previous release, and non-expiring indexes require no
weekly refresh. See [the publication contract](packaging/README.md#independent-publication).

See [the workflow module](.github/workflows/MODULE.md) for the separate
website, system, SDK and App platform publication entry points.

Run manual validation with `gh workflow run test.yml --ref main`. Do not push
a temporary branch to dispatch it. GitHub blocks creating branches under `ci/`.

The repository is consumable as:

```bash
curl -fsSL https://xiaoyu-work.github.io/Yu-OS/apt/yuos-archive-keyring.gpg \
  | sudo tee /usr/share/keyrings/yuos-archive-keyring.gpg >/dev/null
echo "deb [signed-by=/usr/share/keyrings/yuos-archive-keyring.gpg] https://xiaoyu-work.github.io/Yu-OS/apt trixie main" \
  | sudo tee /etc/apt/sources.list.d/yuos.list
sudo apt update
sudo apt install yuos-agent
```

For details see `packaging/README.md`.

### SDK release

The manually dispatched **Publish SDK Release** workflow validates one SemVer
across Python, Node, Rust, and Go, regenerates wire bindings, runs all SDK
tests, and publishes GitHub Release artifacts plus the matching SDK and Go
module tags. It does not publish to PyPI, npm, or crates.io.

### Run Locally (Development)

```bash
# Point yu to local apps directory
YUOS_APPS_DIR=./apps YUOS_DATA_DIR=/tmp/yuos-data ./core/target/debug/yu sys info
YUOS_APPS_DIR=./apps YUOS_DATA_DIR=/tmp/yuos-data ./core/target/debug/yu fs ls .
```

### Run Tests

From the repository root, prepare the product/capability inputs used by App
contract and worker integration tests. Preparation fetches current App `main`,
records its exact SHA, and validates that immutable cache; Cargo itself does
not download fixtures. The source selection's optional
`capabilities` groups resolve only explicitly marked shared-capability clients,
not business products or local-source fallbacks. `doc` is owned by the external
Document Engine group; its declared Files parser is staged in the Agent package.
`db` and `kv` are owned by Storage SDK, with separate identities, exact scopes
and their existing owner/App SQLite and JSON namespaces. No migrated source
is loaded from a local OS copy. KV has only its manifest-selected MCP entrypoint,
not a `main.py`; runtime fixtures use public stdio calls and complete staged
payloads. See [the KV compatibility note](docs/updating.md#app-data-moves-into-per-app-directories).
`net` is owned by the HTTP capability group. Its two MCP/CLI tools consume the
App-owned common HTTP support and OS policy/transport; contracts use the public
MCP entrypoint rather than legacy operation schemas or private client modules.
`summarize` is owned by AI Helpers, using public SDK AI and the bundled
policy/memory exports under its own identity, consent and budget. Its fixture
calls must use isolated synthetic wire peers, never paid/live models.
All 75 original App sources and their common libraries are external. The OS
has no `apps/` source tree. App `shared/python` stages separately into
`usr/lib/yuos/python`, beside OS SDK/runtime, through
`python3 scripts/app_sources.py --stage-shared <root>`. Its helper tests/vectors
belong to App `tests/shared`. Current Agent package assembly remains
compatibility delivery, not the independent App APT ownership cutover.

```bash
python3 scripts/app_sources.py

# Validate explicit source kinds and the published Doc/DB/KV/Net/Summarize fixtures.
cargo test -p yu --test app_source_fixtures -- --test-threads=1

# Core tests share process-global environment variables.
(cd core && cargo test -- --test-threads=1)

# Exact CI lint.
(cd core && cargo clippy -- -D warnings)

# OS Capture helpers: private PNG/interactive portal, real yuosd children and checked Close.
GITHUB_WORKSPACE="$PWD" RUNNER_TEMP="$PWD/build" \
  bash scripts/ci/test-boundaries.sh capture

# OS-owned Python tests, from the repository root. App tests live in yuos-app.
PYTHONPATH=yuos-sdk/python/src \
  python3 -m pytest -q adapters yuos-sdk/python/src \
    packaging/deb/tests/test_app_sources.py

# Browser broker.
cargo test -p yu --lib yuosd::browser -- --test-threads=1

# OS Applet providers build independently, without App sources or GUI inputs.
cargo test --manifest-path desktop/services/yuos-applet-services/Cargo.toml --lib --locked -- --test-threads=1
cargo test --manifest-path desktop/services/yuos-applet-services/Cargo.toml --features provider --locked -- --test-threads=1
```

### Project Structure

```
yuos/
├── core/              Rust binary (yu)
│   └── src/
│       ├── main.rs        Entry point
│       ├── agentd/        Unprivileged model/tool worker supervision
│       ├── extension_host/ Task-owned dynamic App/MCP process boundary
│       ├── router.rs      Command dispatch
│       ├── sandbox.rs     Namespace + cgroup isolation
│       ├── proc.rs        Process session manager
│       ├── yuosd/browser/ Private Helium rendering
│       ├── bridge.rs      Python app subprocess bridge
│       ├── audit.rs       JSONL audit logging
│       ├── sysinfo.rs     Native system info
│       └── apps.rs        App manifest discovery
├── packaging/         OS packages, App main selection and resolved build snapshots
├── rootfs/            Linux rootfs build scripts + overlay
├── targets/           Per-distribution build scripts (docker, wsl, iso, vm)
│   └── docker/          Dockerfiles + build.sh for the docker target
├── web/               Static marketing website
├── build.sh           Top-level dispatcher (./build.sh <target>)
├── cli/               yuos-ctl management tool
├── clients/           Bridge (LLM ↔ Yu OS)
└── .github/workflows/ Test and publication workflows
```

## Architecture Rules

These rules apply to every subsystem in `core/src/`. They exist so
agent state stays auditable and so cross-machine work (sandbox,
remote provider) does not require forked implementations.

### Capability seam requires all three roles

A *capability seam* is what lets one implementation of a primitive
be swapped for another (e.g. a local-only filesystem provider
replaced by one that transparently proxies to a sandbox or a remote
node). It is not a seam unless all three roles are present:

1. **Service Definition** — the interface. A trait or explicit type
   in a well-known module. Callers depend on this, not on the
   provider. Example: `agent::memory::app_memory`.
2. **Service Provider** — the implementation behind the interface.
   There MAY be more than one (SQLite FTS, vector, remote); the
   interface is what makes them interchangeable. Example:
   `agent::memory::sqlite_fts::MemoryDb` and
   `agent::memory::semantic`.
3. **Consumer** — typically a model-facing `yu_*` tool, or another
   subsystem that goes through the definition. Example: the
   `yu_memory` / `yu_recall` tools.

One role alone does not constitute a seam. A concrete type with no
interface is not a seam (it is an implementation). An interface
with no consumer is speculative. An interface with a single
hard-wired provider that callers reach past is not a seam either.

**Why this matters here.** Yu OS wants to run agent work in a
sandbox or on another machine without forking every subsystem.
That property only holds if filesystem, subprocess, memory, and
network all share the same three-role shape: pointing the provider
layer at a remote target then relocates the whole capability
without touching consumers. Subsystems that skip the definition
(consumer imports the concrete provider) block that entire
property for their capability.

**When adding a new subsystem** (diagnostics, storage, network,
sandbox surface, credential store, …):

- Introduce the Service Definition first, in its own module. Give
  it a name that describes the capability, not the implementation
  (`storage`, not `sqlite_storage`).
- Land at least one Consumer against the Definition in the same
  change. If nothing consumes it yet, the seam is speculative and
  the definition should wait.
- If you only need one provider today, the seam is still worth
  it — the second provider is what pays off the design, and
  retrofitting a seam onto a hard-wired consumer is the expensive
  case this rule is meant to prevent.

### Model-visible content must be logged

Anything that reaches a model request has to be reconstructable
from the session log. Concretely: system-prompt injections
(`MEMORY.md`, `USER.md`, due nudges, per-session extras) are
recorded as `injected` rows in the memory DB by
`agent::prompt::build_system_prompt_traced`. Adding a new kind of
model-visible input requires adding a corresponding record path
*in the same change* — a transcript reader must never have to
guess what the model saw.

### Long-running mutations are three-phase bracketed

Any operation that mutates durable state and can be interrupted by
a crash appends a `start` event *before* the mutation, then the
`end` (`Completed` / `Failed`) event *after* the mutation returns
success. A crash in between leaves an orphan `start` with no
matching `end`, which is detectable and explicitly treated as such
(`agent::memory::curator::CurationLog::orphaned_runs`). Do not
record "the operation finished" before the durable write returns.
## Nearby sharing development

The file/radio helper is an independent workspace, not another core member.
From the repository root on Linux/WSL:

```sh
cargo test -p yuosd-client --lib --locked
cargo test -p yu --lib nearby -- --test-threads=1
cargo test --manifest-path crates/yuos-nearby/Cargo.toml --locked
cargo test --manifest-path crates/yuos-nearby/vendor/owlyd/Cargo.toml --test service --locked
python3 -B packaging/deb/tests/test_nearby.py
```

CI's `scripts/ci/test-boundaries.sh nearby` additionally executes the ignored
owner-file and namespace cases as Root, using only generated temporary data
and a private network namespace. TLS loopback and private D-Bus cases require
no radio; neither they nor a successful build certify a physical Wi-Fi/iPhone
transfer. See [Nearby Sharing](docs/nearby-sharing.md) for the four clients and
target-device acceptance checklist.
