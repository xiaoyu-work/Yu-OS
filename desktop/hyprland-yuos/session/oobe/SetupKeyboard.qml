pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls

Column {
    id: keyboard
    required property var layouts
    required property string selected
    required property string preferred
    required property bool reduceMotion
    property int sweepTime: -1
    readonly property var layout: layouts.find(row => row.id === selected)
    readonly property var keys: {
        if (!layout)
            return [];
        const alpha = layout.keys.map((row, i) => row.map(letter => [
            i && letter.toUpperCase().length === 1 ? letter.toUpperCase() : letter, 1
        ]));
        // XKB reports the optional ISO key for pc105 even when a US ANSI diagram is used.
        if (["us", "pinyin", "mozc", "hangul"].includes(layout.id) && alpha[3].length === 11)
            alpha[3].shift();
        if (alpha[1].length === 13)
            alpha[1][12][1] = 1.5;
        return [
            [...alpha[0], ["", 2]],
            [["", 1.5], ...alpha[1]],
            [["", 1.75], ...alpha[2], ["", 2.25]],
            [["", alpha[3].length > 10 ? 1.25 : 2.25], ...alpha[3], ["", 2.75]],
            [["", 1.25], ["", 1.25], ["", 1.25], ["", 6.25], ["", 1.25], ["", 1.25], ["", 1.25], ["", 1.25]]
        ];
    }
    readonly property real keyUnits: Math.max(15, ...keys.map(row => row.reduce((sum, key) => sum + key[1], 0)))
    signal choose(string value)
    spacing: 12
    Accessible.description: qsTr("Applies to your desktop. Setup and sign-in use the current keyboard.")

    function sweepKeys() {
        if (!reduceMotion) {
            sweepTime = -1;
            sweep.restart();
        }
    }
    Component.onCompleted: sweepKeys()
    onSelectedChanged: sweepKeys()

    SequentialAnimation {
        id: sweep
        NumberAnimation { target: keyboard; property: "sweepTime"; from: 0; to: 650; duration: 650 }
        PropertyAction { target: keyboard; property: "sweepTime"; value: -1 }
    }
    SetupPicker {
        id: picker
        width: parent.width
        search: true
        name: qsTr("Search keyboard layouts")
        placeholder: qsTr("Search")
        choices: keyboard.layouts
        selected: keyboard.selected
        preferred: keyboard.preferred
        onChosen: value => keyboard.choose(value)
    }
    SetupGroup {
        width: parent.width
        height: content.implicitHeight
        Column {
            id: content
            width: parent.width
            Column {
                id: caps
                x: 16
                topPadding: 16
                bottomPadding: 16
                width: parent.width - 32
                spacing: 4
                opacity: picker.popupOpen ? 0.4 : 1
                Repeater {
                    model: keyboard.keys
                    Row {
                        id: keyRow
                        required property int index
                        required property var modelData
                        readonly property real unit: Math.min(40.8, (caps.width + spacing) / keyboard.keyUnits)
                        spacing: 4
                        Repeater {
                            model: keyRow.modelData
                            Rectangle {
                                required property int index
                                required property var modelData
                                width: keyRow.unit * modelData[1] - keyRow.spacing
                                height: Math.min(36, keyRow.unit * 0.9)
                                radius: 6
                                readonly property int lead: 200 + (index - 1) * 60
                                readonly property bool lit: keyRow.index === 1 && index >= 1 && index <= 6
                                    && keyboard.sweepTime >= lead && keyboard.sweepTime < lead + 150
                                border.color: lit ? "#005cfe" : "#dde2e8"
                                color: lit ? "#005cfe" : "#f6f8fa"
                                Rectangle {
                                    visible: !parent.lit
                                    anchors.bottom: parent.bottom
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.margins: 3
                                    anchors.bottomMargin: 0
                                    height: 1
                                    color: "#c3cbd5"
                                }
                                Controls.Label {
                                    anchors.centerIn: parent
                                    text: parent.modelData[0]
                                    textFormat: Text.PlainText
                                    color: parent.lit ? "white" : "#222a36"
                                    font.pixelSize: 13
                                    font.weight: Font.Medium
                                }
                            }
                        }
                    }
                }
            }
            SetupRow {
                width: parent.width
                minHeight: 64
                divided: true
                strong: true
                interactive: false
                text: keyboard.layout?.title ?? ""
                trailing: keyboard.layout?.meta ?? ""
            }
        }
    }
}
