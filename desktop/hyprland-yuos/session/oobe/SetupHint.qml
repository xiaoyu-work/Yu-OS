import QtQuick
import QtQuick.Layouts

RowLayout {
    id: hint
    property alias text: caption.text
    property bool error: false
    property bool busy: false
    property bool reduceMotion: false
    spacing: 6
    visible: text !== ""

    SetupIcon {
        visible: hint.error || hint.busy
        Layout.preferredWidth: hint.error ? 16 : 12
        Layout.preferredHeight: Layout.preferredWidth
        name: hint.error ? "alert" : "spinner"
        spinning: hint.busy && !hint.reduceMotion
        ink: caption.color
    }
    Text {
        id: caption
        Layout.fillWidth: true
        font.family: "Inter"
        font.pixelSize: 13
        font.letterSpacing: -0.078
        font.hintingPreference: Font.PreferNoHinting
        lineHeight: 18
        lineHeightMode: Text.FixedHeight
        textFormat: Text.PlainText
        color: hint.error ? "#c0321f" : "#667085"
        wrapMode: Text.Wrap
    }
}
