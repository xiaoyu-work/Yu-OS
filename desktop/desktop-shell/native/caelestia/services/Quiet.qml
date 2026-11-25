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

    readonly property list<string> pins: {
        const valid = preferences.pins.filter(id => /^[A-Za-z0-9][A-Za-z0-9._-]{0,254}$/.test(id));
        const ordered = [...new Set([...valid, DesktopMenu.launcher])];
        return ordered.length <= 24 ? ordered : [DesktopMenu.launcher, ...ordered.filter(id => id !== DesktopMenu.launcher).slice(0, 23)];
    }
    readonly property var byDesktopId: new Map(DesktopEntries.applications.values
        .map(entry => [entry.id, entry]))
    readonly property var byStartupClass: new Map(DesktopEntries.applications.values
        .filter(entry => entry.startupClass)
        .map(entry => [entry.startupClass, entry]))

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
        const running = new Set(Hypr.desktop.runningApps);
        const rows = new Map();
        for (const id of new Set([...pins, ...running, ...dockWindows.keys()])) {
            const window = dockWindows.get(id);
            if (window || id === DesktopMenu.files || id === DesktopMenu.launcher || desktopEntry(id)) {
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
        if (!app || app === DesktopMenu.agent)
            return "";
        return desktopEntry(app)?.id ?? byStartupClass.get(app)?.id ?? (pins.includes(app) ? app : "");
    }

    function desktopEntry(desktopId: string): DesktopEntry {
        // The index notifies when the initial scan or a later App install adds an entry.
        return byDesktopId.get(desktopId) ?? DesktopEntries.byId(desktopId);
    }

    function title(desktopId: string): string {
        const entry = desktopEntry(desktopId);
        if (entry)
            return entry.name;
        if (desktopId === DesktopMenu.launcher)
            return Tr.tr("Yu Launcher");
        if (desktopId === DesktopMenu.files)
            return Tr.tr("Files");
        return desktopId;
    }

    function icon(desktopId: string): url {
        const entry = desktopEntry(desktopId);
        if (entry?.icon)
            return Quickshell.iconPath(entry.icon, "application-x-executable");
        if (desktopId === DesktopMenu.files)
            return Quickshell.shellPath("assets/dock/files.svg");
        if (desktopId === DesktopMenu.launcher)
            return Quickshell.shellPath("assets/dock/launcher.svg");
        return Quickshell.iconPath(desktopId, "application-x-executable");
    }

    function activate(desktopId: string): void {
        // Resolve the current target even when the visible roles have not changed.
        const window = dockWindows.get(desktopId);
        if (window)
            Hypr.act("focus", window.identifier, "");
        else if (desktopId !== DesktopMenu.launcher || !Hypr.desktop.runningApps.includes(desktopId))
            DesktopMenu.activate(desktopId);
    }

    function togglePin(id: string): void {
        if (id === DesktopMenu.launcher)
            return;
        if (pins.includes(id))
            preferences.pins = pins.filter(pin => pin !== id);
        else if (pins.length < 24 && (desktopEntry(id) || dockRows.has(id)))
            preferences.pins = [...pins, id];
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

            property list<string> pins: [DesktopMenu.files, DesktopMenu.launcher]
            property string appearance: ""
        }
    }
}
