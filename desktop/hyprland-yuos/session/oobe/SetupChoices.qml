pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls

SetupGroup {
    id: choices
    property var items: []
    property string selected: ""
    property int maxHeight: 452
    property bool showOverflowCue: false
    property bool floating: false
    property int activeIndex: -1
    signal choose(string value)
    implicitHeight: Math.min(maxHeight, items.length
        ? items.reduce((height, entry) => height + (entry.detail ? 56 : 52), 0) : 52)

    function focusRow(index) {
        if (index < 0 || index >= items.length) {
            activeIndex = -1;
            return;
        }
        activeIndex = index;
        list.positionViewAtIndex(index, ListView.Contain);
    }
    function moveFocus(direction) {
        if (items.length)
            focusRow((Math.max(0, activeIndex) + direction + items.length) % items.length);
    }

    Loader {
        active: choices.floating
        anchors.fill: parent
        anchors.margins: -44
        z: -1
        sourceComponent: Canvas {
            onWidthChanged: requestPaint()
            onHeightChanged: requestPaint()
            onPaint: {
                const c = getContext("2d");
                c.reset();
                c.shadowColor = "#47101828";
                c.shadowOffsetY = 18;
                c.shadowBlur = 28;
                c.fillStyle = "white";
                c.beginPath();
                c.roundedRect(54, 54, width - 108, height - 108, 12, 12);
                c.fill();
            }
        }
    }
    ListView {
        id: list
        anchors.fill: parent
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: choices.items
        currentIndex: -1
        keyNavigationEnabled: false
        delegate: SetupRow {
            required property int index
            required property var modelData
            width: ListView.view.width
            text: modelData.title
            detail: modelData.detail ?? ""
            trailing: modelData.meta ?? ""
            divided: index > 0
            roundTop: index === 0
            roundBottom: index + 1 === choices.items.length
            reserveCheck: true
            selected: modelData.id === choices.selected
            highlighted: index === choices.activeIndex
            suppressDivider: index > 0 && index - 1 === choices.activeIndex
            onClicked: choices.choose(modelData.id)
        }
    }
    Canvas {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 32
        visible: choices.showOverflowCue && !list.atYEnd
        onWidthChanged: requestPaint()
        onPaint: {
            const c = getContext("2d");
            const radius = 11;
            c.reset();
            const shade = c.createLinearGradient(0, 0, 0, height);
            shade.addColorStop(0, "#00ffffff");
            shade.addColorStop(0.9, "white");
            c.fillStyle = shade;
            c.beginPath();
            c.moveTo(0, 0);
            c.lineTo(width, 0);
            c.arcTo(width, height, width - radius, height, radius);
            c.arcTo(0, height, 0, height - radius, radius);
            c.closePath();
            c.fill();
        }
    }
    Controls.Label {
        visible: choices.items.length === 0
        text: qsTr("No results")
        x: 16
        anchors.verticalCenter: parent.verticalCenter
        color: "#667085"
    }
}
