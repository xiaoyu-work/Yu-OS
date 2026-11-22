pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts

SetupGroup {
    id: wifi
    required property var snapshot
    property string network: ""
    property string password: ""
    readonly property bool connecting: snapshot.wifiState === "connecting"
    signal connectTo(string name, string password)
    implicitHeight: content.implicitHeight + 2

    ListModel { id: visibleNetworks }

    function syncNetworks() {
        const rows = snapshot.networks;
        if (rows.some(item => item.name === network && item.connected))
            password = "";
        for (let i = 0; i < rows.length; i++) {
            const row = rows[i];
            let j = i;
            while (j < visibleNetworks.count && visibleNetworks.get(j).name !== row.name)
                j++;
            if (j === visibleNetworks.count) {
                visibleNetworks.insert(i, row);
            } else {
                if (j !== i)
                    visibleNetworks.move(j, i, 1);
                visibleNetworks.set(i, row);
            }
        }
        if (visibleNetworks.count > rows.length)
            visibleNetworks.remove(rows.length, visibleNetworks.count - rows.length);
    }
    onSnapshotChanged: if (visibleNetworks) syncNetworks()
    Component.onCompleted: syncNetworks()

    Column {
        id: content
        x: 1
        y: 1
        width: parent.width - 2

        SetupRow {
            width: parent.width
            visible: wifi.snapshot.networkState === "wired" || wifi.snapshot.networkState === "portal"
                || (wifi.snapshot.networkState === "online" && !wifi.snapshot.networks.some(item => item.connected))
            interactive: false
            strong: true
            text: wifi.snapshot.networkState === "portal" ? qsTr("Network sign-in required")
                : wifi.snapshot.networkState === "wired" ? qsTr("Ethernet connected") : qsTr("Connected")
        }
        SetupRow {
            width: parent.width
            visible: wifi.snapshot.networks.length === 0 && !["online", "wired", "portal"].includes(wifi.snapshot.networkState)
            interactive: false
            text: wifi.snapshot.networkState === "checking" ? qsTr("Checking connection…")
                : wifi.snapshot.hasWifi ? qsTr("Looking for Wi-Fi networks…") : qsTr("No Wi-Fi networks available")
            detail: wifi.snapshot.hasWifi ? "" : qsTr("Connect Ethernet or continue offline.")
        }

        Repeater {
            model: visibleNetworks
            Column {
                id: entry
                required property int index
                required property string name
                required property int strength
                required property bool secure
                required property bool connected
                width: content.width
                readonly property bool selected: wifi.network === name
                readonly property string problem: {
                    if (!selected || connected || wifi.snapshot.wifiName !== name)
                        return "";
                    switch (wifi.snapshot.wifiState) {
                    case "unsupported": return qsTr("This network’s security is not supported in setup.");
                    case "failed": return secure ? qsTr("Couldn't connect. Check the password and try again.")
                        : qsTr("Couldn't connect. Try again.");
                    default: return "";
                    }
                }

                SetupRow {
                    width: parent.width
                    text: entry.name
                    symbol: "wifi"
                    strength: entry.strength
                    detail: entry.connected ? qsTr("Connected")
                        : entry.selected && wifi.connecting && !entry.secure ? qsTr("Connecting…") : ""
                    good: entry.connected
                    strong: entry.selected || entry.connected
                    arrow: entry.secure && !entry.connected ? "lock" : ""
                    selected: entry.connected
                    divided: entry.index > 0
                    inset: 48
                    interactive: !wifi.connecting && !entry.connected
                    onClicked: {
                        if (!entry.selected) {
                            wifi.network = entry.name;
                            wifi.password = "";
                        }
                        if (!entry.secure)
                            wifi.connectTo(entry.name, "");
                        else
                            passwordBox.focusInput();
                    }
                }
                RowLayout {
                    x: 48
                    width: parent.width - 64
                    spacing: 8
                    visible: entry.selected && entry.secure && !entry.connected
                    SetupField {
                        id: passwordBox
                        Layout.fillWidth: true
                        secret: true
                        placeholder: qsTr("Password")
                        name: qsTr("Password for %1").arg(entry.name)
                        maxLength: 128
                        enabled: !wifi.connecting
                        text: visible ? wifi.password : ""
                        invalid: entry.problem !== ""
                        onEdited: wifi.password = text
                        onAccepted: if (connectButton.enabled) wifi.connectTo(entry.name, wifi.password)
                    }
                    SetupButton {
                        id: connectButton
                        Layout.preferredWidth: Math.min(160, entry.width * 0.38)
                        kind: "primary"
                        text: wifi.connecting ? qsTr("Connecting…") : qsTr("Connect")
                        enabled: !wifi.connecting && wifi.password !== ""
                        onClicked: wifi.connectTo(entry.name, wifi.password)
                    }
                }
                Controls.Label {
                    visible: entry.problem !== ""
                    x: 48
                    width: parent.width - 64
                    topPadding: 8
                    text: entry.problem
                    color: "#c0321f"
                    font.pixelSize: 13
                    wrapMode: Text.Wrap
                }
                Item {
                    visible: (entry.selected && entry.secure && !entry.connected) || entry.problem !== ""
                    height: 16
                    width: 1
                }
            }
        }
    }
}
