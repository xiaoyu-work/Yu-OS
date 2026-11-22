pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls

SetupGroup {
    id: choices
    property var items: []
    property string selected: ""
    property int maxHeight: 452
    property alias list: list
    signal choose(string value)
    implicitHeight: Math.min(maxHeight, Math.max(1, items.length) * 52 + 2)

    ListView {
        id: list
        anchors.fill: parent
        anchors.margins: 1
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: choices.items
        // Resetting the search list must never activate the first result.
        currentIndex: -1
        keyNavigationEnabled: true
        delegate: SetupRow {
            required property int index
            required property var modelData
            width: ListView.view.width
            text: modelData.title
            detail: modelData.detail ?? ""
            trailing: modelData.meta ?? ""
            divided: index > 0
            selected: modelData.id === choices.selected
            highlighted: ListView.isCurrentItem
            onClicked: choices.choose(modelData.id)
        }
        Controls.ScrollBar.vertical: Controls.ScrollBar {}
    }
    Controls.Label {
        visible: choices.items.length === 0
        text: qsTr("No results")
        x: 16
        anchors.verticalCenter: parent.verticalCenter
        color: "#667085"
    }
}
