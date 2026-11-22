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
    property int minHeight: 52
    property bool reserveCheck: false
    property bool spinning: false
    property color symbolInk: "#4b5566"
    property color trailingInk: "#667085"
    property bool selected: false
    property bool strong: selected
    property bool good: false
    property bool divided: false
    property bool interactive: true
    property bool roundTop: false
    property bool roundBottom: false
    property bool suppressDivider: false
    hoverEnabled: interactive
    focusPolicy: Qt.StrongFocus
    enabled: interactive
    leftPadding: symbol !== "" ? Math.max(0, inset - 32) : 16
    rightPadding: 16
    topPadding: 9
    bottomPadding: 9
    implicitHeight: Math.max(minHeight, implicitContentHeight + topPadding + bottomPadding)
    Accessible.name: [text, detail, trailing].filter(value => value).join(", ")
    Accessible.selected: selected
    contentItem: RowLayout {
        spacing: 12
        SetupIcon {
            visible: row.symbol !== ""
            Layout.preferredWidth: 20
            Layout.preferredHeight: 20
            name: row.symbol
            strength: row.strength
            ink: row.symbolInk
            spinning: row.spinning
        }
        Column {
            Layout.fillWidth: true
            Text {
                width: parent.width
                height: 20
                verticalAlignment: Text.AlignVCenter
                textFormat: Text.PlainText
                text: row.text
                font.family: row.font.family
                font.pixelSize: row.font.pixelSize
                font.weight: row.strong ? Font.DemiBold : Font.Normal
                font.letterSpacing: row.font.letterSpacing
                font.hintingPreference: row.font.hintingPreference
                elide: Text.ElideRight
                color: "#0f1520"
            }
            Text {
                visible: row.detail !== ""
                width: parent.width
                textFormat: Text.PlainText
                text: row.detail
                font.family: row.font.family
                font.pixelSize: 13
                font.letterSpacing: -0.078
                font.hintingPreference: row.font.hintingPreference
                lineHeight: 18
                lineHeightMode: Text.FixedHeight
                wrapMode: Text.Wrap
                color: row.good ? "#0d7a53" : "#667085"
            }
        }
        Text {
            Layout.maximumWidth: Math.min(300, row.width * 0.48)
            visible: text !== ""
            textFormat: Text.PlainText
            text: row.trailing
            elide: Text.ElideRight
            font.family: row.font.family
            font.pixelSize: 13
            font.letterSpacing: -0.078
            font.hintingPreference: row.font.hintingPreference
            color: row.trailingInk
        }
        SetupIcon {
            visible: row.reserveCheck || row.selected || row.arrow !== ""
            opacity: row.selected || row.arrow !== "" ? 1 : 0
            name: row.selected && row.arrow === "" ? "check" : row.arrow
            ink: row.arrow === "" ? "#005cfe" : "#8a93a2"
            Layout.preferredWidth: row.arrow === "" ? 18 : 16
            Layout.preferredHeight: Layout.preferredWidth
        }
    }
    background: Rectangle {
        topLeftRadius: row.visualFocus || row.roundTop ? 12 : 0
        topRightRadius: topLeftRadius
        bottomLeftRadius: row.visualFocus || row.roundBottom ? 12 : 0
        bottomRightRadius: bottomLeftRadius
        color: row.down && row.interactive ? "#eef1f4"
            : row.hovered || row.highlighted ? "#f6f8fa" : "transparent"
        border.width: row.visualFocus ? 2 : 0
        border.color: "#005cfe"
        Rectangle {
            visible: row.divided && !row.suppressDivider && !row.hovered && !row.highlighted
            x: row.inset
            width: parent.width - x
            height: 1
            color: "#ebeef2"
        }
    }
}
