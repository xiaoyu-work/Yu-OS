#!/usr/bin/env bash
# rootfs/features/yuos-core/install.sh -- install the shared agent and the
# Yu OS distribution integration package into ROOTFS.
#
# The .deb is the canonical source of truth: this script just makes sure it
# exists (building it on the fly if needed) and lets dpkg/apt put the files
# in place. Use of `apt-get install /path/to/file.deb` lets required
# dependencies resolve from the chroot's apt sources.
#
# Inherited from environment: ROOTFS, PROJECT_DIR, YUOS_VERSION.

set -euo pipefail

DEBS_DIR="$PROJECT_DIR/build/debs"
APP_PACKAGE_INPUTS="$(python3 "$PROJECT_DIR/scripts/app_packages.py" check \
    --variant agent --architecture "$DEB_ARCH")"

FILE_VERSION="${YUOS_VERSION//:/%3a}"
AGENT_DEB="$DEBS_DIR/yuos-agent_${FILE_VERSION}_${DEB_ARCH:-amd64}.deb"
BASE_DEB="$DEBS_DIR/yuos-base_${FILE_VERSION}_all.deb"
# Build this exact version if missing — stale packages from an earlier local
# build must never satisfy the check.
if [ ! -f "$AGENT_DEB" ] || [ ! -f "$BASE_DEB" ]; then
    if [ "${CLAW_PREBUILT_SYSTEM_PACKAGES:-0}" = 1 ]; then
        echo "error: installer requires the prepared Agent and Base packages for $YUOS_VERSION" >&2
        exit 1
    fi
    echo "  :: Yu OS agent/base packages not found -- building them"
    "$PROJECT_DIR/packaging/deb/build-debs.sh"
fi

for deb in "$AGENT_DEB" "$BASE_DEB"; do
    if [ ! -f "$deb" ]; then
        echo "error: expected package missing after build: $deb" >&2
        exit 1
    fi
done
echo "  :: installing $(basename "$AGENT_DEB") + $(basename "$BASE_DEB")"

# The base overlay (applied in step 2 of rootfs/build.sh) ships these
# same files; dpkg refuses to overwrite unowned files by default. Remove
# them so the package can take ownership.
rm -f \
    "$ROOTFS/usr/local/bin/yuos-init" \
    "$ROOTFS/usr/local/bin/yu" \
    "$ROOTFS/usr/lib/yuos/init/setup-home.sh" \
    "$ROOTFS/etc/yuos/profile.sh"
rm -rf \
    "$ROOTFS/usr/lib/yuos/skills"

# Stage both .debs inside the chroot and install them in one apt transaction.
mkdir -p "$ROOTFS/var/cache/yuos-debs"
cp "$AGENT_DEB" "$BASE_DEB" "$ROOTFS/var/cache/yuos-debs/"
install -Dm0644 "$PROJECT_DIR/core/assets/app-store-archive-key.asc" \
    "$ROOTFS/usr/share/keyrings/yuos-apps-archive-key.asc"
install -Dm0644 "$PROJECT_DIR/packaging/deb/yuos-agent/claw-apps.sources" \
    "$ROOTFS/etc/apt/sources.list.d/claw-apps.sources"
install -Dm0644 "$PROJECT_DIR/packaging/deb/yuos-agent/helium-archive-key.asc" \
    "$ROOTFS/usr/share/keyrings/claw-helium-archive-key.asc"
install -Dm0644 "$PROJECT_DIR/packaging/deb/yuos-agent/claw-helium.sources" \
    "$ROOTFS/etc/apt/sources.list.d/claw-helium.sources"
install -Dm0644 "$PROJECT_DIR/packaging/deb/yuos-agent/claw-helium.pref" \
    "$ROOTFS/etc/apt/preferences.d/claw-helium.pref"
mkdir -p "$ROOTFS/var/cache/yuos-debs/apps"
cp "$APP_PACKAGE_INPUTS"/archives/*.deb "$ROOTFS/var/cache/yuos-debs/apps/"
app_debs=()
for app_deb in "$APP_PACKAGE_INPUTS"/archives/*.deb; do
    app_debs+=("/var/cache/yuos-debs/apps/$(basename "$app_deb")")
done
chroot "$ROOTFS" apt-get update --error-on=any -qq
chroot "$ROOTFS" apt-get install -y --no-install-recommends \
    "/var/cache/yuos-debs/$(basename "$AGENT_DEB")" \
    "/var/cache/yuos-debs/$(basename "$BASE_DEB")" helium-bin "${app_debs[@]}"
chroot "$ROOTFS" apt-get clean
rm -rf "$ROOTFS/var/cache/yuos-debs/apps"
rm -rf "$ROOTFS/var/lib/apt/lists"/*
