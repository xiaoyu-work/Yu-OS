# Building the Yu OS Desktop Image

The desktop profile builds Hyprland and the graphical Agent against Debian 13.
`yuos-desktop` builds and embeds the OS-owned Desktop Shell (Quickshell,
Caelestia and private Qt). Its complete tree inventory is bound to the signed
OS release. There is no separate Shell release, version to enter, or package
to install. Every ordinary Yu OS App still comes from its
own signed APT package. Missing or invalid inputs stop the build.

## Local build prerequisites

Build on Linux or WSL2, with the checkout and build directory on a Linux
filesystem. Install Git LFS, stable Rust with the native musl target, and the
image tools. On amd64:

```bash
sudo apt install git git-lfs build-essential musl-tools debootstrap qemu-utils \
  parted dosfstools mtools rsync util-linux e2fsprogs grub-efi-amd64-bin \
  grub-pc-bin dpkg-dev curl gnupg ca-certificates python3
arch=amd64
rust_target=x86_64-unknown-linux-musl
rustup target add --toolchain stable "$rust_target"
```

On arm64, use `arch=arm64`, `rust_target=aarch64-unknown-linux-musl`, and
`grub-efi-arm64-bin` instead of the amd64 GRUB packages.

From the OS checkout, prepare the signed independent App inputs:

```bash
python3 scripts/app_packages.py prepare --variant agent --architecture "$arch"
python3 scripts/app_packages.py prepare --variant desktop --architecture "$arch"
```

Build with `sudo ./presets/desktop.sh` after configuring the
[OS package signer](../packaging/README.md#update-downgrade-protection)
and [archive keyring](../packaging/README.md#apt-repo-url).
The Shell is built inside the target Debian 13 staging environment and signed
with the OS release, not an App publisher key. App package preparation verifies
the APT index with the pinned App archive key and checks each downloaded package
against that index. The default output is
`build/yuos-vm-amd64.vmdk`; import it into VMware.
See [the Azure build](building-azure.md) for a generalized Gen2 VHD.

## Installer

The release workflow publishes updates to APT, not installer downloads.
For local Calamares testing, prepare the same signed App inputs above and follow
[Build an Installable ISO](../CONTRIBUTING.md#build-an-installable-iso).

## Login

The local desktop VM opens the graphical setup wizard without asking you to
sign in. Choose a language and a keyboard layout, connect if needed, and
select your models or choose **Set up later**. Create your account and pick
your time zone and appearance. If matching driver packages are still pending,
**Hardware** shows their status without requiring you to wait. Select
**Finish setup** to sign in and see the welcome animation before the desktop
opens. There is no separate review step.

After logout or reboot, sign in with that account and password. Sign-in,
lock, and the Desktop Shell share the default wallpaper at
`/usr/share/backgrounds/yuos/yuos-default.jpg`: the centered Yu OS mark on
a light-gray background. Sign-in and lock place the time above and the
password field at the bottom. A wallpaper chosen by the user takes
precedence on the desktop.

The release-verified system shell starts with its built-in desktop permissions.
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
