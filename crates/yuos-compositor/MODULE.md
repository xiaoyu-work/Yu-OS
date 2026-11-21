# Native Compositor Authority

## Responsibility

Provide the OS-private native compositor enforcement bridge for Hyprland.
It consumes the existing [display control contract](../yuos-display-control/MODULE.md);
it does not replace the Root broker or make App capability decisions.

The lease/origin rules preserve Root-only creator descriptors, actual
process/cgroup identity, single-use
context creation, independent selection read/write and panel rights, finite
leases and checked retirement. This implementation is GPL-3.0-only; the
shared Apache-2.0 protocol crate and the Agent remain separate.

## Files and ownership

| File | Purpose |
| --- | --- |
| `src/authority.rs` | Authenticated Root command processing, immutable grants, native resource accounting and correlated replies |
| `src/ffi.rs` | Bounded OS-private C ABI and event-thread confinement |
| `test/unit/authority.rs` | Grant separation, one-shot context, retirement and expiry regressions |
| `../../desktop/hyprland-yuos/include/yuos_compositor.h` | Native declaration and ownership contract |
| `../../desktop/hyprland-yuos/YuOs.cpp` | Native Wayland connection admission, origin tracking, timed control dispatch and checked resource callbacks |
| `../../desktop/hyprland-yuos/Desktop.cpp` | Closed Root window queries/effects over the native compositor model |
| `../../desktop/hyprland-yuos/test/abi.cpp` | Real C++ linkage and unauthenticated-startup refusal |

Only the protected compositor receives its Root activation descriptor.
An App ID, string label, native token supplied by an App, environment claim
or a successful FFI call is not a grant. Native code must obtain an origin
from the checked connection/context path and recheck rights before disclosure
or effects.

The native adapter must report actual client/listener addition and destruction.
Retirement is acknowledged only after those counts are empty. Root selection
payloads retain sealed, bounded MIME transfer semantics and do not inherit an
App lifetime. The native adapter must still enforce read checks on offers and
transfers; owning a payload does not authorize its disclosure.

The locker origin exists only while the matching Root-provided private
Wayland client is attached. It exposes the session-lock global without
clipboard, panel or context-creator authority. Normal login and App
connections cannot bind that global. Lock readiness and process custody
remain in the Root display supervisor.

The bridge is mandatory in the native Hyprland build. The authenticated
guest checks and official package activation remain separate gates.

Root desktop commands carry an exact display epoch, owner, correlation and
finite deadline. Native handoff and result acceptance both recheck that
binding. Stable native window IDs are metadata, not permission; broker clients
must bind any public identifier to the display epoch before requesting an
effect. Results return through a sealed bounded descriptor. A timeout or lost
requester never becomes GUI-instance retirement or an automatic mutation retry.

## Validation

From the OS repository root:

```bash
cargo test -p yuos-compositor -- --test-threads=1
cargo clippy -p yuos-compositor -- -D warnings
cargo build -p yuos-compositor
clang-format --version
clang-format --style=file:desktop/hyprland/.clang-format --dry-run --Werror \
  desktop/hyprland-yuos/include/yuos_compositor.h \
  desktop/hyprland-yuos/YuOs.hpp desktop/hyprland-yuos/YuOs.cpp \
  desktop/hyprland-yuos/test/abi.cpp
c++ -std=c++20 $(pkg-config --cflags wayland-server) \
  desktop/hyprland-yuos/test/abi.cpp desktop/hyprland-yuos/YuOs.cpp \
  target/debug/libyuos_compositor.a $(pkg-config --libs wayland-server) \
  -ldl -lpthread -lm -o target/yuos-compositor-abi
target/yuos-compositor-abi
```

When `CARGO_TARGET_DIR` is set, use that target directory for the static
library and smoke executable instead. A successful ABI smoke test proves
linkage and refusal only, not a rendered or authenticated GUI session.
The imported upstream formatting rules require a compatible formatter;
23.1.5 is the verified version. Older distribution formatters may reject the
upstream `PackArguments` option rather than checking the files.

The adapter also has an independent native build:

```bash
cmake -S desktop/hyprland-yuos -B build/yuos-compositor-adapter -G Ninja \
  -DCLAW_COMPOSITOR_LIBRARY="$PWD/target/debug/libyuos_compositor.a"
cmake --build build/yuos-compositor-adapter
ctest --test-dir build/yuos-compositor-adapter --output-on-failure
```
