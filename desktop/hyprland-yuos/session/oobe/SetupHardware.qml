import QtQuick

SetupGroup {
    id: hardware
    required property string status
    required property bool reduceMotion
    implicitHeight: statusRow.implicitHeight

    SetupRow {
        id: statusRow
        width: parent.width
        objectName: "driverStatus"
        interactive: false
        strong: true
        minHeight: 68
        inset: 48
        text: ({
            pending: qsTr("Waiting for internet"),
            installing: qsTr("Installing drivers…"),
            ready: qsTr("Drivers installed"),
            failed: qsTr("Couldn't install drivers")
        })[hardware.status] ?? qsTr("Checking hardware…")
        detail: hardware.status === "failed" ? qsTr("Yu OS will retry when you reconnect.") : ""
        symbol: ({pending: "wifi-status", installing: "spinner", ready: "check", failed: "alert"})[hardware.status] ?? ""
        symbolInk: ({installing: "#005cfe", ready: "#0d7a53", failed: "#c0321f"})[hardware.status] ?? "#4b5566"
        spinning: hardware.status === "installing" && !hardware.reduceMotion
    }
}
