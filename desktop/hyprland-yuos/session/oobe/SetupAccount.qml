pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts

Column {
    id: account
    required property string avatar
    required property string username
    required property string password
    required property string confirmation
    required property string error
    signal chooseAvatar(string value)
    signal editName(string value)
    signal editPassword(string value)
    signal editConfirmation(string value)
    signal submit()
    spacing: 0

    Row {
        id: avatars
        width: parent.width
        spacing: Math.max(2, (width - 6 * size) / 5)
        readonly property real size: Math.min(80, (width - 10) / 6)
        Repeater {
            model: [
                ["fox", qsTr("Fox")], ["penguin", qsTr("Penguin")], ["rabbit", qsTr("Rabbit")],
                ["frog", qsTr("Frog")], ["robot", qsTr("Robot")], ["star", qsTr("Star")]
            ]
            Controls.AbstractButton {
                id: avatarButton
                required property var modelData
                width: avatars.size
                height: width
                hoverEnabled: true
                checked: account.avatar === modelData[0]
                Accessible.checked: checked
                Accessible.name: modelData[1]
                Accessible.role: Accessible.RadioButton
                onClicked: account.chooseAvatar(modelData[0])

                background: Rectangle {
                    radius: width / 2
                    color: "transparent"
                    border.width: avatarButton.checked || avatarButton.hovered || avatarButton.visualFocus ? 2 : 0
                    border.color: avatarButton.checked || avatarButton.visualFocus ? "#005cfe" : "#c8d0da"
                }
                contentItem: Item {
                    AccountAvatar {
                        avatarId: avatarButton.modelData[0]
                        anchors.fill: parent
                        anchors.margins: 5
                    }
                }
            }
        }
    }
    Item { width: 1; height: 32 }
    GridLayout {
        width: parent.width
        columns: account.width < 440 ? 1 : 2
        rowSpacing: 20
        columnSpacing: 12

        SetupField {
            objectName: "accountName"
            Layout.fillWidth: true
            Layout.columnSpan: parent.columns
            label: qsTr("Username")
            text: account.username
            maxLength: 256
            invalid: ["usernameTaken", "invalidUsername"].includes(account.error)
            onEdited: account.editName(text)
            onAccepted: account.submit()
        }
        SetupField {
            objectName: "accountPassword"
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            label: qsTr("Password")
            secret: true
            text: account.password
            maxLength: 256
            invalid: ["passwordRejected", "invalidPassword"].includes(account.error)
            onEdited: account.editPassword(text)
            onAccepted: account.submit()
        }
        SetupField {
            objectName: "accountConfirmation"
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            label: qsTr("Confirm password")
            secret: true
            text: account.confirmation
            maxLength: 256
            invalid: !editing && text !== "" && account.password !== text
            onEdited: account.editConfirmation(text)
            onAccepted: account.submit()
        }
    }
    Controls.Label {
        width: parent.width
        topPadding: 8
        visible: text !== ""
        text: ({
            usernameTaken: qsTr("Can't use this username."),
            invalidUsername: qsTr("Can't use this username."),
            invalidPassword: qsTr("Use a password of up to 256 bytes without line breaks."),
            passwordRejected: qsTr("Your system didn't accept this password. Try a different one."),
            invalidAvatar: qsTr("Choose an available avatar.")
        })[account.error] ?? (account.confirmation && account.confirmation !== account.password
            ? qsTr("Passwords don't match.") : "")
        color: "#c0321f"
        font.pixelSize: 13
        wrapMode: Text.Wrap
    }
}
