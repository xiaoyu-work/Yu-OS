pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls

Column {
    id: keyboard
    required property var layouts
    required property string selected
    readonly property var layout: layouts.find(row => row.id === selected)
    readonly property var keys: {
        if (!layout)
            return [];
        const alpha = layout.keys.map((row, i) => row.map(letter => [i ? letter.toUpperCase() : letter, 1]));
        return [
            [...alpha[0], ["", 2]],
            [["", 1.5], ...alpha[1]],
            [["", 1.75], ...alpha[2], ["", 2.25]],
            [["", 1.5], ...alpha[3], ["", 2]],
            [["", 1.25], ["", 1.25], ["", 1.25], ["", 6.25], ["", 1.25], ["", 1.25], ["", 1.25], ["", 1.25]]
        ];
    }
    signal choose(string value)
    spacing: 12

    SetupPicker {
        width: parent.width
        search: true
        name: qsTr("Search keyboard layouts")
        placeholder: qsTr("Search")
        choices: keyboard.layouts
        selected: keyboard.selected
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
                width: parent.width
                padding: 16
                spacing: 4
                Repeater {
                    model: keyboard.keys
                    Row {
                        id: keyRow
                        required property var modelData
                        readonly property real unit: Math.max(16, (caps.width - 32 + spacing) / 15.5)
                        spacing: 4
                        Repeater {
                            model: keyRow.modelData
                            Rectangle {
                                required property var modelData
                                width: keyRow.unit * modelData[1] - keyRow.spacing
                                height: Math.min(36, keyRow.unit * 0.9)
                                radius: 6
                                border.color: "#dde2e8"
                                color: "#f6f8fa"
                                Controls.Label {
                                    anchors.centerIn: parent
                                    text: parent.modelData[0]
                                    textFormat: Text.PlainText
                                    color: "#222a36"
                                    font.pixelSize: width > 28 ? 10 : 13
                                }
                            }
                        }
                    }
                }
            }
            SetupRow {
                width: parent.width - 2
                x: 1
                implicitHeight: 64
                divided: true
                strong: true
                interactive: false
                text: keyboard.layout?.title ?? ""
                trailing: keyboard.layout?.meta ?? ""
            }
        }
    }
    Controls.Label {
        width: parent.width
        text: qsTr("Applies to your desktop. Setup and sign-in use the current keyboard.")
        color: "#667085"
        font.pixelSize: 13
        wrapMode: Text.Wrap
    }
}
