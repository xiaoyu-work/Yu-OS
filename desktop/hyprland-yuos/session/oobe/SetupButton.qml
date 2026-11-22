import QtQuick
import QtQuick.Controls.Basic as Controls

Controls.Button {
    id: button
    property string kind: "ghost"
    property string symbol: ""
    property bool trailingSymbol: false
    property bool busy: false
    property bool reduceMotion: false
    property bool menuOpen: false
    implicitHeight: kind === "menu" ? 36 : 44
    implicitWidth: kind === "square" ? 44
        : Math.max(kind === "primary" ? 120 : 0, implicitContentWidth + leftPadding + rightPadding)
    padding: 0
    leftPadding: kind === "back" || kind === "square" ? 0 : kind === "menu" ? 10 : 18
    rightPadding: kind === "square" ? 0 : kind === "menu" ? 10 : 18
    hoverEnabled: true

    contentItem: Item {
        implicitWidth: group.implicitWidth
        implicitHeight: group.implicitHeight
        Row {
            id: group
            anchors.centerIn: parent
            spacing: button.kind === "back" ? 4 : button.kind === "menu" ? 6 : 8
            layoutDirection: button.trailingSymbol ? Qt.RightToLeft : Qt.LeftToRight
            SetupIcon {
                anchors.verticalCenter: parent.verticalCenter
                visible: button.busy || (button.symbol !== "" && button.kind !== "menu")
                width: button.busy ? 16 : 18
                height: width
                name: button.busy ? "spinner" : button.symbol
                spinning: button.busy && !button.reduceMotion
                ink: button.kind === "square" ? "#0f1520" : caption.color
            }
            Text {
                id: caption
                anchors.verticalCenter: parent.verticalCenter
                text: button.text
                visible: text !== ""
                color: button.kind === "primary" && (button.enabled || button.busy) ? "white"
                    : button.enabled ? "#0f1520" : "#8a93a2"
                font.family: button.font.family
                font.pixelSize: button.font.pixelSize
                font.weight: button.font.weight
                font.letterSpacing: button.font.letterSpacing
                font.hintingPreference: button.font.hintingPreference
                textFormat: Text.PlainText
                elide: Text.ElideRight
            }
            SetupIcon {
                anchors.verticalCenter: parent.verticalCenter
                visible: button.kind === "menu" && button.symbol !== ""
                width: 16
                height: 16
                name: button.symbol
                ink: "#667085"
            }
        }
    }
    background: Rectangle {
        radius: button.kind === "menu" ? 8 : 10
        color: {
            if (button.kind === "primary")
                return !button.enabled && !button.busy ? "#dce1e7" : button.down ? "#0047c2" : button.hovered ? "#0050dd" : "#005cfe";
            if (button.kind === "secondary" || button.kind === "square")
                return button.down ? "#eef1f4" : "white";
            return button.down ? "#e5e9ef" : button.hovered || button.menuOpen ? "#eaedf1" : "transparent";
        }
        border.width: button.visualFocus ? 2 : button.kind === "secondary" || button.kind === "square" ? 1 : 0
        border.color: button.visualFocus ? "#005cfe" : button.hovered ? "#b8c1cc" : "#d1d8e0"
    }
}
