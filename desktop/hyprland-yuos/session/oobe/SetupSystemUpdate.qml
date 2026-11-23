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
    readonly property bool showingDrivers: softwarePhase === "waiting" || (requiredDriver && softwarePhase !== "repair")
    readonly property bool statusUnavailable: unavailable && !requiredDriver && softwarePhase !== "repair"
    readonly property bool driverError: requiredDriver || ["offline", "failed"].includes(driverPhase)
    readonly property bool softwareError: ["offline", "failed", "restart-failed", "repair"].includes(softwarePhase)

    SetupGroup {
        width: parent.width
        height: status.implicitHeight
        SetupRow {
            id: status
            objectName: "updateStatus"
            width: parent.width
            interactive: false
            strong: true
            minHeight: 64
            inset: 48
            text: {
                if (updates.statusUnavailable)
                    return qsTr("Couldn't check update status. Try again.");
                if (updates.showingDrivers)
                    return ({
                        checking: qsTr("Checking drivers…"),
                        installing: qsTr("Installing drivers…"),
                        ready: qsTr("Drivers ready"),
                        offline: qsTr("Driver updates not installed"),
                        failed: qsTr("Couldn't install driver updates"),
                        "required-offline": qsTr("Required drivers aren't installed"),
                        "required-failed": qsTr("Couldn't install required drivers")
                    })[updates.driverPhase];
                return ({
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
                })[updates.softwarePhase];
            }
            symbol: {
                if (updates.statusUnavailable || (updates.showingDrivers ? updates.driverError : updates.softwareError))
                    return "alert";
                if (updates.showingDrivers)
                    return updates.driverPhase === "ready" ? "check" : "spinner";
                if (["checking", "downloading", "restarting"].includes(updates.softwarePhase))
                    return "spinner";
                return ["installed", "current"].includes(updates.softwarePhase) ? "check" : "";
            }
            symbolInk: symbol === "alert" ? "#c0321f" : symbol === "spinner" ? "#005cfe" : "#0d7a53"
            spinning: symbol === "spinner" && !updates.reduceMotion
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
                return "";
            if (updates.softwarePhase === "restart")
                return qsTr("Restart to install the updates. Your setup choices will be saved.")
            if (updates.softwarePhase === "offline")
                return qsTr("No internet connection. You can finish setup and update later.")
            if (["failed", "restart-failed"].includes(updates.softwarePhase))
                return qsTr("You can finish setup and update later.")
            if (updates.driverError)
                return updates.driverPhase === "offline"
                    ? qsTr("Yu OS will retry optional driver updates when you reconnect.")
                    : qsTr("Optional driver updates weren't installed.")
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
