#!/usr/bin/env bash
set -euo pipefail

cd "${GITHUB_WORKSPACE:?GITHUB_WORKSPACE is required}"
: "${RUNNER_TEMP:?RUNNER_TEMP is required}"
source scripts/lib/arch.sh

tui() {
    if [ -e /proc/sys/kernel/apparmor_restrict_unprivileged_userns ]; then
        sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
    fi
    yu_fixture="$(mktemp /tmp/yuos-agent-tui.XXXXXX)"
    trap 'rm -f "$yu_fixture"' EXIT
    install -m 0755 "target/$RUST_TARGET/release/yu" "$yu_fixture"
    original_namespace="$(readlink /proc/self/ns/mnt)"
    for scenario in complete slow-progress cancel commands confirmations durable-queue task-center task-controls approval-center approval-choice attachments appearance agents side platform memory-center hooks-center mcp-center extensions-center usage-center debug-center account-center voice-settings file-mentions copy-export raw-scrollback vim backtrack notification-inbox activity-lifecycle activity-controls activity-evidence activity-review workspace multiline multiline-key reconnect startup-reconnect resume resume-running plain; do
        unshare --user --map-current-user --keep-caps --mount --net \
            python3 -B core/tests/agent_tui_pty.py \
            --yu "$yu_fixture" \
            --case "$scenario" \
            --original-mount-namespace "$original_namespace" \
            --transcript "/tmp/yuos-tui-$scenario.log"
    done
}

apt_install() {
    # Keep the real postinst scan away from transient build caches.
    sudo unshare --mount --propagation private \
        /bin/bash -euo pipefail -c '
            workspace=$1
            cargo_home=$2
            rustup_home=$3
            shift 3
            cd /tmp
            for path in "$workspace" "$cargo_home" "$rustup_home"; do
                if [ -d "$path" ]; then
                    mount -t tmpfs -o nodev,nosuid,noexec,size=1m tmpfs "$path"
                fi
            done
            apt-get install -y --no-install-recommends "$@"
            apt-get install -y --reinstall --no-install-recommends "$@"
        ' bash "$GITHUB_WORKSPACE" "$CARGO_HOME" \
        "${RUSTUP_HOME:-$HOME/.rustup}" "$@"
}

wait_for_yuosd() {
    for attempt in {1..30}; do
        if sudo systemctl is-active --quiet yuosd.service \
            && sudo test -S /run/yuos/yuosd.sock; then
            break
        fi
        sleep 1
    done
    if ! sudo systemctl is-enabled --quiet yuosd.service \
        || ! sudo systemctl is-active --quiet yuosd.service \
        || ! sudo test -S /run/yuos/yuosd.sock; then
        sudo systemctl status yuosd.service --no-pager || true
        sudo journalctl -u yuosd.service --no-pager -n 100 || true
        exit 1
    fi
}

installed() {
    : "${YUOS_PACKAGE_VERSION:?YUOS_PACKAGE_VERSION is required}"
    file_version="${YUOS_PACKAGE_VERSION//:/%3a}"
    install_debs=()
    trap 'rm -f -- "${install_debs[@]}"' EXIT
    stage_deb() {
        local copy
        copy="$(mktemp "/tmp/$1.XXXXXX.deb")"
        install_debs+=("$copy")
        install -m 0644 "build/debs/$1_${file_version}_$2.deb" "$copy"
    }
    stage_deb yuos-framework "$ARCH"
    if [ "${1:-}" = core ]; then
        stage_deb yuos-base all
        # The Framework and Base run without the optional Agent.
        apt_install "${install_debs[@]}"
        wait_for_yuosd
        sudo /usr/lib/yuos/bin/yuos-security-floor verify-installed
        test ! -e /usr/local/bin/yuos-agentd
        if output="$(/usr/local/bin/yu agent skills list 2>&1)"; then
            echo "error: Agent commands succeeded without the Agent installed" >&2
            exit 1
        fi
        grep -Fq "Yu Agent is not installed" <<<"$output"
    fi
    stage_deb yuos-agent "$ARCH"
    apt_install "${install_debs[@]}"
    wait_for_yuosd
    test -x /usr/bin/pkexec
    test -x /usr/bin/pkttyagent
    test -x /usr/local/bin/yuos-approval-helper
    test -x /usr/local/bin/yuos-admin-approval-helper
    sudo /usr/lib/yuos/bin/yuos-security-floor verify-installed
    original_mount="$(readlink /proc/self/ns/mnt)"
    original_pid="$(readlink /proc/self/ns/pid)"
    original_network="$(readlink /proc/self/ns/net)"
    sudo unshare --mount --pid --fork --mount-proc --net \
        python3 -B core/tests/approval_polkit_process.py \
        --yu /usr/local/bin/yu \
        --yuosd /usr/local/bin/yuosd \
        --helper /usr/local/bin/yuos-approval-helper \
        --policy /usr/share/polkit-1/actions/org.yuos.approval.policy \
        --original-mount-namespace "$original_mount" \
        --original-pid-namespace "$original_pid" \
        --original-network-namespace "$original_network"
    test ! -e /usr/local/bin/yuos-browser
    YUOS_USER_DATA_DIR="$RUNNER_TEMP/yuos-user-data" \
        /usr/local/bin/yu agent skills list \
        | python3 -c 'import json,sys; data=json.load(sys.stdin); assert "yuos" in data["names"]'
    YUOS_USER_DATA_DIR="$RUNNER_TEMP/yuos-user-data" \
        /usr/local/bin/yu agent skills info yuos \
        | python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["source"] == "builtin"'
    timeout 15s sudo /usr/local/bin/yu agent service status \
        | python3 -c 'import json,sys; data=json.load(sys.stdin); assert data["status"] == "running", data; assert data["daemon"] == "yuosd", data'
    service_cgroup=/sys/fs/cgroup/system.slice/yuosd.service
    main_pid="$(systemctl show yuosd.service --property MainPID --value)"
    test -n "$main_pid"
    test -z "$(sudo cat "$service_cgroup/cgroup.procs")"
    sudo grep -qx "$main_pid" "$service_cgroup/yuos-broker/cgroup.procs"
    for controller in cpu memory pids; do
        sudo grep -qw "$controller" "$service_cgroup/cgroup.subtree_control"
    done
}

case "${1:-}" in
    tui) tui ;;
    installed) installed ;;
    installed-core) installed core ;;
    *) echo "usage: $0 {tui|installed|installed-core}" >&2; exit 2 ;;
esac
