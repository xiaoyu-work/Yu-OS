#!/usr/bin/env bash
set -euo pipefail

if ! APP_PACKAGE_INPUTS="$(python3 "$PROJECT_DIR/scripts/app_packages.py" check \
    --variant desktop --architecture "$DEB_ARCH" 2>/dev/null)"; then
    if [[ "${YUOS_PREBUILT_SYSTEM_PACKAGES:-0}" == 1 ]]; then
        echo "error: installer requires prepared official App packages for desktop/$DEB_ARCH" >&2
        exit 1
    fi
    echo "  :: official App packages (desktop/$DEB_ARCH) not prepared -- fetching them"
    APP_PACKAGE_INPUTS="$(python3 "$PROJECT_DIR/scripts/app_packages.py" prepare \
        --variant desktop --architecture "$DEB_ARCH")"
fi

FILE_VERSION="${YUOS_VERSION//:/%3a}"
DESKTOP_DEB="$PROJECT_DIR/build/debs/yuos-desktop_${FILE_VERSION}_${DEB_ARCH}.deb"
install -d "$ROOTFS/build"
work="$(mktemp -d "$ROOTFS/build/yuos-desktop.XXXXXX")"
inside="${work#"$ROOTFS"}"
stage="$work/package"
cleanup() {
    if mountpoint -q "$work/source"; then
        umount "$work/source" || return 1
    fi
    rm -rf -- "$work"
}
trap cleanup EXIT

if [[ "${YUOS_PREBUILT_SYSTEM_PACKAGES:-0}" == 1 ]]; then
    if [[ "${DESKTOP_PACKAGE_ONLY:-0}" == 1 || ! -f "$DESKTOP_DEB" ]]; then
        echo "error: installer requires the prepared Desktop package for $YUOS_VERSION" >&2
        exit 1
    fi
else
    install -d "$work/source" "$stage"
    mount --bind "$PROJECT_DIR" "$work/source"
    mount --make-private "$work/source"
    mount -o remount,bind,ro "$work/source"
    (umask 022; cp -a --no-preserve=mode,ownership "$SCRIPT_DIR/features/desktop/overlay/." "$stage/")

    echo "  :: building Yu OS Hyprland and desktop services against Debian $SUITE"
    chroot "$ROOTFS" /bin/bash "$inside/source/desktop/hyprland-yuos/stage-desktop.sh" \
        "$inside/source" "$inside/package" "$inside"
    YUOS_DESKTOP_RUNTIME_DEPS="$(cat "$work/runtime-depends")" \
        "$PROJECT_DIR/packaging/deb/build-desktop-deb.sh" "$stage"

    if [[ "${DESKTOP_PACKAGE_ONLY:-0}" == 1 ]]; then
        exit 0
    fi
fi

install -m0644 "$DESKTOP_DEB" "$work/desktop.deb"
install -d "$work/apps"
cp -- "$APP_PACKAGE_INPUTS"/archives/*.deb "$work/apps/"
app_debs=()
for app_deb in "$work"/apps/*.deb; do
    app_debs+=("${app_deb#"$ROOTFS"}")
done
chroot "$ROOTFS" apt-get update --error-on=any -qq
chroot "$ROOTFS" env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    "$inside/desktop.deb" "${app_debs[@]}"
test "$(chroot "$ROOTFS" dpkg-query -W -f='${db:Status-Status}' yuos-desktop)" = installed
if [[ "${YUOS_AGENT:-1}" == 1 ]]; then
    test "$(chroot "$ROOTFS" dpkg-query -S /usr/local/bin/yuos-agent-ui)" = "yuos-agent: /usr/local/bin/yuos-agent-ui"
fi
test -x "$ROOTFS/usr/libexec/yuos-hyprland/Hyprland"
test -x "$ROOTFS/usr/libexec/yuos-hyprland/yuos-lockscreen"
test -x "$ROOTFS/usr/libexec/yuos-greeter"
test -x "$ROOTFS/usr/libexec/yuos-create-local-owner"
test -x "$ROOTFS/usr/libexec/yuos-desktop-reviews"
test -s "$ROOTFS/usr/share/backgrounds/yuos/yuos-default.jpg"
test -f "$ROOTFS/etc/pam.d/yuos-desktop-lock"
shell_manifest=/usr/lib/yuos/system-desktop/app.json
test "$(chroot "$ROOTFS" dpkg-query -S "$shell_manifest")" = "yuos-desktop: $shell_manifest"
python3 "$PROJECT_DIR/scripts/system_desktop_tree.py" --stage "$ROOTFS" --check
test -f "$ROOTFS/usr/share/wayland-sessions/yuos.desktop"
test "$(readlink "$ROOTFS/etc/systemd/system/display-manager.service")" = /usr/lib/systemd/system/greetd.service
echo "  :: installed the authenticated Hyprland desktop and OS Desktop Shell"
