# Desktop source provenance

[`hyprland/`](hyprland/) is the complete BSD-3-Clause
[`hyprwm/Hyprland`](https://github.com/hyprwm/Hyprland) source at
`f3e076844484255b77629acc5c75379817c405a8`, original tree
`85b7b316d0b4104f23b1a7e73b9def3564b9b6db`. The
[`source lock`](hyprland-source.lock.json) records the expanded Tracy and
udis86 sources and their exact original trees. The imported `.gitmodules`
is provenance, not a Yu OS submodule dependency.

The [GPL-3.0-only Root authority adapter](hyprland-yuos/MODULE.md) is linked
unconditionally. Native dependency revisions and archive hashes are recorded
in [`dependencies.lock.json`](hyprland-yuos/dependencies.lock.json).
[`toolchain.lock.json`](hyprland-yuos/toolchain.lock.json) pins the GCC
compiler layer of the Docker Official Image for each native architecture.
The installed private C++ runtime carries the original GCC license and
runtime exception; its compiler is not included.
No Omarchy source is used. The Shell source snapshot in
[`desktop-shell/`](desktop-shell/) was imported from
`yuos-app` commit `35262bce654ad3c96ac0cdb602161b72131a621d`, tree
`f4667b3fc1fec0703fd86db549761722bb091f06`. Its original notices and
documentation are retained as imported; OS build and login integration is
described in [`hyprland-yuos/MODULE.md`](hyprland-yuos/MODULE.md).
The [Media Player App](https://github.com/xiaoyu-work/yuos-app/tree/main/products/media-player)
records the provenance of its independently packaged native player.

The independent native locker builds the MIT-licensed Swaylock 1.8.6
source pinned in [`build-locker.sh`](hyprland-yuos/build-locker.sh). Its
original notice ships with the binary. The Yu OS native password form replaces
the upstream indicator. The PAM patch requires the dedicated Yu OS service
for unlock and removes non-PAM recovery paths.

The [default wallpaper](../assets/brand/wallpaper/yuos-default.svg) centers
the Yu OS mark on a light-gray background. The image and its softened variant
are original project artwork under [Apache-2.0](../LICENSE); see the
[artwork notice](../assets/brand/wallpaper/NOTICE) for the previous mountain
image's attribution. No COSMIC greeter or compositor code is used for these
pages.

The [bundled account avatars](../assets/brand/avatars/) are original generated
illustrations for Yu OS. They use no third-party characters or images.
The first-run time zone map uses the approved setup design's
[public-domain Natural Earth dot grid](hyprland-yuos/session/oobe/WorldDots.NOTICE).
The [light](hyprland-yuos/session/oobe/appearance-light.png) and
[dark](hyprland-yuos/session/oobe/appearance-dark.png) appearance thumbnails
are the original setup mock's decorative artwork. They show the theme on an
illustrated Files window without launching Files.

## Shared App framework

These are retained to support existing standalone products and the Agent UI.
They do not provide a compositor, login, or desktop session.

| Directory | Upstream | Revision | License |
|---|---|---|---|
| `toolkit/` | pop-os/libcosmic | `4fab6c777dbd` | MPL-2.0 |
| `toolkit/iced/` | pop-os/iced | `347d91f7ead2` | MIT |
| `icons/`, `toolkit/cosmic-icons/` | pop-os/cosmic-icons | `2c697e8e97cf` | CC-BY-SA-4.0 |
| `text/` | pop-os/cosmic-text | `c24886c2471e` | Apache-2.0 |
| `theme/` | pop-os/cosmic-theme | `ce3a63a10638` | MPL-2.0 |
| `time/` | pop-os/cosmic-time | `257aecae2aa4` | MIT |
| `launcher-backend/` | pop-os/launcher | `5b8685107166` | MPL-2.0; binary terms in its manifest |

The forked [Tela icons](icons-tela/README.md) keep their original license.
System76's trademark notice remains in [TRADEMARK.md](TRADEMARK.md).
The [shared Applet services](services/yuos-applet-services/MODULE.md) were
extracted from the former applet host under their original GPL terms.
The installed Settings/AccountsService policy from the `703a934b096b`
Settings fork retains its GPL-3.0-only license in the Desktop package.
The relocated [Smithay protocol fixture](../crates/yuos-display-login/test/vendor/README.md)
retains its own upstream pin and MIT notice; no Desktop package builds it.
