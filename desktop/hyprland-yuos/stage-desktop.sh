#!/usr/bin/env bash
set -euo pipefail

project="${1:?source checkout required}"
stage="${2:?package staging path required}"
work="${3:?build directory required}"
jobs="${CMAKE_BUILD_PARALLEL_LEVEL:-4}"

export PATH="/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
export CARGO_TARGET_DIR="$work/cargo"
rustup toolchain install stable --profile minimal

cargo +stable build --manifest-path "$project/Cargo.toml" --locked --release -p yuos-compositor
cargo +stable build --manifest-path "$project/desktop/agent/Cargo.toml" --locked --release --workspace
cargo +stable build --manifest-path "$project/yuos-runtime/rust/Cargo.toml" --locked --release \
    --bin yuos-ask-launcher
cargo +stable build --manifest-path "$project/desktop/services/yuos-applet-services/Cargo.toml" \
    --locked --release --features provider --bin yuos-applet-provider

source_adapter="$project/desktop/hyprland-yuos"
bash "$source_adapter/build-locker.sh" "$stage" "$work"
compiler="$(python3 "$source_adapter/build-toolchain.py" "$work")"
export CC="$compiler/bin/gcc" CXX="$compiler/bin/g++"
export LD_LIBRARY_PATH="$compiler/lib64:$compiler/lib"
toolchain="$("$CXX" -dumpfullversion)"
dependencies="$work/deps-gcc-$toolchain"
cmake_build="$work/compositor-gcc-$toolchain"
cp -a "$project/desktop/hyprland" "$source_adapter" "$work/"
python3 "$source_adapter/build-dependencies.py" --cache "$dependencies" --jobs "$jobs"
prefix="$dependencies/prefix"
export PATH="$prefix/bin:$PATH"
export CMAKE_PREFIX_PATH="$prefix"
export PKG_CONFIG_PATH="$prefix/lib/pkgconfig:$prefix/share/pkgconfig"
cmake --fresh -S "$work/hyprland" -B "$cmake_build" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr \
    -DPKG_CONFIG_EXECUTABLE=/usr/bin/env \
    '-DPKG_CONFIG_ARGN=-u;PKG_CONFIG_ALLOW_SYSTEM_LIBS;/usr/bin/pkg-config' \
    "-DYUOS_COMPOSITOR_LIBRARY=$CARGO_TARGET_DIR/release/libyuos_compositor.a" \
    -DNO_UWSM=ON -DBUILD_TESTING=OFF
cmake --build "$cmake_build" --parallel "$jobs" --target Hyprland

native="$stage/usr/libexec/yuos-hyprland"
install -Dm0755 "$cmake_build/Hyprland" "$native/Hyprland"
install -d "$native/lib"
cp -a "$prefix"/lib/*.so.* "$native/lib/"
# Use the same newer C++ runtime as the build, private to the compositor.
for library in libstdc++.so.6 libgcc_s.so.1; do
    file="$("$CXX" "-print-file-name=$library")"
    test "${file#"$compiler/"}" != "$file"
    install -m0644 "$file" "$native/lib/$library"
done
unset LD_LIBRARY_PATH
patchelf --set-rpath '$ORIGIN/lib' "$native/Hyprland"
find "$native/lib" -type f -name '*.so.*' -exec patchelf --set-rpath '$ORIGIN' {} +
if find "$native/lib" -xtype l -print -quit | grep -q .; then
    echo "error: staged Hyprland has a broken shared-library link" >&2
    exit 1
fi

share="$stage/usr/share/yuos-hyprland"
install -d "$share"
cp -a "$prefix/share/libinput" "$share/libinput"
hardware="$project/packaging/deb/yuos-base/hardware"
install -m0644 "$hardware/99-yuos-hardware.quirks" "$share/libinput/"
license="$stage/usr/share/doc/yuos-desktop"
install -Dm0644 "$hardware/omarchy.LICENSE" "$license/omarchy-hardware.LICENSE"
install -Dm0644 "$work/hyprland/LICENSE" "$license/hyprland/LICENSE"
install -Dm0644 "$work/hyprland/subprojects/udis86/LICENSE" "$license/hyprland/LICENSE-udis86"
install -Dm0644 /usr/share/common-licenses/GPL-3 "$license/hyprland/LICENSE-yuos-authority"
install -Dm0644 "$work/hyprland-yuos/dependencies.lock.json" "$license/hyprland/sources.json"
install -Dm0644 "$source_adapter/toolchain.lock.json" "$license/hyprland/gcc/sources.json"
install -m0644 "$compiler/yuos-notices/"* "$license/hyprland/gcc/"
for source in "$dependencies"/sources/*; do
    while IFS= read -r -d '' notice; do
        install -Dm0644 "$notice" "$license/hyprland/${source##*/}/${notice##*/}"
    done < <(find "$source" -maxdepth 1 -type f \
        \( -iname 'license*' -o -iname 'copying*' -o -iname 'copyright*' \) -print0)
done
install -Dm0644 "$dependencies/sources/lua-5.5.1/doc/readme.html" "$license/hyprland/lua/LICENSE.html"

for binary in yuos-agent-ui yuos-agent-bridge yuos-ask-launcher; do
    install -Dm0755 "$CARGO_TARGET_DIR/release/$binary" "$stage/usr/local/bin/$binary"
done
install -Dm0755 "$project/tools/browser-chat-native-host" \
    "$stage/usr/lib/yuos/browser-chat-native-host"
install -Dm0755 "$CARGO_TARGET_DIR/release/yuos-applet-provider" \
    "$stage/usr/libexec/yuos-applet-provider"
install -Dm0644 "$project/desktop/services/yuos-applet-services/LICENSE" "$license/applet-services.LICENSE"

session="$work/hyprland-yuos/session"
install -Dm0644 "$session/yuos.desktop" "$stage/usr/share/wayland-sessions/yuos.desktop"
install -Dm0644 "$session/hyprland.lua" "$stage/etc/xdg/hypr/hyprland.lua"
install -Dm0644 "$session/yuos-desktop-session.target" "$stage/usr/lib/systemd/user/yuos-desktop-session.target"
install -Dm0644 "$session/yuos-screen-reader.service" "$stage/usr/lib/systemd/user/yuos-screen-reader.service"
install -Dm0644 "$session/yuos-desktop.pam" "$stage/etc/pam.d/yuos-desktop"
install -Dm0644 "$session/desktop-session.env" "$stage/usr/share/yuos/desktop-session.env"
install -Dm0755 "$session/yuos-greeter" "$stage/usr/libexec/yuos-greeter"
install -Dm0755 "$session/yuos-desktop-login" "$stage/usr/libexec/yuos-desktop-login"
for name in yuos-system-settings yuos-updates yuos-setup-update; do
    install -Dm0755 "$session/$name" "$stage/usr/libexec/$name"
done
install -Dm0644 "$session/org.yuos.desktop.settings.policy" \
    "$stage/usr/share/polkit-1/actions/org.yuos.desktop.settings.policy"
install -Dm0644 "$session/org.yuos.setup-update.policy" \
    "$stage/usr/share/polkit-1/actions/org.yuos.setup-update.policy"
install -Dm0644 "$session/yuos-setup-update.rules" \
    "$stage/usr/share/polkit-1/rules.d/yuos-setup-update.rules"
units="$stage/usr/lib/systemd/system"
for file in "$session"/yuos-update-*.service "$session"/yuos-update-*.timer; do
    install -Dm0644 "$file" "$units/${file##*/}"
done
install -d "$units/system-update.target.wants" "$units/timers.target.wants"
ln -s ../yuos-update-offline.service "$units/system-update.target.wants/"
ln -s ../yuos-update-automatic.timer "$units/timers.target.wants/"
install -Dm0755 "$session/yuos-create-local-owner" "$stage/usr/libexec/yuos-create-local-owner"
install -Dm0755 "$session/yuos-desktop-locales" "$stage/usr/libexec/yuos-desktop-locales"
install -Dm0755 "$session/yuos-desktop-reviews" "$stage/usr/libexec/yuos-desktop-reviews"
install -Dm0644 "$session/greetd.toml" "$stage/etc/greetd/yuos-desktop.toml"
install -Dm0644 "$session/greetd.conf" "$stage/usr/lib/systemd/system/greetd.service.d/yuos-desktop.conf"
install -Dm0644 "$session/yuos-desktop-accounts.conf" \
    "$units/yuosd.service.d/yuos-desktop-accounts.conf"
install -Dm0644 "$session/yuos-desktop-users.conf" "$stage/usr/lib/sysusers.d/yuos-desktop-users.conf"
install -Dm0644 "$session/hyprland-portals.conf" "$stage/usr/share/xdg-desktop-portal/hyprland-portals.conf"
install -Dm0755 "$session/yuos-oobe" "$stage/usr/libexec/yuos-oobe"
for module in models regions updates; do
    install -Dm0644 "$session/yuos_oobe_${module}.py" "$stage/usr/libexec/yuos_oobe_${module}.py"
done
for file in "$session"/oobe/*; do
    install -Dm0644 "$file" "$stage/usr/share/yuos/oobe/${file##*/}"
done
for file in "$project"/assets/brand/avatars/*.png; do
    install -Dm0644 "$file" "$stage/usr/share/yuos/avatars/${file##*/}"
done
install -Dm0644 "$project/assets/brand/yuos-icon-192.png" "$stage/usr/share/yuos/oobe/logo.png"
install -Dm0644 "$project/assets/brand/wallpaper/yuos-default.jpg" "$stage/usr/share/backgrounds/yuos/yuos-default.jpg"
install -Dm0644 "$project/assets/brand/wallpaper/yuos-default-blurred.jpg" "$stage/usr/share/yuos/oobe/wallpaper-blurred.jpg"
install -Dm0644 "$project/assets/brand/wallpaper/NOTICE" "$license/wallpaper.NOTICE"
install -Dm0644 "$project/LICENSE" "$license/wallpaper.LICENSE"
install -Dm0644 "$session/yuos-driver-online.service" "$stage/usr/lib/systemd/system/yuos-driver-online.service"
install -Dm0755 "$session/70-yuos-driver-setup" "$stage/usr/lib/NetworkManager/dispatcher.d/70-yuos-driver-setup"

shell_build="$work/system-desktop-build"
shell_install="$work/system-desktop-install"
shell_root="$stage/usr/lib/yuos/system-desktop"
(
    unset CMAKE_PREFIX_PATH PKG_CONFIG_PATH
    export CC=/usr/bin/cc CXX=/usr/bin/c++ INSTALL_DIR=/usr/share
    export PATH=/usr/bin:/bin:/usr/sbin:/sbin
    cmake --fresh -S "$project/desktop/desktop-shell/native" -B "$shell_build" -G Ninja \
        -DCMAKE_BUILD_TYPE=Release "-DGIT_REVISION=${YUOS_VERSION:?package version required}" \
        -DBUILD_TESTING=OFF "-DYUOS_QT_CACHE=$work/system-desktop-qt" "-DYUOS_QT_JOBS=$jobs"
    cmake --build "$shell_build" --parallel "$jobs"
    install -d "$shell_install"
    DESTDIR="$shell_install" cmake --install "$shell_build" --prefix /usr
)
install -Dm0755 "$shell_install/usr/bin/yuos-desktop-shell" "$shell_root/bin/yuos-desktop-shell"
install -Dm0644 "$project/desktop/desktop-shell/apps/desktop-shell/app.json" "$shell_root/app.json"
install -d "$shell_root/resources/usr/share"
cp -a "$shell_install/usr/share/yuos-desktop-shell" "$shell_root/resources/usr/share/"
install -Dm0644 "$project/desktop/desktop-shell/native/LICENSE" \
    "$shell_root/resources/usr/share/yuos-desktop-shell/licenses/yuos-desktop-shell.LICENSE"
install -Dm0644 "$project/desktop/desktop-shell/PROVENANCE.md" \
    "$shell_root/resources/usr/share/yuos-desktop-shell/licenses/PROVENANCE.md"
install -Dm0644 "$project/desktop/desktop-shell/upstream.lock.json" \
    "$shell_root/resources/usr/share/yuos-desktop-shell/licenses/upstream.lock.json"
python3 "$project/scripts/system_desktop_tree.py" --stage "$stage"

# Query the target distro's shlibs database, not that of the image builder.
cd "$work"
mkdir -p debian
printf 'Source: yuos\n\nPackage: yuos-desktop\nArchitecture: any\nDescription: Yu OS Desktop\n' >debian/control
mapfile -d '' libraries < <(find "$native/lib" -type f -name '*.so.*' -print0)
mapfile -d '' shell_elfs < <(python3 - "$shell_root" <<'PY'
import os
from pathlib import Path
import sys

for path in sorted(Path(sys.argv[1]).rglob("*")):
    if path.is_file() and not path.is_symlink():
        with path.open("rb") as source:
            if source.read(4) == b"\x7fELF":
                sys.stdout.buffer.write(os.fsencode(path) + b"\0")
PY
)
shell_libargs=()
declare -A shell_libdirs=()
for file in "${shell_elfs[@]}"; do
    directory="${file%/*}"
    if [[ ! ${shell_libdirs[$directory]+present} ]]; then
        shell_libdirs["$directory"]=1
        shell_libargs+=("-l$directory")
    fi
done
for library in "${libraries[@]}" "${shell_elfs[@]}"; do
    objdump -p "$library" | sed -nE 's/^[[:space:]]+SONAME[[:space:]]+(.+)\.so\.(.+)$/\1 \2 yuos-desktop/p'
done | LC_ALL=C sort -u >debian/shlibs.local
mkdir "$stage/DEBIAN"
dpkg-shlibdeps -O -l"$native/lib" "${shell_libargs[@]}" -xyuos-desktop "$native/Hyprland" "${libraries[@]}" \
    "${shell_elfs[@]}" \
    "$native/yuos-lockscreen" \
    "$stage/usr/local/bin/yuos-agent-ui" "$stage/usr/local/bin/yuos-agent-bridge" \
    "$stage/usr/local/bin/yuos-ask-launcher" "$stage/usr/libexec/yuos-applet-provider" \
    | sed -n 's/^shlibs:Depends=//p' >"$work/runtime-depends"
rmdir "$stage/DEBIAN"
test -s "$work/runtime-depends"
