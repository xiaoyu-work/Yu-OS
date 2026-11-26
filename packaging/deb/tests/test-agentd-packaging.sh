#!/bin/bash
# packaging/deb/tests/test-agentd-packaging.sh -- the unprivileged agent
# worker and extension host must be installed wherever yuosd is.
#
# yuosd no longer runs the model/tool loop in its own process: it spawns
# /usr/local/bin/yuos-agentd per task. An install that ships the broker
# without the worker, or a unit that never points at it, would leave
# every agent task failing, so both are contract-checked here.

set -euo pipefail

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/../../.." && pwd)

BUILD_DEBS="$PROJECT_DIR/packaging/deb/build-debs.sh"
UNIT="$PROJECT_DIR/rootfs/features/systemd/overlay/usr/lib/systemd/system/yuosd.service"
TEST_WORKFLOW="$PROJECT_DIR/.github/workflows/test.yml"
TEST_BOUNDARIES="$PROJECT_DIR/scripts/ci/test-boundaries.sh"
RELEASE_WORKFLOW="$PROJECT_DIR/.github/workflows/release.yml"
AGENT_PACKAGE_TESTS="$PROJECT_DIR/scripts/ci/test-agent-package.sh"
CARGO_TOML="$PROJECT_DIR/core/Cargo.toml"
SYSUSERS="$PROJECT_DIR/packaging/deb/yuos-framework/yuos-agent.sysusers"
PREINST="$PROJECT_DIR/packaging/deb/yuos-framework/preinst"
POSTINST="$PROJECT_DIR/packaging/deb/yuos-framework/postinst"
POSTRM="$PROJECT_DIR/packaging/deb/yuos-framework/postrm"
IDENTITY_HELPER="$PROJECT_DIR/packaging/deb/yuos-framework/extension-identities.sh"
IDENTITY_RS="$PROJECT_DIR/core/src/extension_host/identity.rs"

fail() {
    printf 'not ok - %s\n' "$*" >&2
    exit 1
}

assert_contains() {
    file=$1
    pattern=$2
    reason=$3
    grep -Fq -- "$pattern" "$file" || fail "$reason ($file is missing '$pattern')"
}

bash -n "$BUILD_DEBS" || fail "build-debs.sh is not valid bash"
for shell in bash dash; do
    "$shell" -n "$PREINST" || fail "yuos-agent preinst is not valid $shell"
    "$shell" -n "$POSTINST" || fail "yuos-agent postinst is not valid $shell"
    "$shell" -n "$POSTRM" || fail "yuos-agent postrm is not valid $shell"
    "$shell" -n "$IDENTITY_HELPER" || fail "extension identity helper is not valid $shell"
    for script in "$PREINST" "$POSTINST" "$POSTRM"; do
        {
            cat "$IDENTITY_HELPER"
            tail -n +2 "$script"
        } | "$shell" -n ||
            fail "assembled $(basename "$script") is not valid $shell"
    done
done

assert_contains "$CARGO_TOML" 'name = "yuos-agentd"' \
    "the agent worker must be a first-class cargo binary"
assert_contains "$CARGO_TOML" 'path = "src/bin/yuos-agentd.rs"' \
    "the agent worker binary must have an entry point"
[ -f "$PROJECT_DIR/core/src/bin/yuos-agentd.rs" ] ||
    fail "core/src/bin/yuos-agentd.rs is missing"
assert_contains "$CARGO_TOML" 'name = "yuos-extension-host"' \
    "the extension host must be a first-class cargo binary"
assert_contains "$CARGO_TOML" 'path = "src/bin/yuos-extension-host.rs"' \
    "the extension host binary must have an entry point"
[ -f "$PROJECT_DIR/core/src/bin/yuos-extension-host.rs" ] ||
    fail "core/src/bin/yuos-extension-host.rs is missing"
for helper in yuos-gui-runner yuos-display-host yuos-display-session yuos-file-manager yuos-notification-panel; do
    assert_contains "$CARGO_TOML" "name = \"$helper\"" \
        "the GUI helper must be a first-class cargo binary"
    assert_contains "$BUILD_DEBS" "ensure_bin $helper yu" \
        "Agent packaging must build every required GUI helper"
done
assert_contains "$BUILD_DEBS" 'ensure_bin libpam_yuos_display.so yuos-display-login "${RUST_TARGET/-musl/-gnu}"' \
    "the PAM module must be built for GNU rather than musl"
assert_contains "$BUILD_DEBS" 'install -Dm0644 "$PAM_DISPLAY_MODULE"' \
    "the PAM module must retain its non-executable installation mode"
assert_contains "$BUILD_DEBS" '/usr/lib/$PAM_MULTIARCH/security/pam_yuos_display.so' \
    "the PAM module must use the target multiarch installation directory"
assert_contains "$PROJECT_DIR/packaging/deb/yuos-framework/control" 'yuos-display-session-v3' \
    "Agent must provide the internal display runtime interface"
assert_contains "$PROJECT_DIR/packaging/deb/yuos-framework/control" 'libpam0g' \
    "Agent must declare its PAM runtime dependency"
assert_contains "$PROJECT_DIR/packaging/deb/yuos-framework/control" 'pkexec' \
    "Agent must install the protected terminal approval launcher"
assert_contains "$PROJECT_DIR/packaging/deb/yuos-framework/control" 'polkitd' \
    "Agent must install the unprivileged pkttyagent authentication presenter"
assert_contains "$PROJECT_DIR/packaging/deb/yuos-desktop/control" 'yuos-display-session-v3' \
    "Desktop must require the Agent display runtime"
if grep -Fq 'yuos-display-session-v2' \
    "$PROJECT_DIR/packaging/deb/yuos-framework/control" \
    "$PROJECT_DIR/packaging/deb/yuos-desktop/control"; then
    fail "the OS Shell requires the new display runtime without a v2 fallback"
fi
assert_contains "$PROJECT_DIR/packaging/deb/yuos-framework/control" 'yuos-file-manager-v1 (= 1)' \
    "Agent must provide the unprivileged file-manager transport interface"
assert_contains "$PROJECT_DIR/packaging/deb/yuos-desktop/control" 'yuos-file-manager-v1 (= 1)' \
    "Desktop must require the file-manager interface without an App entitlement"
assert_contains "$BUILD_DEBS" 'install -m 755 "$FILE_MANAGER_BIN" "$FRAMEWORK_STAGE/usr/lib/yuos/bin/yuos-file-manager"' \
    "Agent must own the actual file-manager transport binary"
assert_contains "$PROJECT_DIR/core/src/display_session/client.rs" 'command("/usr/lib/yuos/bin/yuos-file-manager").spawn()' \
    "the authenticated session must start and retain the OS intent transport itself"
assert_contains "$RELEASE_WORKFLOW" 'libpam0g-dev' \
    "the native package build must install PAM development inputs"
assert_contains "$RELEASE_WORKFLOW" 'bash scripts/ci/test-agent-package.sh installed' \
    "the Agent release must invoke the installed-package checks"
assert_contains "$AGENT_PACKAGE_TESTS" \
    'python3 -B core/tests/approval_polkit_process.py' \
    "the installed package must exercise actual PAM authentication, not a YES override"
python3 - "$PROJECT_DIR/rootfs/overlay/usr/share/polkit-1/actions/org.yuos.approval.policy" <<'PY'
import sys
import xml.etree.ElementTree as ET

expected = {
    "org.yuos.approval.decide": ("auth_self", "/usr/local/bin/yuos-approval-helper"),
    "org.yuos.approval.administrator": ("auth_admin", "/usr/local/bin/yuos-admin-approval-helper"),
}
actions = ET.parse(sys.argv[1]).getroot().findall("action")
assert [action.attrib["id"] for action in actions] == list(expected)
for action in actions:
    authentication, helper = expected[action.attrib["id"]]
    for audience in ("allow_any", "allow_inactive", "allow_active"):
        assert action.findtext(f"defaults/{audience}") == authentication, audience
    assert {
        annotation.attrib["key"]: annotation.text
        for annotation in action.findall("annotate")
    } == {
        "org.freedesktop.policykit.exec.path": helper,
    }
PY
assert_contains "$RELEASE_WORKFLOW" \
    'cargo build --release --locked -p yuos-display-login --target "${RUST_TARGET/-musl/-gnu}"' \
    "the Agent build must produce the native GNU PAM library"
assert_contains "$TEST_WORKFLOW" 'bash scripts/ci/test-boundaries.sh display' \
    "CI must invoke the authenticated display checks"
assert_contains "$TEST_BOUNDARIES" 'crates/yuos-display-login/test/fixtures/bootstrap.sh' \
    "CI must retain the private authenticated display fixture"
assert_contains "$TEST_BOUNDARIES" '"$CARGO_TARGET_DIR/debug/yuos-display-session" "$CARGO_TARGET_DIR/debug/yuos-file-manager"' \
    "the private fixture must receive the real display subscriber and FileManager1 facade"
assert_contains "$TEST_BOUNDARIES" 'private_deferred_retirement_reclaims_the_exact_slot_and_workload' \
    "CI must exercise deferred display retirement and slot reclamation"
assert_contains "$BUILD_DEBS" 'ensure_bin yuos-agentd yu' \
    "yuos-agent must build the agent worker"
assert_contains "$BUILD_DEBS" '/usr/local/bin/yuos-agentd' \
    "yuos-agent must install the agent worker beside yuosd"
assert_contains "$BUILD_DEBS" 'ensure_bin yuos-extension-host yu' \
    "yuos-agent must build the extension host"
assert_contains "$BUILD_DEBS" '/usr/local/bin/yuos-extension-host' \
    "yuos-agent must install the extension host beside yuos-agentd"
assert_contains "$BUILD_DEBS" '/usr/lib/yuos/extensions' \
    "yuos-agent must create the authenticated Agent extension package root"
assert_contains "$BUILD_DEBS" '/usr/lib/sysusers.d/yuos-agent.conf' \
    "yuos-agent must install its dedicated extension group definition"
assert_contains "$SYSUSERS" 'g yuos-extension - -' \
    "sysusers must preserve a safely retained legacy extension gid"
assert_contains "$POSTINST" 'systemd-sysusers /usr/lib/sysusers.d/yuos-agent.conf' \
    "postinst must create the extension execution group before starting yuosd"
assert_contains "$BUILD_DEBS" 'yuos-agent/preinst' \
    "yuos-agent must ship an identity-provisioning preinst"
assert_contains "$BUILD_DEBS" 'ensure_calendar_reader' \
    "Agent must build the independent Calendar reader"
assert_contains "$BUILD_DEBS" '--manifest-path desktop/services/yuos-applet-services/Cargo.toml' \
    "Calendar query code must remain in its existing standalone workspace"
assert_contains "$BUILD_DEBS" 'usr/lib/yuos/bin/yuos-calendar-reader' \
    "Agent must own the Calendar reader executable"
assert_contains "$BUILD_DEBS" 'calendar-reader.LICENSE' \
    "Agent must carry the Calendar reader GPL license"
assert_contains "$PROJECT_DIR/packaging/deb/yuos-framework/control" 'yuos-calendar-read-v1' \
    "Agent must provide the Calendar read service"
assert_contains "$PROJECT_DIR/packaging/deb/yuos-desktop/control" 'yuos-calendar-read-v1' \
    "Desktop must depend on the Calendar read service"
assert_contains "$TEST_WORKFLOW" 'bash scripts/ci/test-boundaries.sh calendar' \
    "CI must invoke the owner-scoped Calendar checks"
assert_contains "$TEST_BOUNDARIES" 'calendar_mcp_to_broker_reader_and_public_sdk' \
    "CI must cover actual Calendar data reaching the public SDK"
assert_contains "$BUILD_DEBS" 'extension-identities.sh' \
    "maintainer scripts must embed the shared identity policy"
assert_contains "$POSTINST" 'identity_provision upgrade "$2"' \
    "postinst must provision identities after dependencies and helper are unpacked"
if grep -Fq 'identity_provision "$1"' "$PREINST"; then
    fail "preinst must not run dependency-backed identity scans before unpack"
fi
provision_line=$(grep -n 'identity_provision upgrade "$2"' "$POSTINST" | cut -d: -f1)
sysusers_line=$(grep -n 'systemd-sysusers /usr/lib/sysusers.d/yuos-agent.conf' "$POSTINST" |
    cut -d: -f1)
[ "$provision_line" -lt "$sysusers_line" ] ||
    fail "postinst must reserve the exact gid before sysusers observes it"
assert_contains "$PREINST" 'deb-systemd-invoke stop yuosd.service' \
    "upgrade must stop yuosd before validating a legacy execution gid"
assert_contains "$POSTINST" 'deb-systemd-helper --quiet was-enabled yuosd.service' \
    "upgrade must distinguish an enabled broker from an administrator-disabled unit"
python3 - "$POSTINST" <<'PY'
import sys

body = open(sys.argv[1], encoding="utf-8").read()
upgrade = body.index('if [ -n "$2" ]; then')
enabled = body.index("deb-systemd-helper --quiet was-enabled yuosd.service", upgrade)
start = body.index("deb-systemd-invoke start yuosd.service", enabled)
assert upgrade < enabled < start
assert "deb-systemd-invoke try-restart yuos-browser.service" not in body
PY
assert_contains "$POSTINST" 'identity_finalize' \
    "postinst must validate and write the runtime reservation manifest"
assert_contains "$POSTRM" 'identity_purge_owned' \
    "purge must remove only identities proven package-owned"
assert_contains "$IDENTITY_HELPER" 'YUOS_EXT_UID_FIRST=61000' \
    "package identity range must start below systemd DynamicUser"
assert_contains "$IDENTITY_HELPER" 'YUOS_EXT_GID=60999' \
    "fresh installs must prefer the fixed reserved gid"
assert_contains "$IDENTITY_HELPER" 'identity_select_gid' \
    "upgrades must safely retain a provable legacy package gid"
assert_contains "$IDENTITY_HELPER" 'YUOS_EXT_DYNAMIC_UID_FIRST=61184' \
    "package policy must encode systemd DynamicUser boundaries"
assert_contains "$IDENTITY_RS" 'FIRST_UID: u32 = 61_000' \
    "runtime and packaged uid-range start must agree"
assert_contains "$IDENTITY_RS" 'GROUP_GID: u32 = 60_999' \
    "runtime and packaged gid must agree"
assert_contains "$IDENTITY_RS" 'IDENTITY_COUNT: u32 = 64' \
    "runtime and packaged identity count must agree"

# The staged worker path and the path the unit hands yuosd have to agree,
# or the daemon looks for a binary the package never installed.
assert_contains "$UNIT" 'YUOS_AGENTD_BIN=/usr/local/bin/yuos-agentd' \
    "yuosd.service must point at the installed agent worker"
assert_contains "$UNIT" 'YUOS_EXTENSION_HOST_BIN=/usr/local/bin/yuos-extension-host' \
    "yuosd.service must point at the installed extension host"
assert_contains "$UNIT" 'YUOS_EXTENSION_EXEC_GROUP=yuos-extension' \
    "yuosd.service must pin the dedicated extension execution group"
assert_contains "$UNIT" 'YUOSD_EXTENSION_HOST_NAMESPACES=on' \
    "yuosd.service must enable available extension-host namespaces"
grep -Eq '^Delegate=yes$' "$UNIT" ||
    fail "yuosd.service must delegate a cgroup subtree for extension cleanup"
grep -Eq '^KillMode=control-group$' "$UNIT" ||
    fail "yuosd.service must kill delegated extension cgroups on daemon stop"
assert_contains "$UNIT" 'YUOSD_AGENTD=on' \
    "yuosd.service must state whether agent supervision is enabled"

# Traversable, never listable: an unprivileged worker needs to walk to
# /var/lib/yuos/users/<uid>. Anything wider or narrower breaks the split.
grep -Eq '^StateDirectoryMode=0711$' "$UNIT" ||
    fail "yuosd.service must set StateDirectoryMode=0711 for owner-partitioned agent state"

# The broker socket stays root:sudo 0660 and the worker never joins that
# group, so no route on it is reachable from a worker.
grep -Eq '^Environment=YUOSD_SOCKET_MODE=0660$' "$UNIT" ||
    fail "yuosd.sock permissions must not be widened"
grep -Eq '^Group=root$' "$UNIT" ||
    fail "yuosd.service must create runtime and state roots as root:root"
grep -Eq '^Environment=YUOSD_SOCKET_GROUP=sudo$' "$UNIT" ||
    fail "yuosd.service must preserve root:sudo ownership for the primary socket"

assert_contains "$TEST_WORKFLOW" 'Run privileged extension boundaries' \
    "CI must execute the root extension boundary suites"
assert_contains "$TEST_WORKFLOW" 'bash scripts/ci/test-boundaries.sh extensions' \
    "CI must invoke the root extension boundary checks"
assert_contains "$TEST_BOUNDARIES" 'install -o root -g root -m 0755' \
    "privileged test binaries must use root-owned non-writable fixtures"
assert_contains "$TEST_BOUNDARIES" 'YUOS_PRIVILEGED_EXTENSION_HOST_BIN' \
    "installed-system tests must use the root-owned extension host fixture"
assert_contains "$TEST_BOUNDARIES" '"$root/extension_host_boundary" --test-threads=1' \
    "CI must run the repaired extension-host lifecycle test"
assert_contains "$TEST_BOUNDARIES" 'YUOS_REQUIRE_PRIVILEGED_CHILD_TESTS=1' \
    "CI must fail when a root-gated child-isolation test skips"
assert_contains "$TEST_BOUNDARIES" 'extension_host::child_isolation::tests' \
    "CI must run the root-gated child-isolation library tests"
assert_contains "$TEST_BOUNDARIES" 'expected_privileged_child_tests=19' \
    "CI must explicitly account for every privileged child-isolation test"
assert_contains "$TEST_BOUNDARIES" '"$lib_test" "$root/yu_lib_tests"' \
    "CI must copy the lib test binary into the root-owned fixture"
bash "$SCRIPT_DIR/test-extension-identities.sh"

printf 'ok - agent isolation packaging contract\n'
