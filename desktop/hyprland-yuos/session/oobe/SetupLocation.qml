import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts
import "WorldDots.js" as World

Column {
    id: location
    required property var choices
    required property string selected
    required property var clock
    required property string localTime
    required property var formatTime
    readonly property var place: choices.find(row => row.id === selected)
    readonly property bool clockReady: clock.id === selected && !!selected
    signal choose(string value)
    spacing: 12

    function offsetText(minutes) {
        const value = Math.abs(minutes);
        const remainder = value % 60;
        return "GMT" + (value === 0 ? "" : (minutes < 0 ? "−" : "+") + Math.floor(value / 60)
            + (remainder ? ":" + String(remainder).padStart(2, "0") : ""));
    }

    SetupPicker {
        width: parent.width
        search: true
        browseOnFocus: false
        resultLimit: 6
        name: qsTr("Search cities")
        placeholder: qsTr("Search")
        choices: location.choices.map(row => Object.assign({}, row, {
            meta: Number.isFinite(row.offsetMinutes) ? location.formatTime(row.offsetMinutes) : "",
            search: Number.isFinite(row.offsetMinutes) ? location.offsetText(row.offsetMinutes).replace("−", "-") : ""
        }))
        selected: location.selected
        onChosen: value => location.choose(value)
    }
    SetupGroup {
        width: parent.width
        height: card.implicitHeight
        Column {
            id: card
            width: parent.width
            Item {
                width: parent.width
                height: map.height + 32
                Canvas {
                    id: map
                    x: 16
                    y: 16
                    width: parent.width - 32
                    readonly property var dataGrid: World.grid
                    readonly property var point: location.place?.point ?? null
                    readonly property real offset: location.clockReady ? location.clock.offsetMinutes / 4 : 0
                    height: width * dataGrid.rows / dataGrid.columns
                    onPointChanged: requestPaint()
                    onOffsetChanged: requestPaint()
                    onWidthChanged: requestPaint()
                    TapHandler {
                        onTapped: point => {
                            const lon = point.position.x / map.width * 360 - 180;
                            const lat = map.dataGrid.north - point.position.y / map.height
                                * (map.dataGrid.north - map.dataGrid.south);
                            let nearest = "";
                            let distance = Infinity;
                            for (const zone of location.choices) {
                                if (!zone.point)
                                    continue;
                                const vertical = zone.point[0] - lat;
                                const horizontal = ((zone.point[1] - lon + 540) % 360 - 180) * Math.cos(lat * Math.PI / 180);
                                const candidate = vertical * vertical + horizontal * horizontal;
                                if (candidate < distance) {
                                    distance = candidate;
                                    nearest = zone.id;
                                }
                            }
                            if (nearest)
                                location.choose(nearest);
                        }
                    }
                    onPaint: {
                        const c = getContext("2d");
                        c.reset();
                        const cell = width / dataGrid.columns;
                        const scale = width / 608;
                        function dot(x, y, radius, color) {
                            c.beginPath();
                            c.arc(x, y, radius * scale, 0, Math.PI * 2);
                            c.fillStyle = color;
                            c.fill();
                        }
                        for (const [col, row] of dataGrid.dots) {
                            const lon = -180 + (col + 0.5) * dataGrid.step;
                            const band = Math.abs(((lon - offset + 540) % 360) - 180) <= 7.5;
                            dot((col + 0.5) * cell, (row + 0.5) * cell, 1.45, band ? "#93b4f5" : "#ccd3dc");
                        }
                        if (point) {
                            const x = (point[1] + 180) / 360 * width;
                            const y = Math.max(0, Math.min(height, (dataGrid.north - point[0])
                                / (dataGrid.north - dataGrid.south) * height));
                            dot(x, y, 15, "#24005cfe");
                            dot(x, y, 6.75, "white");
                            dot(x, y, 4.5, "#005cfe");
                        }
                    }
                }
            }
            Rectangle { width: parent.width; height: 1; color: "#ebeef2" }
            RowLayout {
                width: parent.width - 32
                x: 16
                height: 63
                Column {
                    Layout.fillWidth: true
                    Controls.Label {
                        width: parent.width
                        text: location.place?.title ?? ""
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.weight: Font.DemiBold
                        color: "#0f1520"
                        height: 20
                        verticalAlignment: Text.AlignVCenter
                    }
                    Controls.Label {
                        width: parent.width
                        visible: location.clockReady
                        text: visible ? location.clock.abbr + " · " + location.offsetText(location.clock.offsetMinutes) : ""
                        textFormat: Text.PlainText
                        font.pixelSize: 13
                        height: 18
                        verticalAlignment: Text.AlignVCenter
                        color: "#667085"
                    }
                }
                Controls.Label {
                    text: location.clockReady ? location.localTime : ""
                    font.pixelSize: 22
                    font.weight: Font.DemiBold
                    font.letterSpacing: -0.26
                    color: "#0f1520"
                }
            }
        }
    }
}
