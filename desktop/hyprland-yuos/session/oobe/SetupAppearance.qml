pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts

RowLayout {
    id: appearance
    required property bool light
    required property string currentTime
    signal choose(bool light)
    spacing: 24

    Repeater {
        model: [true, false]
        Controls.AbstractButton {
            id: preview
            required property bool modelData
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            implicitHeight: artwork.implicitHeight
            hoverEnabled: true
            text: modelData ? qsTr("Light") : qsTr("Dark")
            Accessible.name: text
            Accessible.role: Accessible.RadioButton
            checked: appearance.light === modelData
            Accessible.checked: checked
            onClicked: appearance.choose(modelData)

            contentItem: Column {
                id: artwork
                spacing: 12
                Rectangle {
                    width: parent.width
                    height: (width - 8) * 10 / 16 + 8
                    radius: 12
                    color: "white"
                    border.width: preview.checked || preview.visualFocus ? 2 : 1
                    border.color: preview.checked || preview.visualFocus ? "#005cfe" : preview.hovered ? "#b8c1cc" : "#dce1e7"
                    Rectangle {
                        id: mini
                        anchors.fill: parent
                        anchors.margins: 4
                        radius: 8
                        color: preview.modelData ? "#f3f5f7" : "#15181c"
                        // The preview shows the default desktop, without launching an App.
                        SetupMark {
                            anchors.centerIn: parent
                            width: mini.width * 0.117
                            height: width * 0.93
                            ink: preview.modelData ? "#1e1e1e" : "#eef2f7"
                        }
                        Rectangle {
                            id: bar
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.leftMargin: 2
                            anchors.rightMargin: 2
                            anchors.topMargin: 2
                            height: Math.max(9, mini.width * 44 / 1280)
                            radius: 3
                            color: preview.modelData ? "#fbfcfd" : "#0f1216"
                            readonly property color ink: preview.modelData ? "#19202b" : "#eef2f7"
                            Row {
                                anchors.left: parent.left
                                anchors.leftMargin: 4
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 3
                                SetupMark { width: bar.height * 0.64; height: width * 0.93; ink: bar.ink }
                                Rectangle { width: 1; height: bar.height / 2; color: bar.ink; opacity: 0.18 }
                                Image { width: bar.height * 0.73; height: width; source: "files.svg" }
                                Image { width: bar.height * 0.73; height: width; source: "launcher.svg" }
                            }
                            Row {
                                anchors.right: parent.right
                                anchors.rightMargin: 4
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 4
                                Text {
                                    text: appearance.currentTime
                                    color: bar.ink
                                    font.pixelSize: Math.max(5, bar.height * 0.53)
                                }
                                SetupIcon { width: bar.height * 0.62; height: width; name: "wifi"; ink: bar.ink }
                                SetupIcon { width: bar.height * 0.62; height: width; name: "power"; ink: bar.ink }
                            }
                        }
                    }
                }
                Controls.Label {
                    text: preview.text
                    font.weight: preview.checked ? Font.DemiBold : Font.Medium
                    color: "#0f1520"
                }
            }
        }
    }
}
