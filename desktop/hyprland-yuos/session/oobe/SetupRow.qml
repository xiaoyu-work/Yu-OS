import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts

Controls.ItemDelegate {
    id: row
    property string detail: ""
    property string trailing: ""
    property string symbol: ""
    property string arrow: ""
    property int strength: 100
    property int inset: 16
    property bool selected: false
    property bool strong: selected
    property bool good: false
    property bool bad: false
    property bool divided: false
    property bool interactive: true
    hoverEnabled: interactive
    focusPolicy: interactive ? Qt.StrongFocus : Qt.NoFocus
    enabled: interactive
    leftPadding: 16
    rightPadding: 16
    topPadding: 8
    bottomPadding: 8
    implicitHeight: Math.max(52, implicitContentHeight + topPadding + bottomPadding)
    Accessible.name: [text, detail, trailing].filter(value => value).join(", ")
    Accessible.selected: selected
    contentItem: RowLayout {
        spacing: 12
        SetupIcon {
            visible: row.symbol !== ""
            name: row.symbol
            strength: row.strength
        }
        Column {
            Layout.fillWidth: true
            spacing: 2
            Text {
                width: parent.width
                textFormat: Text.PlainText
                text: row.text
                font.family: row.font.family
                font.pixelSize: row.font.pixelSize
                font.weight: row.strong ? Font.DemiBold : Font.Normal
                elide: Text.ElideRight
                color: row.bad ? "#c0321f" : "#0f1520"
            }
            Text {
                visible: row.detail !== ""
                width: parent.width
                textFormat: Text.PlainText
                text: row.detail
                font.family: row.font.family
                font.pixelSize: 13
                wrapMode: Text.Wrap
                color: row.good ? "#0d7a53" : "#667085"
            }
        }
        Text {
            Layout.maximumWidth: row.width * 0.45
            visible: text !== ""
            textFormat: Text.PlainText
            text: row.trailing
            elide: Text.ElideRight
            font.family: row.font.family
            font.pixelSize: 13
            color: "#667085"
        }
        SetupIcon {
            visible: row.selected || row.arrow !== ""
            name: row.selected && row.arrow === "" ? "check" : row.arrow
            ink: row.selected && row.arrow === "" ? "#005cfe" : "#667085"
            Layout.preferredWidth: 16
            Layout.preferredHeight: 16
        }
    }
    background: Rectangle {
        radius: 10
        color: row.down && row.interactive ? "#eef1f4"
            : row.hovered || row.selected || row.highlighted ? "#f6f8fa" : "transparent"
        border.width: row.visualFocus ? 2 : 0
        border.color: "#005cfe"
        Rectangle {
            visible: row.divided && !row.hovered && !row.selected
            x: row.inset
            width: parent.width - x
            height: 1
            color: "#ebeef2"
        }
    }
}
