import QtQuick

Column {
    id: updates
    required property string driverPhase
    required property string softwarePhase
    required property bool unavailable
    required property bool canConnect
    required property bool canRetry
    required property bool reduceMotion
    signal connectRequested
    signal retryRequested
    spacing: 12

    readonly property bool requiredDriver: driverPhase.startsWith("required-")
    readonly property bool driverBusy: ["checking", "installing"].includes(driverPhase) && !unavailable
    readonly property bool softwareBusy: ["checking", "downloading", "restarting"].includes(softwarePhase) && !unavailable
    readonly property bool driverError: requiredDriver || ["offline", "failed"].includes(driverPhase)
    readonly property bool softwareError: unavailable || ["offline", "failed", "restart-failed", "repair"].includes(softwarePhase)

    SetupGroup {
        width: parent.width
        height: rows.implicitHeight
        Column {
            id: rows
            width: parent.width
            SetupRow {
                objectName: "driverStatus"
                width: parent.width
                interactive: false
                strong: true
                minHeight: 64
                inset: 48
                divided: true
                text: ({
                    checking: qsTr("Checking drivers…"),
                    installing: qsTr("Installing drivers…"),
                    ready: qsTr("Drivers ready"),
                    offline: qsTr("Driver updates not installed"),
                    failed: qsTr("Couldn't install driver updates"),
                    "required-offline": qsTr("Required drivers aren't installed"),
                    "required-failed": qsTr("Couldn't install required drivers")
                })[updates.driverPhase]
                symbol: updates.driverError ? "alert" : updates.driverBusy ? "spinner" : updates.driverPhase === "ready" ? "check" : ""
                symbolInk: updates.driverError ? "#c0321f" : updates.driverBusy ? "#005cfe" : "#0d7a53"
                spinning: updates.driverBusy && !updates.reduceMotion
            }
            SetupRow {
                objectName: "softwareStatus"
                width: parent.width
                interactive: false
                strong: true
                minHeight: 64
                inset: 48
                text: ({
                    waiting: qsTr("Update software"),
                    checking: qsTr("Checking for software updates…"),
                    downloading: qsTr("Downloading software updates…"),
                    restart: qsTr("Software updates ready to install"),
                    restarting: qsTr("Restarting to install updates…"),
                    installed: qsTr("Software updates installed"),
                    current: qsTr("Software is up to date"),
                    offline: qsTr("Software updates not installed"),
                    failed: qsTr("Couldn't update software"),
                    "restart-failed": qsTr("Couldn't restart to install updates"),
                    repair: qsTr("System update needs attention")
                })[updates.softwarePhase]
                symbol: updates.softwareError ? "alert" : updates.softwareBusy ? "spinner"
                    : ["installed", "current"].includes(updates.softwarePhase) ? "check" : ""
                symbolInk: updates.softwareError ? "#c0321f" : updates.softwareBusy ? "#005cfe" : "#0d7a53"
                spinning: updates.softwareBusy && !updates.reduceMotion
            }
        }
    }

    SetupHint {
        width: parent.width
        error: updates.driverError || updates.softwareError
        text: {
            if (updates.softwarePhase === "repair")
                return qsTr("Setup can't finish until the system update is repaired.")
            if (updates.driverPhase === "required-offline")
                return qsTr("Connect to the internet to install the drivers needed to start the desktop.")
            if (updates.driverPhase === "required-failed")
                return qsTr("Install the required drivers before continuing.")
            if (updates.unavailable)
                return qsTr("Couldn't check update status. Try again.")
            if (updates.softwarePhase === "restart")
                return qsTr("Restart to install the updates. Your setup choices will be saved.")
            if (updates.softwarePhase === "offline")
                return qsTr("No internet connection. You can finish setup and update later.")
            if (["failed", "restart-failed"].includes(updates.softwarePhase))
                return qsTr("You can finish setup and update later.")
            if (updates.driverError)
                return updates.driverPhase === "offline"
                    ? qsTr("No internet connection. Yu OS will try these driver updates when you reconnect.")
                    : qsTr("These driver updates aren't needed to start the desktop.")
            return "";
        }
    }

    Flow {
        width: parent.width
        spacing: 8
        visible: connectButton.visible || retryButton.visible
        SetupButton {
            id: connectButton
            visible: updates.canConnect
            text: qsTr("Connect to internet")
            onClicked: updates.connectRequested()
        }
        SetupButton {
            id: retryButton
            visible: updates.canRetry
            text: qsTr("Try again")
            onClicked: updates.retryRequested()
        }
    }
}
