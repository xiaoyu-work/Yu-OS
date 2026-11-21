# Yu OS desktop

Yu OS uses [Hyprland](hyprland-yuos/MODULE.md) with the signed
[Desktop Shell App](https://github.com/xiaoyu-work/yuos-app/tree/main/products/desktop-shell),
which owns Quickshell, Caelestia, and their private runtime. The OS release
builds it from the locked `yuos-app/main` commit and embeds the signed payload
in `yuos-desktop`; there is no separate Shell release or APT package.

The OS owns greetd/PAM login, display and App authority, system services,
and the native Agent interface. The Agent's terminal and Web use do not
depend on the desktop. Existing Apps keep their identities and shared
toolkit without requiring the previous shell.

Build the native Media Player in the `yuos-app` checkout with
`python3 tools/native_build.py media-player build`.

- [Build a VM image](../docs/building-desktop.md)
- [Build an Azure image](../docs/building-azure.md)
- [Source origins and licenses](PROVENANCE.md)
- [Installed login and updates](../core/src/display_session/MODULE.md)
- [Agent visual window control](../docs/desktop-vision.md)
