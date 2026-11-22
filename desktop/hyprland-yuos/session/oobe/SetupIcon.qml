import QtQuick

Canvas {
    id: icon
    property string name: ""
    property color ink: "#4b5566"
    property int strength: 100
    property bool spinning: false
    property real angle: 0
    implicitWidth: 18
    implicitHeight: 18
    onNameChanged: requestPaint()
    onInkChanged: requestPaint()
    onStrengthChanged: requestPaint()
    rotation: spinning ? angle : 0
    NumberAnimation on angle {
        from: 0
        to: 360
        duration: 800
        loops: Animation.Infinite
        running: icon.spinning && icon.visible
    }

    onPaint: {
        const c = getContext("2d");
        c.reset();
        c.scale(width / 24, height / 24);
        c.lineWidth = 1.75;
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
            circle(10.8, 10.8, 6.3, false);
            line([15.6, 15.6, 20, 20]);
        } else if (name === "check") {
            c.lineWidth = 2.2;
            line([5, 12.6, 9.3, 16.9, 19, 7.2]);
        } else if (name === "down" || name === "up") {
            line(name === "down" ? [5.5, 9.5, 12, 16, 18.5, 9.5] : [5.5, 14.5, 12, 8, 18.5, 14.5]);
        } else if (name === "left") {
            line([14.5, 5.5, 8, 12, 14.5, 18.5]);
        } else if (name === "alert") {
            circle(12, 12, 8.5, false);
            line([12, 7.8, 12, 12.7]);
            circle(12, 16.1, 0.9, true);
        } else if (name === "spinner") {
            c.lineWidth = 2.4;
            c.beginPath();
            c.arc(12, 12, 8.6, Math.PI / 4, Math.PI * 1.75);
            c.stroke();
        } else if (name === "copy") {
            c.beginPath();
            c.roundedRect(8.5, 8.5, 11, 11, 2, 2);
            c.stroke();
            c.beginPath();
            c.moveTo(15.5, 8.5);
            c.lineTo(15.5, 6.5);
            c.quadraticCurveTo(15.5, 4.5, 13.5, 4.5);
            c.lineTo(6.5, 4.5);
            c.quadraticCurveTo(4.5, 4.5, 4.5, 6.5);
            c.lineTo(4.5, 13.5);
            c.quadraticCurveTo(4.5, 15.5, 6.5, 15.5);
            c.lineTo(8.5, 15.5);
            c.stroke();
        } else if (name === "eye" || name === "eye-off") {
            c.beginPath();
            if (name === "eye-off") {
                c.moveTo(9.9, 6.1);
                c.quadraticCurveTo(11, 5.8, 12, 5.8);
                c.bezierCurveTo(16.1, 5.8, 19.6, 8.3, 21.4, 12);
                c.quadraticCurveTo(20.3, 14, 18.8, 15.4);
                c.moveTo(6.3, 7.6);
                c.quadraticCurveTo(3.9, 9.3, 2.6, 12);
                c.bezierCurveTo(4.4, 15.7, 7.9, 18.2, 12, 18.2);
                c.quadraticCurveTo(14.4, 18.2, 16, 17.1);
                c.moveTo(9.9, 9.9);
                c.quadraticCurveTo(8, 14.9, 14.1, 14.1);
            } else {
                c.moveTo(2.6, 12);
                c.bezierCurveTo(4.4, 8.3, 7.9, 5.8, 12, 5.8);
                c.bezierCurveTo(16.1, 5.8, 19.8, 8.3, 21.4, 12);
                c.bezierCurveTo(19.6, 15.7, 16.1, 18.2, 12, 18.2);
                c.bezierCurveTo(7.9, 18.2, 4.4, 15.7, 2.6, 12);
            }
            c.stroke();
            if (name === "eye-off")
                line([3.5, 3.5, 20.5, 20.5]);
            else
                circle(12, 12, 2.9, false);
        } else if (name === "wifi-status") {
            c.scale(1.5, 1.5);
            c.lineWidth = 1.75;
            c.beginPath();
            c.moveTo(1.5, 6);
            c.quadraticCurveTo(8, 0.5, 14.5, 6);
            c.moveTo(4, 8.6);
            c.quadraticCurveTo(8, 5.2, 12, 8.6);
            c.moveTo(6.4, 11.1);
            c.quadraticCurveTo(8, 9.8, 9.6, 11.1);
            c.stroke();
            circle(8, 13.4, 0.85, true);
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
            c.beginPath();
            c.roundedRect(5.5, 10.5, 13, 9.5, 2.2, 2.2);
            c.stroke();
            c.beginPath();
            c.moveTo(8.5, 10.5);
            c.lineTo(8.5, 8);
            c.arc(12, 8, 3.5, Math.PI, 0);
            c.lineTo(15.5, 10.5);
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
