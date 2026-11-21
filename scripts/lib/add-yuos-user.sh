#!/usr/bin/env bash
# shellcheck shell=bash

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    echo "error: scripts/lib/add-yuos-user.sh must be sourced" >&2
    exit 1
fi

add_yuos_user() {
    local rootfs="${1:?add_yuos_user: rootfs path required}"
    chroot "$rootfs" /bin/bash -c '
        set -e
        if ! id yuos >/dev/null 2>&1; then
            useradd -m -u 1000 -s /bin/bash -G sudo yuos
            passwd --lock yuos >/dev/null
        fi
        rm -f /etc/sudoers.d/yuos
    '
}
