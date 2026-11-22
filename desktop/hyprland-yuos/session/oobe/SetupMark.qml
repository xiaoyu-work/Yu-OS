import QtQuick

Canvas {
    id: mark
    property color ink: "#1e1e1e"
    property bool working: false
    property real activity: working ? 1 : 0
    property double epoch: 0
    readonly property var bars: [[64, 514, 298, 70, 900, 0],
        [230, 514, 381, 226, 700, 250], [388, 514, 483, 334, 1100, 500]]
    onInkChanged: requestPaint()
    onWorkingChanged: {
        if (working)
            epoch = Date.now();
        requestPaint();
    }
    onActivityChanged: requestPaint()
    Behavior on activity { NumberAnimation { duration: mark.working ? 0 : 240; easing.type: Easing.OutCubic } }
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    Timer {
        repeat: true
        interval: 33
        running: mark.working || mark.activity > 0
        onTriggered: mark.requestPaint()
    }
    onPaint: {
        const c = getContext("2d");
        c.reset();
        const unit = Math.min(width / 563, height / 524);
        c.translate((width - 563 * unit) / 2, (height - 524 * unit) / 2);
        c.scale(unit, unit);
        c.translate(-29, -35);
        c.lineWidth = 70;
        c.lineCap = "round";
        c.strokeStyle = ink;
        const elapsed = Date.now() - epoch;
        for (const [x1, y1, x2, y2, period, lead] of bars) {
            const phase = ((elapsed + lead) % period) / period;
            const length = 1 - activity * 0.23 * (1 - Math.cos(2 * Math.PI * phase));
            c.beginPath();
            c.moveTo(x1, y1);
            c.lineTo(x1 + (x2 - x1) * length, y1 + (y2 - y1) * length);
            c.stroke();
        }
        c.beginPath();
        c.arc(553, 520, 39, 0, 2 * Math.PI);
        c.fillStyle = "#005cfe";
        c.fill();
    }
}
