pragma Singleton
pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io
import Caelestia
import Caelestia.I18n

Singleton {
    id: root

    readonly property bool light: preferences.appearance !== "dark"
    readonly property color panel: light ? "#f8f7f8fa" : "#f814171c"
    readonly property color panelTop: light ? "#ebfbfcfd" : "#eb0f1216"
    readonly property color ink: light ? "#19202b" : "#eef2f7"
    readonly property color dim: light ? "#5a6270" : "#b8c0cc"
    readonly property color line: light ? "#dde2e8" : "#26ffffff"
    readonly property color floatingLine: light ? "#cfd6df" : "#38ffffff"
    readonly property color field: light ? "#eef1f5" : "#1affffff"
    readonly property color selected: light ? "#e2e7ee" : "#26ffffff"
    readonly property color pressed: light ? "#d5dce5" : "#38ffffff"
    readonly property color accent: "#005cfe"

    property bool reordering: false
    readonly property list<string> defaultPins: [
        DesktopMenu.files,
        DesktopMenu.browser,
        DesktopMenu.terminal,
        DesktopMenu.store,
        DesktopMenu.launcher
    ]
    readonly property list<string> pins: {
        const valid = preferences.pins.filter(id => /^[A-Za-z0-9][A-Za-z0-9._-]{0,254}$/.test(id));
        return [...new Set(valid)].slice(0, 24);
    }
    readonly property var byDesktopId: new Map(DesktopEntries.applications.values
        .map(entry => [entry.id, entry]))
    readonly property var byLowerId: new Map(DesktopEntries.applications.values
        .map(entry => [entry.id.toLowerCase(), entry]))
    readonly property var byStartupClass: new Map(DesktopEntries.applications.values
        .filter(entry => entry.startupClass)
        .map(entry => [entry.startupClass.toLowerCase(), entry]))

    readonly property ListModel dock: ListModel {}

    readonly property var dockWindows: {
        const windows = new Map();
        for (const window of Hypr.toplevels.values) {
            const id = dockId(window.lastIpcObject.class);
            if (id && (!windows.has(id) || window.active))
                windows.set(id, window);
        }
        return windows;
    }

    readonly property var dockRows: {
        const running = new Set(Hypr.desktop.runningApps
            .filter(id => !id.startsWith("com.yuos.Settings.")));
        const rows = new Map();
        for (const id of new Set([...pins, ...running, ...dockWindows.keys()])) {
            const window = dockWindows.get(id);
            if (pins.includes(id) || window || (running.has(id) && desktopEntry(id))) {
                rows.set(id, {
                    desktopId: id,
                    appRunning: running.has(id) || !!window,
                    focusedOutput: window?.active ? (window.monitor?.name ?? "") : ""
                });
            }
        }
        return rows;
    }

    readonly property string pendingDockId: {
        const identifier = Hypr.pendingFocusId;
        if (!identifier)
            return "";
        const window = Hypr.toplevels.values.find(window => window.identifier === identifier);
        return window ? dockId(window.lastIpcObject.class) : "";
    }

    onDockRowsChanged: Qt.callLater(syncDock)
    Component.onCompleted: Qt.callLater(syncDock)

    function syncDock(): void {
        if (reordering)
            return;
        for (let i = dock.count - 1; i >= 0; --i) {
            if (!dockRows.has(dock.get(i).desktopId))
                dock.remove(i);
        }

        let index = 0;
        for (const row of dockRows.values()) {
            let existing = index;
            while (existing < dock.count && dock.get(existing).desktopId !== row.desktopId)
                ++existing;
            if (existing === dock.count) {
                dock.insert(index, row);
            } else {
                if (existing !== index)
                    dock.move(existing, index, 1);
                const current = dock.get(index);
                if (current.appRunning !== row.appRunning || current.focusedOutput !== row.focusedOutput)
                    dock.set(index, { appRunning: row.appRunning, focusedOutput: row.focusedOutput });
            }
            ++index;
        }
    }

    function dockId(app: string): string {
        if (!app || app === DesktopMenu.agent || app === "YuOS-Agent" || app === "yuos-desktop-reviews")
            return "";
        const lower = app.toLowerCase();
        if (lower === "helium" || lower === "helium-browser" || lower === "chromium" || lower === "com.yuos.browser")
            return DesktopMenu.browser;
        return desktopEntry(app)?.id
            ?? byLowerId.get(lower)?.id
            ?? byStartupClass.get(lower)?.id
            ?? (/^[A-Za-z0-9][A-Za-z0-9._-]{0,254}$/.test(app) ? app : "");
    }

    function desktopEntry(desktopId: string): DesktopEntry {
        // The index notifies when the initial scan or a later App install adds an entry.
        return byDesktopId.get(desktopId) ?? DesktopEntries.byId(desktopId);
    }

    function title(desktopId: string): string {
        if (desktopId === DesktopMenu.launcher)
            return Tr.tr("Launcher");
        if (desktopId === DesktopMenu.store)
            return Tr.tr("App Store");
        if (desktopId === DesktopMenu.terminal)
            return Tr.tr("Terminal");
        if (desktopId === DesktopMenu.browser)
            return Tr.tr("Helium");
        const entry = desktopEntry(desktopId);
        if (entry)
            return entry.name;
        if (desktopId === DesktopMenu.files)
            return Tr.tr("Files");
        return desktopId;
    }

    function icon(desktopId: string): url {
        if (desktopId === DesktopMenu.files)
            return Quickshell.shellPath("assets/dock/files.svg");
        if (desktopId === DesktopMenu.browser)
            return Quickshell.shellPath("assets/dock/browser.svg");
        if (desktopId === DesktopMenu.terminal)
            return Quickshell.shellPath("assets/dock/terminal.svg");
        if (desktopId === DesktopMenu.store)
            return Quickshell.shellPath("assets/dock/store.svg");
        if (desktopId === DesktopMenu.launcher)
            return Quickshell.shellPath("assets/dock/launcher.svg");
        if (desktopId === "com.yuos.Settings")
            return Quickshell.shellPath("assets/dock/settings.svg");
        if (desktopId === "com.yuos.Edit")
            return Quickshell.shellPath("assets/dock/edit.svg");
        if (desktopId === "com.yuos.Screenshot")
            return Quickshell.shellPath("assets/dock/screenshot.svg");
        if (desktopId === "com.yuos.Player")
            return Quickshell.shellPath("assets/dock/player.svg");
        if (desktopId === "com.yuos.Office")
            return Quickshell.shellPath("assets/dock/office.svg");
        const entry = desktopEntry(desktopId);
        if (entry?.icon)
            return Quickshell.iconPath(entry.icon, "application-x-executable");
        return Quickshell.iconPath(desktopId, "application-x-executable");
    }

    function activate(desktopId: string): void {
        if (desktopId === DesktopMenu.agent) {
            for (const window of Hypr.toplevels.values) {
                const cls = window.lastIpcObject.class;
                if (cls === DesktopMenu.agent || cls === "YuOS-Agent") {
                    Hypr.act("focus", window.identifier, "");
                    return;
                }
            }
            DesktopMenu.launch(DesktopMenu.agent);
            return;
        }
        // Resolve the current target even when the visible roles have not changed.
        const window = dockWindows.get(desktopId);
        if (window)
            Hypr.act("focus", window.identifier, "");
        else if (desktopId !== DesktopMenu.launcher || !Hypr.desktop.runningApps.includes(desktopId))
            DesktopMenu.activate(desktopId);
    }

    function togglePin(id: string): void {
        if (pins.includes(id))
            preferences.pins = pins.filter(pin => pin !== id);
        else if (pins.length < 24 && (desktopEntry(id) || dockRows.has(id)))
            preferences.pins = [...pins, id];
    }

    function moveItem(fromIndex: int, toIndex: int): void {
        if (fromIndex === toIndex || fromIndex < 0 || toIndex < 0
            || fromIndex >= dock.count || toIndex >= dock.count)
            return;
        dock.move(fromIndex, toIndex, 1);
    }

    function commitOrder(movedId: string): void {
        const pinnedSet = new Set(pins);
        if (movedId && !pinnedSet.has(movedId) && pinnedSet.size < 24)
            pinnedSet.add(movedId);
        const ordered = [];
        for (let i = 0; i < dock.count; ++i) {
            const id = dock.get(i).desktopId;
            if (pinnedSet.has(id))
                ordered.push(id);
        }
        preferences.pins = ordered;
        Qt.callLater(syncDock);
    }

    function toggleAppearance(): void {
        setAppearance(!light);
    }

    function setAppearance(light: bool): void {
        preferences.appearance = light ? "light" : "dark";
    }

    FileView {
        id: file

        readonly property string directory: Quickshell.env("YUOS_DATA_DIR") || ""
        path: directory ? `${directory}/quiet-desktop.json` : ""
        onAdapterUpdated: {
            if (directory)
                writeAdapter();
        }
        onSaveFailed: Toaster.toast(Tr.tr("Desktop preferences not saved"),
            Tr.tr("Your changes apply for this session only."), "error")

        JsonAdapter {
            id: preferences

            property list<string> pins: [
                DesktopMenu.files,
                DesktopMenu.browser,
                DesktopMenu.terminal,
                DesktopMenu.store,
                DesktopMenu.launcher
            ]
            property string appearance: ""
        }
    }
}
