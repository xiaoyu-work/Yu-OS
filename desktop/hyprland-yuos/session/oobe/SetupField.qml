import QtQuick
import QtQuick.Controls.Basic as Controls

Column {
    id: field
    property string label: ""
    property string name: label
    property bool secret: false
    property bool search: false
    property bool picker: false
    property bool invalid: false
    property bool revealed: false
    property alias text: input.text
    property alias placeholder: input.placeholderText
    property alias maxLength: input.maximumLength
    readonly property alias editing: input.activeFocus
    readonly property alias anchorItem: input
    signal edited()
    signal accepted()
    signal pick()
    signal browse(int direction)
    spacing: 6

    function focusInput() { input.forceActiveFocus(); }

    Text {
        visible: field.label !== ""
        text: field.label
        textFormat: Text.PlainText
        color: "#4b5566"
        font.family: input.font.family
        font.pixelSize: 13
        font.weight: Font.Medium
    }
    Controls.TextField {
        id: input
        width: field.width
        height: 44
        padding: 16
        topPadding: 0
        bottomPadding: 0
        leftPadding: field.search ? 44 : 16
        rightPadding: field.secret || field.picker ? 44 : 16
        color: "#0f1520"
        placeholderTextColor: "#667085"
        selectionColor: "#005cfe"
        selectByMouse: true
        echoMode: field.secret && !field.revealed ? TextInput.Password : TextInput.Normal
        inputMethodHints: Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase | (field.secret ? Qt.ImhSensitiveData : 0)
        Accessible.name: field.name
        onTextEdited: field.edited()
        onAccepted: field.accepted()
        Keys.onDownPressed: event => {
            if (field.picker)
                field.browse(1);
            else
                event.accepted = false;
        }
        Keys.onUpPressed: event => {
            if (field.picker)
                field.browse(-1);
            else
                event.accepted = false;
        }
        onActiveFocusChanged: if (!activeFocus && field.secret) field.revealed = false
        background: Rectangle {
            radius: 10
            color: input.enabled ? "white" : "#f7f8fa"
            border.width: field.invalid || input.activeFocus ? 2 : 1
            border.color: field.invalid ? "#c0321f" : input.activeFocus ? "#005cfe" : "#d1d8e0"
        }
        SetupIcon {
            visible: field.search
            anchors.left: parent.left
            anchors.leftMargin: 16
            anchors.verticalCenter: parent.verticalCenter
            name: "search"
            ink: "#667085"
        }
        Controls.ToolButton {
            id: endButton
            visible: field.secret || field.picker
            anchors.right: parent.right
            anchors.rightMargin: 6
            anchors.verticalCenter: parent.verticalCenter
            width: 32
            height: 32
            hoverEnabled: true
            Accessible.name: field.secret
                ? (field.revealed ? qsTr("Hide %1") : qsTr("Show %1")).arg(field.name)
                : qsTr("Show suggestions")
            onClicked: {
                if (field.secret)
                    field.revealed = !field.revealed;
                else
                    field.pick();
            }
            background: Rectangle {
                radius: 8
                color: endButton.hovered ? "#eef1f4" : "transparent"
                border.width: endButton.visualFocus ? 2 : 0
                border.color: "#005cfe"
            }
            contentItem: SetupIcon {
                anchors.centerIn: parent
                width: 18
                height: 18
                name: field.secret ? (field.revealed ? "eye-off" : "eye") : "down"
            }
        }
    }
}
