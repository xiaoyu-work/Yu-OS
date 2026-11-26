pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import QtQuick.Controls.Basic
import QtQuick.Shapes
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import Quickshell.Bluetooth
import Caelestia
import Caelestia.I18n
import Caelestia.State
import qs.components
import qs.components.containers
import qs.services

StyledWindow {
    id: root

    required property StyledWindow bar
    property string page
    property string pinId
    property real pinX
    readonly property bool opened: page !== ""
    property string displayedPage
    property real reveal: opened ? 1 : 0
    readonly property bool pinMenu: displayedPage === "pin"
    readonly property int slideDistance: pinMenu ? 0 : 4
    readonly property var backlight: screen ? Brightness.getMonitorForScreen(screen) : null
    property real virtualBrightness: 0.85

    // Keep the last page and its anchor for the exit, but release input immediately.
    onPageChanged: {
        if (page !== "")
            displayedPage = page;
    }
    onOpenedChanged: {
        if (opened)
            scroll.forceActiveFocus();
    }

    Behavior on reveal {
        Anim { type: Anim.FastEffects }
    }

    name: "yuos-top-menu"
    visible: opened || reveal > 0
    anchors {
        top: true
        left: root.pinMenu
        right: !root.pinMenu
    }
    margins {
        top: root.pinMenu ? 0 : 6 - root.slideDistance
        left: root.pinMenu ? Math.max(8, Math.min(root.pinX, root.screen.width - root.implicitWidth - 8)) : 0
        right: root.pinMenu ? 0 : 8
    }
    exclusiveZone: 0
    implicitWidth: Math.min(pinMenu ? 168 : 336, screen.width - 16)
    implicitHeight: slideDistance
        + Math.max(1, Math.min(body.implicitHeight + (pinMenu ? 8 : 28), screen.height - bar.implicitHeight - 16))
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: opened ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None

    mask: Region {
        // Fixed bounds avoid updating the compositor's input region on every animation frame.
        y: root.slideDistance
        width: root.opened ? root.width : 0
        height: root.opened ? Math.max(0, root.height - y) : 0
        radius: surface.radius
    }

    function toggle(name: string): void {
        if (page === name) {
            page = "";
        } else {
            displayedPage = name;
            page = name;
        }
    }

    function openPin(id: string, x: real): void {
        displayedPage = "pin";
        pinId = id;
        pinX = x;
        page = "pin";
    }

    function settings(): void {
        page = "";
        DesktopMenu.launch("com.yuos.Settings");
    }

    HyprlandFocusGrab {
        active: root.opened
        windows: [root, root.bar]
        onCleared: root.page = ""
    }

    QuietSurface {
        id: surface

        width: parent.width
        height: Math.max(0, parent.height - root.slideDistance)
        y: root.slideDistance * root.reveal
        opacity: root.reveal
        radius: root.pinMenu ? 8 : 12
        border.color: Quiet.floatingLine

        ScrollView {
            id: scroll

            anchors.fill: parent
            anchors.margins: root.pinMenu ? 4 : 14
            contentWidth: availableWidth
            clip: true
            enabled: root.opened
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            focus: true
            Keys.priority: Keys.BeforeItem
            Keys.onEscapePressed: root.page = ""

            ColumnLayout {
                id: body

                width: scroll.availableWidth
                spacing: root.pinMenu ? 2 : 8

                RowLayout {
                    visible: root.displayedPage === "wifi" || root.displayedPage === "bluetooth" || root.displayedPage === "notifications"
                    Layout.fillWidth: true

                    QuietButton {
                        text: "\u2039"
                        padding: 0
                        font.pixelSize: 20
                        Layout.preferredWidth: 28
                        Accessible.name: Tr.tr("Back")
                        onClicked: root.page = "control"
                    }

                    Text {
                        Layout.fillWidth: true
                        color: Quiet.ink
                        font.pixelSize: 14
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                        text: ({
                            wifi: Tr.tr("Wi-Fi"),
                            bluetooth: Tr.tr("Bluetooth"),
                            notifications: Tr.tr("Notifications")
                        })[root.displayedPage] ?? ""
                    }

                    QuietButton {
                        visible: root.displayedPage === "wifi"
                        text: Nmcli.wifiEnabled ? Tr.tr("On") : Tr.tr("Off")
                        highlighted: Nmcli.wifiEnabled
                        onClicked: Nmcli.toggleWifi(null)
                    }

                    QuietButton {
                        visible: root.displayedPage === "bluetooth"
                        text: Bluetooth.defaultAdapter?.enabled ? Tr.tr("On") : Tr.tr("Off")
                        highlighted: Bluetooth.defaultAdapter?.enabled ?? false
                        enabled: Bluetooth.defaultAdapter !== null
                        onClicked: {
                            if (Bluetooth.defaultAdapter)
                                Bluetooth.defaultAdapter.enabled = !Bluetooth.defaultAdapter.enabled;
                        }
                    }
                }

                Separator { visible: root.displayedPage === "wifi" || root.displayedPage === "bluetooth" || root.displayedPage === "notifications" }

                ColumnLayout {
                    visible: root.displayedPage === "control"
                    Layout.fillWidth: true
                    spacing: 10

                    GridLayout {
                        Layout.fillWidth: true
                        columns: 2
                        rowSpacing: 8
                        columnSpacing: 8

                        ToggleTile {
                            Layout.fillWidth: true
                            title: Nmcli.active?.ssid || Tr.tr("Wi-Fi")
                            toggled: Nmcli.wifiEnabled
                            hasMore: true
                            iconPath: "M1.5 5 Q8 -0.5 14.5 5 M4 8 Q8 4.5 12 8 M6.5 11 Q8 9.5 9.5 11 M8 13.5 H8.1"
                            onClicked: Nmcli.toggleWifi(null)
                            onMoreClicked: root.page = "wifi"
                        }
                        ToggleTile {
                            Layout.fillWidth: true
                            title: Tr.tr("Bluetooth")
                            toggled: Bluetooth.defaultAdapter?.enabled ?? false
                            hasMore: true
                            iconPath: "M8 1.5 L12 5.2 4 11.6 M8 14.5 L12 10.8 4 4.4 M8 1.5 V14.5"
                            onClicked: {
                                if (Bluetooth.defaultAdapter)
                                    Bluetooth.defaultAdapter.enabled = !Bluetooth.defaultAdapter.enabled;
                                else
                                    root.page = "bluetooth";
                            }
                            onMoreClicked: root.page = "bluetooth"
                        }
                        ToggleTile {
                            Layout.fillWidth: true
                            title: Tr.tr("Do not disturb")
                            toggled: PersistentState.doNotDisturb
                            iconPath: "M11.5 2.5 A5.5 5.5 0 1 0 13.5 11.5 A4.5 4.5 0 0 1 11.5 2.5 Z"
                            onClicked: PersistentState.doNotDisturb = !PersistentState.doNotDisturb
                        }
                        ToggleTile {
                            Layout.fillWidth: true
                            title: Tr.tr("Dark mode")
                            toggled: !Quiet.light
                            iconPath: "M8 2 A6 6 0 1 0 8 14 A6 6 0 1 0 8 2 Z M8 2.2 V13.8 M9.6 3.2 V12.8 M11.2 4.4 V11.6 M12.6 6 V10"
                            onClicked: Quiet.toggleAppearance()
                        }
                    }

                    LevelRow {
                        title: Tr.tr("Display")
                        iconPath: "M8 4.5 A3.5 3.5 0 1 0 8 11.5 A3.5 3.5 0 1 0 8 4.5 Z M8 1.2 V2.6 M8 13.4 V14.8 M1.2 8 H2.6 M13.4 8 H14.8 M3.2 3.2 L4.2 4.2 M11.8 11.8 L12.8 12.8 M12.8 3.2 L11.8 4.2 M4.2 11.8 L3.2 12.8"
                        available: true
                        adjustable: true
                        reading: (root.backlight?.available ?? false) ? root.backlight.brightness : root.virtualBrightness
                        onChangedByUser: value => {
                            if (root.backlight?.adjustable ?? false)
                                root.backlight.setBrightness(value);
                            else
                                root.virtualBrightness = value;
                        }
                    }
                    LevelRow {
                        title: Tr.tr("Sound")
                        iconPath: Audio.muted
                            ? "M2.5 6.2 H5.2 L8.8 3.2 V12.8 L5.2 9.8 H2.5 Z M11.2 5.8 L14.4 10.2 M14.4 5.8 L11.2 10.2"
                            : "M2.5 6.2 H5.2 L8.8 3.2 V12.8 L5.2 9.8 H2.5 Z M11.2 5.4 Q13 8 11.2 10.6 M12.8 3.6 Q15.5 8 12.8 12.4"
                        available: true
                        adjustable: true
                        reading: Audio.muted ? 0 : (Audio.sink !== null ? Audio.volume : 0.75)
                        onIconClicked: Audio.setMuted(false, !Audio.muted)
                        onChangedByUser: value => Audio.setVolume(value)
                    }

                    Separator {}

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        QuietButton {
                            text: Notifs.notClosed.length > 0
                                ? `${Tr.tr("Notifications")} (${Notifs.notClosed.length})`
                                : Tr.tr("Notifications")
                            onClicked: root.page = "notifications"
                        }
                        Item { Layout.fillWidth: true }
                        QuietButton {
                            width: 34
                            height: 34
                            Accessible.name: Tr.tr("Settings")
                            onClicked: root.settings()
                            contentItem: Item {
                                ControlIcon {
                                    anchors.centerIn: parent
                                    color: Quiet.ink
                                    drawing: "M2.5 4.5 H13.5 M2.5 8 H13.5 M2.5 11.5 H13.5 M5.5 3 V6 M10.5 6.5 V9.5 M6.5 10 V13"
                                }
                            }
                        }
                        QuietButton {
                            width: 34
                            height: 34
                            Accessible.name: Tr.tr("Power")
                            onClicked: root.page = "power"
                            contentItem: Item {
                                ControlIcon {
                                    anchors.centerIn: parent
                                    color: Quiet.ink
                                    drawing: "M8 1.8 V7.2 M4.3 4.2 A5.4 5.4 0 1 0 11.7 4.2"
                                }
                            }
                        }
                    }
                }

                ColumnLayout {
                    visible: root.displayedPage === "wifi"
                    Layout.fillWidth: true
                    spacing: 4

                    Repeater {
                        model: root.visible && root.displayedPage === "wifi" ? Nmcli.networks : []
                        delegate: QuietButton {
                            required property var modelData
                            Layout.fillWidth: true
                            highlighted: modelData.active
                            text: modelData.ssid || Tr.tr("Hidden network")
                            onClicked: root.settings()
                        }
                    }
                    QuietButton {
                        Layout.fillWidth: true
                        text: Tr.tr("Network Settings…")
                        onClicked: root.settings()
                    }
                }

                ColumnLayout {
                    visible: root.displayedPage === "bluetooth"
                    Layout.fillWidth: true
                    spacing: 4

                    QuietButton {
                        Layout.fillWidth: true
                        text: Tr.tr("Bluetooth Settings…")
                        onClicked: root.settings()
                    }
                }

                ColumnLayout {
                    visible: root.displayedPage === "overview"
                    Layout.fillWidth: true

                    Text {
                        color: Quiet.ink
                        font.pixelSize: 25
                        text: Time.format("hh:mm")
                    }
                    Text {
                        color: Quiet.dim
                        text: Time.format("dddd, MMMM d")
                    }
                    Separator {}
                    QuietButton {
                        text: Tr.tr("Settings")
                        onClicked: root.settings()
                    }
                }

                ColumnLayout {
                    visible: root.displayedPage === "notifications"
                    Layout.fillWidth: true

                    Text {
                        visible: Notifs.notClosed.length === 0 || Notifs.unavailable
                        text: Notifs.unavailable ? Tr.tr("Notifications unavailable") : Tr.tr("No notifications")
                        color: Quiet.dim
                    }

                    Repeater {
                        model: root.visible && root.displayedPage === "notifications" ? Notifs.notClosed : []

                        delegate: ColumnLayout {
                            id: notification

                            required property var modelData
                            Layout.fillWidth: true

                            SettingRow {
                                title: notification.modelData.summary || notification.modelData.appName
                                description: notification.modelData.body
                                action: Tr.tr("Dismiss")
                                actionEnabled: notification.modelData.confirmed && !notification.modelData.pending
                                onClicked: notification.modelData.close()
                            }
                            Separator {}
                        }
                    }
                }

                ColumnLayout {
                    visible: root.displayedPage === "power"
                    Layout.fillWidth: true

                    QuietButton {
                        text: Tr.tr("Lock")
                        onClicked: {
                            root.page = "";
                            ScreenLock.request();
                        }
                    }
                    Repeater {
                        model: [
                            {label: Tr.tr("Sleep"), action: "suspend"},
                            {label: Tr.tr("Restart"), action: "reboot"},
                            {label: Tr.tr("Shut down"), action: "poweroff"}
                        ]
                        QuietButton {
                            required property var modelData

                            text: modelData.label
                            enabled: !Power.busy
                            onClicked: {
                                root.page = "";
                                Power.request(modelData.action);
                            }
                        }
                    }
                    QuietButton {
                        text: Tr.tr("Log out / switch user")
                        onClicked: root.page = "logout"
                    }
                }

                ColumnLayout {
                    visible: root.displayedPage === "logout"
                    Layout.fillWidth: true

                    Text {
                        Layout.fillWidth: true
                        color: Quiet.dim
                        wrapMode: Text.WordWrap
                        text: Tr.tr("Save your work first. Logging out closes your apps and returns to the account picker.")
                    }

                    RowLayout {
                        QuietButton {
                            text: Tr.tr("Cancel")
                            onClicked: root.page = "power"
                        }
                        QuietButton {
                            text: Tr.tr("Log out")
                            onClicked: Qt.quit()
                        }
                    }
                }

                ColumnLayout {
                    visible: root.pinMenu
                    Layout.fillWidth: true
                    spacing: 1

                    PinMenuItem {
                        label: Quiet.title(root.pinId)
                        iconSource: root.pinId ? Quiet.icon(root.pinId).toString() : ""
                        onClicked: {
                            const id = root.pinId;
                            root.page = "";
                            if (id)
                                Quiet.activate(id);
                        }
                    }

                    PinMenuItem {
                        label: Quiet.pins.includes(root.pinId)
                            ? Tr.tr("Unpin from top bar")
                            : Tr.tr("Pin to top bar")
                        drawing: Quiet.pins.includes(root.pinId)
                            ? "M2 2 L14 14 M9.5 2.5 L13.5 6.5 L12 8 L10.2 6.8 L7.5 9.5 M6.5 8.5 L4.5 7.5 L3.3 8.7 L7.3 12.7 L8.5 11.5 M5.3 10.7 L2.5 13.5"
                            : "M9.5 2.5 L13.5 6.5 L12 8 L10.2 6.8 L7.5 9.5 L8.5 11.5 L7.3 12.7 L5.2 10.6 L2.5 13.3 M5.2 10.6 L3.3 8.7 L4.5 7.5 L6.5 8.5 L9.2 5.8 L8 4 Z"
                        onClicked: {
                            Quiet.togglePin(root.pinId);
                            root.page = "";
                        }
                    }

                    PinMenuItem {
                        visible: Quiet.dockWindows.has(root.pinId)
                        label: Tr.tr("Close window")
                        drawing: "M4 4 L12 12 M12 4 L4 12"
                        onClicked: {
                            const win = Quiet.dockWindows.get(root.pinId);
                            root.page = "";
                            if (win)
                                Hypr.act("close", win.identifier, "");
                        }
                    }
                }
            }
        }
    }

    component Separator: Rectangle {
        Layout.fillWidth: true
        implicitHeight: 1
        color: Quiet.line
    }

    component SettingRow: RowLayout {
        id: setting

        required property string title
        property string description
        required property string action
        property bool actionEnabled: true
        signal clicked()

        Layout.fillWidth: true

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Text {
                Layout.fillWidth: true
                text: setting.title
                textFormat: Text.PlainText
                color: Quiet.ink
                elide: Text.ElideRight
            }
            Text {
                Layout.fillWidth: true
                text: setting.description
                textFormat: Text.PlainText
                color: Quiet.dim
                font.pixelSize: 11
                elide: Text.ElideRight
                visible: text.length > 0
            }
        }

        QuietButton {
            text: setting.action
            enabled: setting.actionEnabled
            onClicked: setting.clicked()
        }
    }

    component ControlIcon: Shape {
        id: cicon

        required property string drawing
        property color color: Quiet.ink

        width: 16
        height: 16
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: cicon.color
            strokeWidth: 1.5
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            fillColor: "transparent"
            PathSvg { path: cicon.drawing }
        }
    }

    component PinMenuItem: Rectangle {
        id: item

        required property string label
        property string iconSource: ""
        property string drawing: ""
        signal clicked()

        Layout.fillWidth: true
        implicitHeight: 28
        radius: 5
        color: itemMouse.pressed ? Quiet.pressed : (itemMouse.containsMouse ? Quiet.field : "transparent")

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 10
            spacing: 9

            Item {
                Layout.preferredWidth: 16
                Layout.preferredHeight: 16

                Image {
                    anchors.fill: parent
                    visible: item.iconSource !== ""
                    source: item.iconSource
                    sourceSize: Qt.size(16, 16)
                    fillMode: Image.PreserveAspectFit
                    smooth: true
                }

                ControlIcon {
                    anchors.centerIn: parent
                    visible: item.drawing !== ""
                    drawing: item.drawing !== "" ? item.drawing : "M0 0"
                    color: Quiet.ink
                }
            }

            Text {
                Layout.fillWidth: true
                text: item.label
                textFormat: Text.PlainText
                color: Quiet.ink
                font.pixelSize: 12
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
            }
        }

        MouseArea {
            id: itemMouse
            anchors.fill: parent
            hoverEnabled: true
            onClicked: item.clicked()
        }
    }

    component ToggleTile: Rectangle {
        id: tile

        required property string title
        required property string iconPath
        property bool toggled: false
        property bool hasMore: false
        signal clicked()
        signal moreClicked()

        implicitHeight: 46
        radius: 10
        color: tile.toggled ? Quiet.accent : (mainMouse.containsMouse ? Quiet.selected : Quiet.field)
        border.width: tile.toggled ? 0 : 1
        border.color: Quiet.line

        readonly property color fg: tile.toggled ? "#ffffff" : Quiet.ink

        RowLayout {
            anchors.fill: parent
            spacing: 0

            MouseArea {
                id: mainMouse
                Layout.fillWidth: true
                Layout.fillHeight: true
                hoverEnabled: true
                onClicked: tile.clicked()

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: tile.hasMore ? 6 : 12
                    spacing: 9

                    ControlIcon {
                        drawing: tile.iconPath
                        color: tile.fg
                    }
                    Text {
                        Layout.fillWidth: true
                        text: tile.title
                        textFormat: Text.PlainText
                        color: tile.fg
                        font.pixelSize: 13
                        font.weight: Font.Medium
                        elide: Text.ElideRight
                    }
                }
            }

            Rectangle {
                visible: tile.hasMore
                width: 1
                Layout.fillHeight: true
                Layout.topMargin: 10
                Layout.bottomMargin: 10
                color: tile.toggled ? "#33ffffff" : Quiet.line
            }

            MouseArea {
                id: moreMouse
                visible: tile.hasMore
                Layout.preferredWidth: 28
                Layout.fillHeight: true
                hoverEnabled: true
                onClicked: tile.moreClicked()

                Text {
                    anchors.centerIn: parent
                    text: "\u203a"
                    color: tile.fg
                    font.pixelSize: 17
                    opacity: moreMouse.containsMouse ? 1 : 0.78
                }
            }
        }
    }

    component LevelRow: Rectangle {
        id: level

        required property string title
        required property string iconPath
        required property bool available
        required property bool adjustable
        required property real reading
        signal changedByUser(real value)
        signal iconClicked()

        Layout.fillWidth: true
        implicitHeight: 42
        radius: 10
        color: Quiet.field
        border.width: 1
        border.color: Quiet.line

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 10

            MouseArea {
                width: 20
                height: 20
                onClicked: level.iconClicked()
                ControlIcon {
                    anchors.centerIn: parent
                    drawing: level.iconPath
                    color: Quiet.ink
                }
            }

            Slider {
                id: slider

                Layout.fillWidth: true
                enabled: level.adjustable
                live: true
                Accessible.name: level.title
                onMoved: level.changedByUser(value)
                Binding {
                    target: slider
                    property: "value"
                    value: level.reading
                    when: !slider.pressed
                }

                background: Rectangle {
                    x: slider.leftPadding
                    y: slider.topPadding + slider.availableHeight / 2 - height / 2
                    implicitWidth: 160
                    implicitHeight: 6
                    width: slider.availableWidth
                    height: implicitHeight
                    radius: 3
                    color: Quiet.selected

                    Rectangle {
                        width: slider.visualPosition * parent.width
                        height: parent.height
                        radius: 3
                        color: Quiet.accent
                    }
                }

                handle: Rectangle {
                    x: slider.leftPadding + slider.visualPosition * (slider.availableWidth - width)
                    y: slider.topPadding + slider.availableHeight / 2 - height / 2
                    implicitWidth: 16
                    implicitHeight: 16
                    radius: 8
                    color: "#ffffff"
                    border.width: 1
                    border.color: Quiet.floatingLine
                }
            }
        }
    }
}
