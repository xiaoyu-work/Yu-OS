import QtQuick

Item {
    Rectangle {
        anchors.fill: parent
        anchors.margins: -1
        color: "white"
        radius: 12
    }
    Rectangle {
        anchors.fill: parent
        anchors.margins: -1
        z: 1
        color: "transparent"
        radius: 12
        border.color: "#e0e5eb"
    }
}
