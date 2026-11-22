pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: welcome
    required property real headerX
    required property real headerY
    required property bool active
    required property bool acknowledged
    required property bool light
    required property bool reduceMotion
    required property string greeting
    required property string clock
    property real travel: 0
    property real captionProgress: 0
    property real stripProgress: 0
    readonly property real wallpaperUnit: 0.72 * Math.max(width / 3840, height / 2160)
    readonly property var curve: [0.2, 0.8, 0.2, 1, 1, 1]
    signal finished()

    function maybeGreet() {
        if (active && acknowledged && travel === 1 && !greet.running)
            greet.start();
    }

    onActiveChanged: {
        move.stop();
        greet.stop();
        captionProgress = 0;
        stripProgress = 0;
        if (reduceMotion) {
            travel = active ? 1 : 0;
            maybeGreet();
        } else {
            move.start();
        }
    }
    onAcknowledgedChanged: maybeGreet()

    SequentialAnimation {
        id: move
        PauseAnimation { duration: welcome.active ? 100 : 0 }
        NumberAnimation {
            target: welcome
            property: "travel"
            to: welcome.active ? 1 : 0
            duration: 640
            easing.type: Easing.BezierSpline
            easing.bezierCurve: welcome.curve
        }
        onFinished: welcome.maybeGreet()
    }
    SequentialAnimation {
        id: greet
        PauseAnimation { duration: welcome.reduceMotion ? 0 : 240 }
        NumberAnimation {
            target: welcome
            property: "captionProgress"
            to: 1
            duration: welcome.reduceMotion ? 200 : 520
            easing.type: Easing.BezierSpline
            easing.bezierCurve: welcome.curve
        }
        PauseAnimation { duration: 1300 }
        NumberAnimation { target: welcome; property: "captionProgress"; to: 0; duration: 300 }
        NumberAnimation {
            target: welcome
            property: "stripProgress"
            to: 1
            duration: welcome.reduceMotion ? 200 : 320
            easing.type: Easing.BezierSpline
            easing.bezierCurve: welcome.curve
        }
        onFinished: welcome.finished()
    }

    SetupMark {
        id: mark
        readonly property real endWidth: 563 * welcome.wallpaperUnit
        width: 26 + (endWidth - 26) * welcome.travel
        height: width * 524 / 563
        x: welcome.headerX + ((welcome.width - endWidth) / 2 - welcome.headerX) * welcome.travel
        y: welcome.headerY + ((welcome.height - endWidth * 524 / 563) / 2 - welcome.headerY) * welcome.travel
        ink: welcome.active && !welcome.light ? "#e9edf1" : "#1e1e1e"
        Behavior on ink { ColorAnimation { duration: welcome.reduceMotion ? 0 : 640 } }
        working: welcome.active && welcome.travel === 1 && !welcome.acknowledged && !welcome.reduceMotion
    }
    Text {
        objectName: "welcomeName"
        x: 24
        y: mark.y + mark.height + Math.min(40, welcome.height * 0.05)
            + (welcome.reduceMotion ? 0 : 10 * (1 - welcome.captionProgress))
        width: welcome.width - 48
        height: 48
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        color: welcome.light ? "#0f1520" : "#eef2f7"
        font.family: "Inter Display"
        font.weight: Font.DemiBold
        font.pixelSize: welcome.width < 800 ? 30 : 40
        font.letterSpacing: welcome.width < 800 ? -0.72 : -0.96
        font.hintingPreference: Font.PreferNoHinting
        textFormat: Text.PlainText
        text: welcome.greeting
        elide: Text.ElideRight
        opacity: welcome.captionProgress
        visible: welcome.acknowledged && opacity > 0
    }

    // Decorative handoff only. The real Shell starts after greetd retires this surface.
    Rectangle {
        id: strip
        y: welcome.reduceMotion ? 0 : (welcome.stripProgress - 1) * height
        width: parent.width
        height: 44
        visible: welcome.acknowledged && welcome.stripProgress > 0
        opacity: welcome.reduceMotion ? welcome.stripProgress : 1
        color: welcome.light ? "#ebfbfcfd" : "#eb0f1216"
        readonly property color ink: welcome.light ? "#19202b" : "#eef2f7"

        Rectangle {
            y: parent.height
            width: parent.width
            height: 1
            color: welcome.light ? "#dde2e8" : "#12ffffff"
        }
        Row {
            x: 10
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6
            Item {
                width: 36
                height: 36
                SetupMark {
                    anchors.centerIn: parent
                    width: 22
                    height: 20
                    ink: welcome.light ? "#1e1e1e" : "#eef2f7"
                }
            }
            Item {
                width: 9
                height: 36
                Rectangle { anchors.centerIn: parent; width: 1; height: 22; color: strip.ink; opacity: 0.16 }
            }
            Repeater {
                model: ["files.svg", "launcher.svg"]
                Item {
                    required property string modelData
                    width: 36
                    height: 36
                    Image { anchors.centerIn: parent; width: 25; height: 25; source: parent.modelData }
                }
            }
        }
        Row {
            anchors.right: parent.right
            anchors.rightMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6
            Text {
                height: 36
                leftPadding: 8
                rightPadding: 8
                verticalAlignment: Text.AlignVCenter
                font.family: "Inter"
                font.pixelSize: 14
                font.weight: Font.DemiBold
                color: strip.ink
                text: welcome.clock
            }
            Repeater {
                model: ["wifi", "power"]
                Item {
                    required property string modelData
                    width: 36
                    height: 36
                    SetupIcon {
                        anchors.centerIn: parent
                        width: 17
                        height: 17
                        name: parent.modelData === "wifi" ? "wifi-status" : parent.modelData
                        ink: strip.ink
                    }
                }
            }
        }
    }
}
