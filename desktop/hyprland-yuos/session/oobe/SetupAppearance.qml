pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts

RowLayout {
    id: appearance
    required property bool light
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
                    border.color: preview.checked || preview.visualFocus ? "#005cfe" : preview.hovered ? "#b8c1cc" : "#e0e5eb"
                    Image {
                        anchors.fill: parent
                        anchors.margins: 4
                        source: preview.modelData ? "appearance-light.png" : "appearance-dark.png"
                        mipmap: true
                    }
                }
                Controls.Label {
                    text: preview.text
                    height: 20
                    verticalAlignment: Text.AlignVCenter
                    font.weight: preview.checked ? Font.DemiBold : Font.Medium
                    color: "#0f1520"
                }
            }
        }
    }
}
