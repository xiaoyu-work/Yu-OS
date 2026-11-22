# Building the Yu OS Desktop Image

The desktop profile builds Hyprland and the graphical Agent against Debian 13.
`yuos-desktop` embeds the signed Desktop Shell (Quickshell, Caelestia and private
Qt). The OS release workflow resolves `yuos-app/main` once, builds that exact
revision natively on Debian 13 for amd64 and arm64, and seals the payload with
the official Yu OS App publisher. There is no separate Shell release, version
to enter, or package to install. Every other Yu OS App still comes from its
own signed APT package. Missing or invalid inputs stop the build.

## Local build prerequisites

Build on Linux or WSL2, with the checkout and build directory on a Linux
filesystem. Install Git LFS, stable Rust with the native musl target, and the
image tools. On amd64:

```bash
sudo apt install git git-lfs build-essential musl-tools debootstrap qemu-utils \
  parted dosfstools mtools rsync util-linux e2fsprogs grub-efi-amd64-bin \
  grub-pc-bin dpkg-dev curl gnupg ca-certificates python3 bubblewrap
arch=amd64
rust_target=x86_64-unknown-linux-musl
rustup target add --toolchain stable "$rust_target"
```

On arm64, use `arch=arm64`, `rust_target=aarch64-unknown-linux-musl`, and
`grub-efi-arm64-bin` instead of the amd64 GRUB packages.

From the OS checkout, record the exact App commit before starting the build:

```bash
app_url=https://github.com/xiaoyu-work/yuos-app.git
app_revision="$(git ls-remote --exit-code "$app_url" refs/heads/main | cut -f1)"
```

In a disposable **Debian 13** builder of the same architecture, with no signing
keys or persisted Git credentials, check out that saved commit of `yuos-app`.
With `git`, `ca-certificates` and `python3` installed, run from that checkout:

```bash
test "$(git rev-parse HEAD)" = "$app_revision"
GITHUB_ACTIONS=true python3 tools/release_ci.py build --desktop-shell --architecture "$arch"
python3 tools/build_desktop_shell.py --architecture "$arch"
```

The CI provisioning command changes the builder's packages; do not run it
on the signing host. Transfer only `build/desktop-shell/unsigned-$arch.tar.gz`.
On the trusted OS checkout, seal that archive with the approved App Ed25519
key JSON (`APP_PUBLISHER_KEY_FILE` points to an authorized local key file).
Keep the archive and the pinned App revision from the preceding steps.

```bash
cargo +stable build --release --locked -p yu --bin yu --target "$rust_target"
install -D -m 0755 "target/$rust_target/release/yu" \
  "build/app-packages/yu-provenance-linux-$arch"
YUOS_APPS_PROVENANCE_SIGNING_PRIVATE_KEY="$(cat "$APP_PUBLISHER_KEY_FILE")" \
  python3 scripts/desktop_shell.py seal --architecture "$arch" \
    --source-revision "$app_revision" --unsigned-archive "/path/to/unsigned-$arch.tar.gz"
python3 scripts/app_packages.py prepare --variant agent --architecture "$arch"
python3 scripts/app_packages.py prepare --variant desktop --architecture "$arch"
```

Build with `sudo ./presets/desktop.sh` after configuring the
[OS package signer](../packaging/README.md#update-downgrade-protection)
and [archive keyring](../packaging/README.md#apt-repo-url).
The App signature check uses Bubblewrap and only the packaged publisher; it
does not read or modify the build host's trust store. The default output is
`build/yuos-vm-amd64.vmdk`; import it into VMware.
See [the Azure build](building-azure.md) for a generalized Gen2 VHD.

## Installer

The release workflow publishes updates to APT, not installer downloads.
For local Calamares testing, use the same signed App and Desktop Shell
inputs above and follow [Build an Installable ISO](../CONTRIBUTING.md#build-an-installable-iso).

## Login

The local desktop VM opens the graphical setup wizard without asking you to
sign in. Create your account, optionally choose an avatar, and select
**Finish** to enter the desktop.
After logout or reboot, sign in with that account and password. Sign-in and
lock show the Yu OS wallpaper full-screen, with the time above and the
password field at the bottom. First-run setup keeps its frosted-glass panels.

Setup, sign-in, lock, and the signed Desktop Shell share the default wallpaper at
`/usr/share/backgrounds/yuos/yuos-default.jpg`: the centered Yu OS mark on a
light-gray background. A wallpaper chosen by the user takes precedence on
the desktop.

The signed system shell starts with its built-in desktop permissions.
Other App permission requests still appear in the graphical OS review
window. The headless VM still sets up its `yu` account on the serial console.

On an installed machine, the greeter and desktop use the connected display's
preferred mode as reported by its graphics driver. The setup wizard fits that
screen and uses a compact layout on narrow displays. The default wallpaper
scales proportionally to fill the display, cropping the edges when the aspect
ratio differs. A VM uses the mode its virtual display advertises; resizing
the viewer window is hypervisor-specific. No per-resolution wallpaper copies
are needed.

With the matching signed Shell, selecting an installed App in the desktop
menu submits its launch without a separate review. Yu OS Apps keep their
signed identity and separate permission checks; ordinary Linux desktop apps
open as the logged-in user and receive no Yu OS App authority. Shutdown and
restart from the system menu go directly to the OS;
other Apps and the Agent receive no power permission from a menu click.

The session menu's Lock button uses the OS locker and the login account's
PAM authentication. The Shell can request a lock but neither sees input on
the lock screen nor offers an unlock action.
The session menu's Sleep button also requires the OS to confirm that native
lock before it sends sleep to logind. Return to the graphical session after
resume so it can present
the lock screen.

Use the [coordinated logout/update procedure](updating.md#root-managed-desktop-session-lifetime)
when replacing the compositor and display authority on an existing system.
