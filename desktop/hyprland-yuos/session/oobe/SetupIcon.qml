import QtQuick

Canvas {
    id: icon
    property string name: ""
    property color ink: "#4b5566"
    property int strength: 100
    implicitWidth: 18
    implicitHeight: 18
    onNameChanged: requestPaint()
    onInkChanged: requestPaint()
    onStrengthChanged: requestPaint()

    onPaint: {
        const c = getContext("2d");
        c.reset();
        c.scale(width / 24, height / 24);
        c.lineWidth = 1.8;
        c.lineCap = "round";
        c.lineJoin = "round";
        c.fillStyle = c.strokeStyle = ink;
        function line(points) {
            c.beginPath();
            c.moveTo(points[0], points[1]);
            for (let i = 2; i < points.length; i += 2)
                c.lineTo(points[i], points[i + 1]);
            c.stroke();
        }
        function circle(x, y, r, solid) {
            c.beginPath();
            c.arc(x, y, r, 0, 2 * Math.PI);
            if (solid)
                c.fill();
            else
                c.stroke();
        }
        if (name === "search") {
            circle(10, 10, 5.5, false);
            line([14, 14, 20, 20]);
        } else if (name === "check") {
            line([5, 12, 10, 17, 20, 7]);
        } else if (name === "down" || name === "up") {
            line(name === "down" ? [6, 9, 12, 15, 18, 9] : [6, 15, 12, 9, 18, 15]);
        } else if (name === "left") {
            line([15, 5, 8, 12, 15, 19]);
        } else if (name === "eye" || name === "eye-off") {
            c.beginPath();
            c.moveTo(2, 12);
            c.bezierCurveTo(8, 3, 16, 3, 22, 12);
            c.bezierCurveTo(16, 21, 8, 21, 2, 12);
            c.stroke();
            circle(12, 12, 2.5, false);
            if (name === "eye-off")
                line([3, 3, 21, 21]);
        } else if (name === "wifi") {
            for (let i = 3; i > 0; --i) {
                c.globalAlpha = strength >= (i - 1) * 30 ? 1 : 0.25;
                c.beginPath();
                c.arc(12, 19, 4.5 * i, 1.25 * Math.PI, 1.75 * Math.PI);
                c.stroke();
            }
            c.globalAlpha = 1;
            circle(12, 19, 1.5, true);
        } else if (name === "lock") {
            c.strokeRect(6, 11, 12, 10);
            c.beginPath();
            c.moveTo(8, 11);
            c.lineTo(8, 8);
            c.arc(12, 8, 4, Math.PI, 0);
            c.lineTo(16, 11);
            c.stroke();
        } else if (name === "power") {
            c.beginPath();
            c.arc(12, 12, 7.5, -Math.PI / 4, 5 * Math.PI / 4);
            c.stroke();
            line([12, 3, 12, 12]);
        } else if (name === "external") {
            line([12, 5, 6, 5, 6, 19, 20, 19, 20, 13]);
            line([14, 5, 20, 5, 20, 11]);
            line([12, 13, 20, 5]);
        }
    }
}
