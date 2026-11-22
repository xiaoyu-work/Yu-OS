# Yu OS desktop

Yu OS uses [Hyprland](hyprland-yuos/MODULE.md) with the OS-owned
[Desktop Shell source](desktop-shell/), including Quickshell, Caelestia and
their private runtime. `yuos-desktop` builds and carries the Shell; there is
no separate Shell release or App package.
The imported Shell documentation describes its former App build; the current
OS integration is documented in [Hyprland Integration](hyprland-yuos/MODULE.md).

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
