#!/usr/bin/env bash
set -euo pipefail

cd "${GITHUB_WORKSPACE:?GITHUB_WORKSPACE is required}"
: "${RUNNER_TEMP:?RUNNER_TEMP is required}"
: "${YUOS_PACKAGE_VERSION:?YUOS_PACKAGE_VERSION is required}"
: "${YUOS_RELEASE_SECURITY_KEYRING:?YUOS_RELEASE_SECURITY_KEYRING is required}"
component="${1:-}"
case "$component" in
    agent|desktop) arch="${ARCH:?ARCH is required}" ;;
    base) arch=all ;;
    *) echo "usage: $0 {agent|base|desktop}" >&2; exit 2 ;;
esac
file_version="${YUOS_PACKAGE_VERSION//:/%3a}"
deb="build/debs/yuos-${component}_${file_version}_${arch}.deb"
test -f "$deb"
test "$(dpkg-deb -f "$deb" Package)" = "yuos-$component"
test "$(dpkg-deb -f "$deb" Version)" = "$YUOS_PACKAGE_VERSION"
test "$(dpkg-deb -f "$deb" Architecture)" = "$arch"

extract="$(mktemp -d "$RUNNER_TEMP/yuos-package-check.XXXXXX")"
trap 'rm -rf -- "$extract"' EXIT
files="$extract/files"
dpkg-deb --fsys-tarfile "$deb" | tar -tf - > "$files"
required=()
case "$component" in
    agent)
        required=(
            usr/local/bin/yu
            usr/local/bin/yuosd
            usr/local/bin/yuos-agentd
            usr/local/bin/yuos-extension-host
            usr/lib/yuos/bin/yuos-calendar-reader
            usr/share/doc/yuos-agent/calendar-reader.LICENSE
            usr/lib/yuos/bin/yuos-nearby-worker
            usr/lib/yuos/bin/yuos-nearby-radio
            usr/share/doc/yuos-agent/nearby-owly.LICENSE
            usr/share/doc/yuos-agent/nearby-PROVENANCE.md
            usr/share/doc/yuos-agent/vendor/owlyd/PROVENANCE.md
            usr/share/doc/yuos-agent/vendor/owlyd/LICENSE
            usr/lib/systemd/system/yuos-nearby-radio.service
            usr/share/dbus-1/system-services/io.github.t4t5.Owly.service
            usr/share/dbus-1/system.d/io.github.t4t5.Owly.conf
            usr/lib/NetworkManager/conf.d/90-yuos-nearby-unmanaged.conf
            usr/lib/systemd/network/10-yuos-nearby-awdl0.link
            usr/local/bin/yuos-gui-runner
            usr/local/bin/yuos-display-session
            usr/lib/yuos/bin/yuos-display-host
            "usr/lib/$(dpkg-architecture -a"$ARCH" -qDEB_HOST_MULTIARCH)/security/pam_yuos_display.so"
            usr/lib/sysusers.d/yuos-agent.conf
            usr/local/bin/yuos-semantic
            usr/local/bin/yuos-semantic-daemon
            usr/lib/yuos/python/yuos_sdk/__init__.py
            usr/lib/yuos/python/yuos_runtime/__init__.py
            usr/share/keyrings/yuos-apps-archive-key.asc
            etc/apt/sources.list.d/yuos-apps.sources
            usr/share/keyrings/yuos-helium-archive-key.asc
            etc/apt/sources.list.d/yuos-helium.sources
            etc/apt/preferences.d/yuos-helium.pref
            usr/lib/yuos/bin/yuos-app-apt-hook
            usr/lib/yuos/bin/yuos-notification-panel
            usr/lib/yuos/app-apt-metadata.py
            usr/lib/yuos/skills/yuos/SKILL.md
            usr/lib/yuos/skills/yuos/diagnostics.md
            usr/lib/systemd/system/yuosd.service
            usr/lib/yuos/bin/yuos-security-floor
            usr/lib/yuos/apt/security-floor-hook
            etc/apt/apt.conf.d/50yuos-security-floor
        )
        dpkg-deb -f "$deb" Provides | grep -qw yuos-calendar-read-v1
        dpkg-deb -f "$deb" Provides | grep -qw yuos-nearby-v1
        dpkg-deb -f "$deb" Provides | grep -qw yuos-display-session-v3
        if dpkg-deb -f "$deb" Provides | grep -qw yuos-display-session-v2; then
            echo "error: Agent still advertises the incompatible display session v2" >&2
            exit 1
        fi
        dpkg-deb -f "$deb" Provides | grep -qw yuos-browser-render-v1
        if grep -Eq '/(yuos-browser(-worker)?|yuos-browser\.service)$' "$files"; then
            echo "error: legacy raw browser service is still packaged" >&2
            exit 1
        fi
        if grep -Fxq './etc/yuos/oauth.conf' "$files" \
            || grep -q '/usr/lib/yuos/apps/' "$files"; then
            echo "error: Agent contains excluded OAuth configuration or App payloads" >&2
            exit 1
        fi
        dpkg-deb -x "$deb" "$extract/payload"
        test "$(stat -c '%a' "$extract/payload/usr/lib/yuos/skills/yuos")" = "755"
        test "$(stat -c '%a' "$extract/payload/usr/lib/yuos/skills/yuos/SKILL.md")" = "644"
        runtime_version="$(python3 scripts/app_packages.py runtime-version)"
        dpkg-deb -f "$deb" Provides | grep -Fq "yuos-app-runtime-v1 (= $runtime_version)"
        ;;
    base)
        required=(usr/local/bin/yuos-init usr/lib/systemd/system/yuos-home-setup.service)
        if grep -Fxq './usr/local/bin/yu' "$files" \
            || grep -q '/usr/lib/yuos/apps/' "$files"; then
            echo "error: Base contains Agent-owned binaries or Apps" >&2
            exit 1
        fi
        ;;
    desktop)
        dpkg-deb -f "$deb" Depends \
            | grep -Eq '(^|,)[[:space:]]*yuos-display-session-v3([[:space:]]*,|[[:space:]]*$)'
        if dpkg-deb -f "$deb" Depends | grep -qw yuos-display-session-v2; then
            echo "error: Desktop still accepts the incompatible display session v2" >&2
            exit 1
        fi
        required=(
            usr/libexec/yuos-hyprland/Hyprland
            usr/lib/yuos/system-desktop/bin/yuos-desktop-shell
            usr/lib/yuos/system-desktop/app.json
            usr/lib/yuos/release-security/yuos-desktop/system-desktop-tree.json
            usr/libexec/yuos-greeter
            usr/libexec/yuos-create-local-owner
            usr/libexec/yuos-desktop-locales
            usr/libexec/yuos-oobe
            usr/libexec/yuos_oobe_models.py
            usr/libexec/yuos_oobe_regions.py
            usr/libexec/yuos_oobe_updates.py
            usr/libexec/yuos-setup-update
            usr/lib/systemd/system/yuos-update-setup.service
            usr/lib/systemd/system/yuos-driver-online.service
            usr/lib/NetworkManager/dispatcher.d/70-yuos-driver-setup
            usr/share/yuos/oobe/greeter.qml
            usr/share/yuos/oobe/shell.qml
            usr/share/yuos/oobe/SetupSystemUpdate.qml
            usr/share/yuos/oobe/SetupHint.qml
            usr/share/yuos/oobe/SetupModels.qml
            usr/share/yuos/oobe/SetupWelcome.qml
            usr/share/yuos/oobe/appearance-light.png
            usr/share/yuos/oobe/appearance-dark.png
            usr/share/yuos/oobe/WorldDots.js
            usr/share/yuos/oobe/wallpaper-blurred.jpg
            usr/share/backgrounds/yuos/yuos-default.jpg
            usr/share/polkit-1/actions/org.yuos.local-desktop-setup.policy
            usr/share/polkit-1/rules.d/yuos-local-desktop-setup.rules
            usr/share/polkit-1/actions/org.yuos.setup-update.policy
            usr/share/polkit-1/rules.d/yuos-setup-update.rules
            etc/xdg/hypr/hyprland.lua
            usr/share/wayland-sessions/yuos.desktop
            etc/pam.d/yuos-desktop
            etc/greetd/yuos-desktop.toml
            usr/local/bin/yuos-agent-ui
            usr/local/bin/yuos-agent-bridge
            usr/local/bin/yuos-ask-launcher
            usr/share/applications/com.yuos.Agent.desktop
        )
        ;;
esac
for path in "${required[@]}"; do
    if ! grep -Fxq "./$path" "$files"; then
        echo "error: $deb is missing $path" >&2
        exit 1
    fi
done

if [ "$component" != base ]; then
    if [ "$component" = desktop ]; then
        dpkg-deb -x "$deb" "$extract/payload"
        python3 scripts/system_desktop_tree.py --stage "$extract/payload" --check
        if [ -e "$extract/payload/usr/lib/yuos/apps/desktop-shell" ]; then
            echo "error: Desktop still packages the old App shell" >&2
            exit 1
        fi
    fi
    python3 scripts/app_packages.py check --variant "$component" \
        --architecture "$arch" --stage "$extract/payload" --preinstall-policy verify >/dev/null
fi

bash packaging/release-security/verify-package-manifest.sh "$deb" \
    --arch "$arch" --require-signature \
    --keyring "$YUOS_RELEASE_SECURITY_KEYRING"
