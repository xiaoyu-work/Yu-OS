#!/usr/bin/env bash
# rootfs/features/yuos-core/install.sh -- install the Yu OS Framework, the
# distribution integration package and, unless YUOS_AGENT=0, the optional
# official Yu Agent into ROOTFS.
#
# The .deb is the canonical source of truth: this script just makes sure it
# exists (building it on the fly if needed) and lets dpkg/apt put the files
# in place. Use of `apt-get install /path/to/file.deb` lets required
# dependencies resolve from the chroot's apt sources.
#
# Inherited from environment: ROOTFS, PROJECT_DIR, YUOS_VERSION, YUOS_AGENT.

set -euo pipefail

DEBS_DIR="$PROJECT_DIR/build/debs"
if ! APP_PACKAGE_INPUTS="$(python3 "$PROJECT_DIR/scripts/app_packages.py" check \
    --variant agent --architecture "$DEB_ARCH" 2>/dev/null)"; then
    if [ "${YUOS_PREBUILT_SYSTEM_PACKAGES:-0}" = 1 ]; then
        echo "error: installer requires prepared official App packages for agent/$DEB_ARCH" >&2
        exit 1
    fi
    echo "  :: official App packages (agent/$DEB_ARCH) not prepared -- fetching them"
    APP_PACKAGE_INPUTS="$(python3 "$PROJECT_DIR/scripts/app_packages.py" prepare \
        --variant agent --architecture "$DEB_ARCH")"
fi

FILE_VERSION="${YUOS_VERSION//:/%3a}"
FRAMEWORK_DEB="$DEBS_DIR/yuos-framework_${FILE_VERSION}_${DEB_ARCH:-amd64}.deb"
AGENT_DEB="$DEBS_DIR/yuos-agent_${FILE_VERSION}_${DEB_ARCH:-amd64}.deb"
BASE_DEB="$DEBS_DIR/yuos-base_${FILE_VERSION}_all.deb"
# Yu OS images select the official Agent explicitly; YUOS_AGENT=0 builds a
# Framework-only system.
SYSTEM_DEBS=("$FRAMEWORK_DEB" "$BASE_DEB")
if [ "${YUOS_AGENT:-1}" = 1 ]; then
    SYSTEM_DEBS+=("$AGENT_DEB")
fi
# Build this exact version if missing — stale packages from an earlier local
# build must never satisfy the check.
for deb in "${SYSTEM_DEBS[@]}"; do
    if [ ! -f "$deb" ]; then
        if [ "${YUOS_PREBUILT_SYSTEM_PACKAGES:-0}" = 1 ]; then
            echo "error: installer requires the prepared Framework, Agent and Base packages for $YUOS_VERSION" >&2
            exit 1
        fi
        echo "  :: Yu OS system packages not found -- building them"
        "$PROJECT_DIR/packaging/deb/build-debs.sh"
        break
    fi
done

for deb in "${SYSTEM_DEBS[@]}"; do
    if [ ! -f "$deb" ]; then
        echo "error: expected package missing after build: $deb" >&2
        exit 1
    fi
done
echo "  :: installing ${SYSTEM_DEBS[*]##*/}"

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

# Stage the .debs inside the chroot and install them in one apt transaction.
mkdir -p "$ROOTFS/var/cache/yuos-debs"
cp "${SYSTEM_DEBS[@]}" "$ROOTFS/var/cache/yuos-debs/"
system_debs=()
for deb in "${SYSTEM_DEBS[@]}"; do
    system_debs+=("/var/cache/yuos-debs/$(basename "$deb")")
done
install -Dm0644 "$PROJECT_DIR/core/assets/app-store-archive-key.asc" \
    "$ROOTFS/usr/share/keyrings/yuos-apps-archive-key.asc"
install -Dm0644 "$PROJECT_DIR/packaging/deb/yuos-framework/yuos-apps.sources" \
    "$ROOTFS/etc/apt/sources.list.d/yuos-apps.sources"
install -Dm0644 "$PROJECT_DIR/packaging/deb/yuos-framework/helium-archive-key.asc" \
    "$ROOTFS/usr/share/keyrings/yuos-helium-archive-key.asc"
install -Dm0644 "$PROJECT_DIR/packaging/deb/yuos-framework/yuos-helium.sources" \
    "$ROOTFS/etc/apt/sources.list.d/yuos-helium.sources"
install -Dm0644 "$PROJECT_DIR/packaging/deb/yuos-framework/yuos-helium.pref" \
    "$ROOTFS/etc/apt/preferences.d/yuos-helium.pref"
mkdir -p "$ROOTFS/var/cache/yuos-debs/apps"
cp "$APP_PACKAGE_INPUTS"/archives/*.deb "$ROOTFS/var/cache/yuos-debs/apps/"
app_debs=()
for app_deb in "$APP_PACKAGE_INPUTS"/archives/*.deb; do
    app_debs+=("/var/cache/yuos-debs/apps/$(basename "$app_deb")")
done
chroot "$ROOTFS" apt-get update --error-on=any -qq
chroot "$ROOTFS" apt-get install -y --no-install-recommends \
    "${system_debs[@]}" helium-bin "${app_debs[@]}"
chroot "$ROOTFS" apt-get clean
rm -rf "$ROOTFS/var/cache/yuos-debs/apps"
rm -rf "$ROOTFS/var/lib/apt/lists"/*
