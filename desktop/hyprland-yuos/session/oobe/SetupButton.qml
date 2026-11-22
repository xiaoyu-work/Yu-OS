import QtQuick
import QtQuick.Controls.Basic as Controls

Controls.Button {
    id: button
    property string kind: "ghost"
    property string symbol: ""
    implicitHeight: 44
    implicitWidth: Math.max(kind === "primary" ? 120 : 0, implicitContentWidth + leftPadding + rightPadding)
    padding: 0
    leftPadding: kind === "back" ? 0 : 18
    rightPadding: 18
    Accessible.name: text
    hoverEnabled: true

    contentItem: Item {
        implicitWidth: group.implicitWidth
        implicitHeight: group.implicitHeight
        Row {
            id: group
            anchors.centerIn: parent
            spacing: 6
            SetupIcon {
                anchors.verticalCenter: parent.verticalCenter
                visible: button.symbol !== ""
                name: button.symbol
                ink: caption.color
            }
            Text {
                id: caption
                anchors.verticalCenter: parent.verticalCenter
                text: button.text
                color: button.kind === "primary" && button.enabled ? "white" : button.enabled ? "#0f1520" : "#8a93a2"
                font.family: button.font.family
                font.pixelSize: button.font.pixelSize
                font.weight: Font.DemiBold
                textFormat: Text.PlainText
                elide: Text.ElideRight
            }
        }
    }
    background: Rectangle {
        radius: 10
        color: {
            if (button.kind === "primary")
                return !button.enabled ? "#dce1e7" : button.down ? "#0047c2" : button.hovered ? "#0050dd" : "#005cfe";
            if (button.kind === "secondary")
                return button.down ? "#eef1f4" : "white";
            return button.down ? "#e5e9ef" : button.hovered ? "#eaedf1" : "transparent";
        }
        border.width: button.visualFocus ? 2 : button.kind === "secondary" ? 1 : 0
        border.color: button.visualFocus ? "#005cfe" : button.hovered ? "#b8c1cc" : "#d1d8e0"
    }
}
