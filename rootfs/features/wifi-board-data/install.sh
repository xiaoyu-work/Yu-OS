#!/usr/bin/env bash
set -euo pipefail

work=$(mktemp -d "$ROOTFS/tmp/yuos-radio.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
chmod 0755 "$work"
python3 "$PROJECT_DIR/scripts/qualcomm_radio.py" --cache "$work/cache" --output "$work"
inside=${work#"$ROOTFS"}
chroot "$ROOTFS" apt-get -qq update
chroot "$ROOTFS" env DEBIAN_FRONTEND=noninteractive apt-get -o Dpkg::Use-Pty=0 \
    install -y --no-install-recommends "$inside/yuos-qualcomm-radio_1-1_all.deb" </dev/null
systemctl --root="$ROOTFS" enable yuos-radio-board-data.service
