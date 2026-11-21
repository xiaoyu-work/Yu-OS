#!/bin/bash
# packaging/deb/tests/test-session-journal-packaging.sh -- the session
# event journal must be installed root-owned and private, and an upgrade
# must not disturb an existing chain.
#
# The journal is the machine's evidence that a privileged mutation ran,
# and its MAC keys are what make that evidence unforgeable by a local
# unprivileged attacker. Both properties are decided by directory
# ownership and mode at install time, so they are contract-checked here
# rather than left to the first daemon start.

set -euo pipefail

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/../../.." && pwd)

POSTINST="$PROJECT_DIR/packaging/deb/yuos-agent/postinst"
POSTRM="$PROJECT_DIR/packaging/deb/yuos-agent/postrm"
STORAGE="$PROJECT_DIR/core/src/storage.rs"

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

bash -n "$POSTINST" || fail "yuos-agent postinst is not valid sh"
bash -n "$POSTRM" || fail "yuos-agent postrm is not valid sh"

assert_contains "$POSTINST" 'install -d -m 0700 -o root -g root "$DPKG_ROOT/var/lib/yuos/journal"' \
    "the journal root must be created root-owned and private"
assert_contains "$POSTINST" \
    'install -d -m 0700 -o root -g root "$DPKG_ROOT/var/lib/yuos/journal/keys"' \
    "the journal MAC keys must live in a root-only directory"

# `install -d` re-asserts the mode on upgrade; it must never remove the
# chain, the head anchor or the keys, because that would be
# indistinguishable from tampering to the next daemon start.
if grep -Eq 'rm -rf?[^\n]*/var/lib/yuos/journal' "$POSTINST" "$POSTRM"; then
    fail "package scripts must not delete the session journal"
fi

# The daemon re-hardens the same tree on every start, so a directory
# somebody widened between upgrades is closed again before the socket
# opens.
assert_contains "$STORAGE" 'data.join("journal")' \
    "harden_yuosd_state must cover the session journal tree"

printf 'ok - session journal packaging contract\n'
