pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls

SetupGroup {
    id: hardware
    required property string status
    required property var packages
    required property bool reduceMotion
    implicitHeight: rows.implicitHeight + 2

    Column {
        id: rows
        x: 1
        y: 1
        width: parent.width - 2

        SetupRow {
            width: parent.width
            objectName: "driverStatus"
            interactive: false
            strong: true
            text: ({
                pending: qsTr("Waiting for internet"),
                installing: qsTr("Installing drivers…"),
                ready: qsTr("Drivers installed"),
                failed: qsTr("Couldn't install drivers")
            })[hardware.status] ?? qsTr("Checking hardware…")
            detail: hardware.status === "failed" ? qsTr("Reconnect to the internet to retry, or finish setup now.") : ""
            bad: hardware.status === "failed"
            arrow: hardware.status === "ready" ? "check" : ""
            Controls.BusyIndicator {
                width: 24
                height: 24
                anchors.right: parent.right
                anchors.rightMargin: 16
                anchors.verticalCenter: parent.verticalCenter
                running: hardware.status === "installing" && !hardware.reduceMotion
            }
        }
        Repeater {
            model: hardware.packages
            SetupRow {
                required property string modelData
                width: rows.width
                interactive: false
                divided: true
                text: modelData
            }
        }
    }
}
