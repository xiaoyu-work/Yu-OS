pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts
import QtQuick.Window
import Quickshell
import Quickshell.Io
import Quickshell.Services.Greetd

ShellRoot {
    id: setup
    readonly property bool local: Quickshell.env("YUOS_LOCAL_DESKTOP_SETUP") === "1"
    readonly property bool reduced: !!state.reduceMotion
    property string current: local ? "language" : ""
    property string networkPage: "undecided"
    property bool hardwareSeen: false
    property var hardwarePackages: []
    property string phase: "setup"
    property bool passwordSubmitted: false
    property string username: ""
    property string password: ""
    property string confirmation: ""
    property string avatar: "fox"
    property string language: ""
    property string keyboard: ""
    property string zone: ""
    property bool light: true
    property var state: ({
        networkState: "checking", wifiState: "idle", wifiName: "", hasWifi: false, networks: [],
        driverState: "checking", packages: [],
        catalogue: {}, agentState: "idle", savedModels: {}, modelError: "", modelErrorModality: "",
        oauthState: "idle", userCode: "", signedIn: false, copilotModels: [],
        languages: [], keyboards: {}, inputMethods: [], zones: [], zoneClock: {},
        finishError: "", reduceMotion: false, backendUnavailable: false
    })
    readonly property bool connected: ["online", "wired"].includes(state.networkState)
    readonly property bool showsNetwork: networkPage === "shown" || (networkPage === "undecided" && !connected)
    readonly property bool showsHardware: hardwareSeen || (state.packages.length > 0
        && ["pending", "installing", "failed"].includes(state.driverState))
    readonly property var steps: [
        ...(local ? ["language", "keyboard"] : []),
        ...(showsNetwork ? ["network"] : []),
        "models",
        ...(local ? ["account", "location", "appearance"] : []),
        ...(showsHardware ? ["drivers"] : [])
    ]
    readonly property int index: steps.indexOf(current)
    readonly property bool last: index === steps.length - 1
    readonly property bool handingOff: ["working", "authenticating", "launching", "welcomed"].includes(phase)
    readonly property bool busy: handingOff || state.agentState === "saving"
    readonly property bool validAccount: username.trim() !== "" && password !== "" && password === confirmation
    readonly property bool oauthBusy: ["starting", "authorizing", "loadingModels"].includes(state.oauthState)
    readonly property var titles: ({
        language: qsTr("Language"), keyboard: qsTr("Keyboard"), network: qsTr("Network"),
        models: qsTr("Models"), account: qsTr("Account"), location: qsTr("Time zone"),
        appearance: qsTr("Appearance"), drivers: qsTr("Hardware")
    })
    readonly property var components: ({
        language: languagePage, keyboard: keyboardPage, network: wifiPage, models: modelPage,
        account: accountPage, location: locationPage, appearance: appearancePage, drivers: driverPage
    })
    readonly property var languageChoices: {
        const names = {
            en_US: ["English (United States)", ""], en_GB: ["English (United Kingdom)", ""],
            zh_CN: ["简体中文", qsTr("Chinese, Simplified")], zh_TW: ["繁體中文", qsTr("Chinese, Traditional")],
            ja_JP: ["日本語", qsTr("Japanese")], ko_KR: ["한국어", qsTr("Korean")], de_DE: ["Deutsch", qsTr("German")],
            fr_FR: ["Français", qsTr("French")], es_ES: ["Español", qsTr("Spanish")], it_IT: ["Italiano", qsTr("Italian")],
            pt_BR: ["Português (Brasil)", qsTr("Portuguese, Brazil")], ru_RU: ["Русский", qsTr("Russian")]
        };
        const ordered = Object.keys(names).map(id => id + ".utf8");
        return [...ordered.filter(id => state.languages.includes(id)), ...state.languages.filter(id => !ordered.includes(id))]
            .map(id => {
                const code = id.split(".")[0];
                const name = names[code];
                return {id: id, title: name?.[0] ?? Qt.locale(code).nativeLanguageName ?? code, meta: name?.[1] ?? code};
            });
    }
    readonly property var keyboardChoices: {
        const labels = {
            us: [qsTr("English (US)"), "QWERTY"], gb: [qsTr("English (UK)"), "QWERTY"],
            de: [qsTr("German"), "QWERTZ"], fr: [qsTr("French"), "AZERTY"],
            es: [qsTr("Spanish"), "QWERTY"], it: [qsTr("Italian"), "QWERTY"],
            br: [qsTr("Portuguese (Brazil)"), "QWERTY"], jp: [qsTr("Japanese"), "JIS"],
            ru: [qsTr("Russian"), "ЙЦУКЕН"]
        };
        const input = {
            pinyin: qsTr("Chinese (Pinyin)"), mozc: qsTr("Japanese (Mozc)"), hangul: qsTr("Korean (Hangul)")
        };
        return Object.keys(labels).filter(id => !!state.keyboards[id])
            .map(id => ({id: id, title: labels[id][0], meta: labels[id][1], keys: state.keyboards[id]}))
            .concat(state.keyboards.us ? state.inputMethods.map(id => ({
                id: id, title: input[id], meta: qsTr("US keyboard · Fcitx5"), keys: state.keyboards.us
            })) : []);
    }
    readonly property var zoneChoices: state.zones.map(row => {
        const parts = row.id.split("/");
        return {id: row.id, title: parts[parts.length - 1].replace(/_/g, " "),
            meta: parts.slice(0, -1).join(" / "), point: row.point};
    }).sort((a, b) => a.title.localeCompare(b.title))
    readonly property string localTime: {
        if (state.zoneClock.id !== zone)
            return Qt.formatTime(clock.date, Qt.DefaultLocaleShortDate);
        const shifted = new Date(clock.date.getTime() + state.zoneClock.offsetMinutes * 60000);
        const hour = shifted.getUTCHours();
        const minute = String(shifted.getUTCMinutes()).padStart(2, "0");
        if (language === "en_US.utf8")
            return qsTr("%1:%2 %3").arg(hour % 12 || 12).arg(minute).arg(hour >= 12 ? qsTr("PM") : qsTr("AM"));
        return String(hour).padStart(2, "0") + ":" + minute;
    }
    readonly property string greeting: {
        const messages = {
            zh_CN: "欢迎，%1", zh_TW: "歡迎，%1", ja_JP: "ようこそ、%1", ko_KR: "%1님, 환영합니다",
            de_DE: "Willkommen, %1", fr_FR: "Bienvenue, %1", es_ES: "Te damos la bienvenida, %1",
            it_IT: "Ti diamo il benvenuto, %1", pt_BR: "Boas-vindas, %1", ru_RU: "Добро пожаловать, %1"
        };
        return (messages[language.split(".")[0]] ?? qsTr("Welcome, %1")).arg(username.trim());
    }

    function send(action, values) {
        backend.write(JSON.stringify(Object.assign({action: action}, values ?? {})) + "\n");
    }
    function change(next) {
        if (!next)
            return;
        if (next === "network")
            networkPage = "shown";
        if (next === "drivers")
            hardwareSeen = true;
        current = next;
        scroll.contentY = 0;
        if (next === "network")
            send("scan");
        else if (next === "models" && !Object.keys(state.catalogue).length)
            send("models");
        else if (next === "location")
            send("zone", {zone: zone});
    }
    function advance() {
        if (current === "keyboard" && networkPage === "undecided" && connected)
            networkPage = "omitted";
        if (last)
            finish();
        else
            change(steps[index + 1]);
    }
    function pickLanguage(value) {
        language = value;
        const preferred = {
            en_US: "us", en_GB: "gb", zh_CN: "pinyin", zh_TW: "us", ja_JP: "mozc", ko_KR: "hangul",
            de_DE: "de", fr_FR: "fr", es_ES: "es", it_IT: "it", pt_BR: "br", ru_RU: "ru"
        }[language.split(".")[0]];
        if (keyboardChoices.some(row => row.id === preferred))
            keyboard = preferred;
        else if (!keyboard)
            keyboard = keyboardChoices[0]?.id ?? "";
    }
    function receive(message) {
        const previousWifi = state.wifiState;
        const saved = Object.assign({}, state.savedModels, message.savedModels ?? {}, message.appliedModels ?? {});
        state = Object.assign({}, state, message, {savedModels: saved});
        if (message.packages?.length)
            hardwarePackages = message.packages;
        if (message.languages && !language) {
            const preferred = (Quickshell.env("LANG") ?? "").replace(/\.(utf-8)$/i, ".utf8");
            pickLanguage(state.languages.includes(preferred) ? preferred
                : state.languages.includes("en_US.utf8") ? "en_US.utf8" : state.languages[0] ?? "");
        }
        if (message.localZone && !zone) {
            zone = state.zones.some(row => row.id === message.localZone) ? message.localZone : "UTC";
            send("zone", {zone: zone});
        }
        if (!local && current === "" && message.networkState) {
            if (connected)
                networkPage = "omitted";
            change(steps[0]);
        }
        if (current === "network" && previousWifi === "connecting" && message.wifiState === "idle")
            page.item.password = "";
        if (current === "models" && message.agentState === "saveFailed") {
            page.item.settle(message.appliedModels ?? {});
            page.item.openError(message.modelErrorModality);
        } else if (current === "models" && message.agentState === "saved") {
            send("discardModels");
            advance();
        }
        if (message.finishError) {
            phase = "setup";
            if (["invalidUsername", "usernameTaken", "invalidPassword", "passwordRejected", "invalidAvatar"].includes(message.finishError))
                change("account");
        } else if (message.finished && phase === "working") {
            confirmation = "";
            if (!local) {
                Qt.quit();
            } else if (Greetd.available && message.login) {
                phase = "authenticating";
                authWindow.restart();
                Greetd.createSession(message.login);
            } else {
                failedSignIn();
            }
        }
    }
    function finish() {
        if (busy || (local && !validAccount))
            return;
        state = Object.assign({}, state, {finishError: ""});
        phase = "working";
        if (!local) {
            send("finish");
            return;
        }
        const ime = state.inputMethods.includes(keyboard);
        send("finish", {
            username: username, password: password, avatar: avatar, locale: language,
            keyboard: ime ? "us" : keyboard, inputMethod: ime ? keyboard : "", zone: zone, light: light
        });
    }
    function clearError() {
        if (state.finishError)
            state = Object.assign({}, state, {finishError: ""});
    }
    function failedSignIn() {
        password = "";
        confirmation = "";
        phase = "signIn";
        authWindow.stop();
    }
    function continueSetup() {
        if (current === "models") {
            state = Object.assign({}, state, {agentState: "saving", modelError: "", modelErrorModality: ""});
            send("saveModels", {entries: page.item.entries()});
        } else {
            advance();
        }
    }

    Component.onCompleted: Quickshell.watchFiles = false

    SystemClock { id: clock; precision: SystemClock.Minutes }
    Timer {
        interval: 30000
        repeat: true
        running: setup.current === "network" && !setup.busy && setup.state.wifiState !== "connecting"
        onTriggered: setup.send("scan")
    }
    Connections {
        target: clock
        enabled: setup.local && setup.zone !== ""
        function onDateChanged() { setup.send("zone", {zone: setup.zone}); }
    }
    Timer {
        id: authWindow
        interval: 30000
        onTriggered: {
            const wasAuthenticating = setup.phase === "authenticating";
            setup.failedSignIn();
            if (wasAuthenticating && Greetd.state !== GreetdState.Inactive)
                Greetd.cancelSession();
        }
    }
    Timer { id: handoff; interval: setup.reduced ? 250 : 620; onTriggered: Qt.quit() }
    Connections {
        target: setup.local ? Greetd : null
        enabled: setup.phase === "authenticating" || setup.phase === "launching"

        function onAuthMessage(message, error, responseRequired, echoResponse) {
            if (!responseRequired)
                return;
            if (echoResponse || setup.password === "" || setup.passwordSubmitted) {
                setup.failedSignIn();
                Greetd.cancelSession();
                return;
            }
            const answer = setup.password;
            setup.password = "";
            setup.passwordSubmitted = true;
            Greetd.respond(answer);
        }
        function onAuthFailure(message) { setup.failedSignIn(); }
        function onError(message) { setup.failedSignIn(); }
        function onReadyToLaunch() {
            if (!setup.passwordSubmitted) {
                setup.failedSignIn();
                Greetd.cancelSession();
                return;
            }
            setup.phase = "launching";
            Greetd.launch(["/usr/bin/systemd-cat", "--identifier=yuos-desktop",
                           "/usr/local/bin/yuos-display-session", "--desktop"], ["XDG_SESSION_TYPE=wayland"], false);
        }
        function onLaunched() {
            authWindow.stop();
            setup.phase = "welcomed";
            // greetd starts the desktop when the greeter exits. Keep this final hold short.
            handoff.start();
        }
    }

    Process {
        id: backend
        running: true
        stdinEnabled: true
        command: ["/usr/bin/env", "-u", "GREETD_SOCK", "/usr/libexec/yuos-oobe", "--backend"]
        stdout: SplitParser { onRead: line => setup.receive(JSON.parse(line)) }
        onExited: () => {
            if (setup.phase === "setup" || setup.phase === "working") {
                setup.password = "";
                setup.confirmation = "";
                setup.phase = "setup";
                setup.state = Object.assign({}, setup.state, {backendUnavailable: true});
            }
        }
    }

    Controls.ApplicationWindow {
        id: window
        title: qsTr("Set up Yu OS")
        flags: Qt.FramelessWindowHint
        visibility: Window.FullScreen
        width: 1280
        height: 800
        color: "#f3f5f7"
        font.family: "Inter"
        font.pixelSize: 15
        onClosing: event => event.accepted = !setup.local || setup.phase === "signIn"

        readonly property real edge: Math.max(24, Math.min(80, width * 0.0625))
        readonly property real vEdge: Math.max(18, Math.min(48, height * 0.06))
        readonly property bool compact: width < 800

        Rectangle {
            anchors.fill: parent
            color: setup.light ? "#f3f5f7" : "#15181c"
            opacity: setup.handingOff ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: setup.reduced ? 0 : 280 } }
        }
        Item {
            id: surface
            anchors.horizontalCenter: parent.horizontalCenter
            width: Math.min(1120, parent.width - 2 * window.edge)
            height: parent.height
            opacity: setup.handingOff ? 0 : 1
            visible: opacity > 0
            enabled: !setup.handingOff
            Behavior on opacity { NumberAnimation { duration: setup.reduced ? 0 : 240 } }

            Item {
                id: header
                anchors.top: parent.top
                anchors.topMargin: window.vEdge
                width: parent.width
                height: 32
                Row {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 4
                    visible: setup.index >= 0
                    Accessible.role: Accessible.ProgressBar
                    Accessible.name: qsTr("Step %1 of %2").arg(setup.index + 1).arg(setup.steps.length)
                    Repeater {
                        model: setup.steps.length
                        Rectangle {
                            required property int index
                            width: index === setup.index ? 24 : 16
                            height: 4
                            radius: 2
                            color: index === setup.index ? "#005cfe" : index < setup.index ? "#0f1520" : "#d5dbe2"
                        }
                    }
                }
            }
            Item {
                id: body
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: header.bottom
                anchors.bottom: footer.top
                anchors.topMargin: Math.min(60, window.height * 0.075)
                anchors.bottomMargin: anchors.topMargin
                readonly property real gap: Math.min(80, width / 14)

                Controls.Label {
                    id: title
                    width: window.compact ? parent.width : Math.round(parent.width * 400 / 1120)
                    y: window.compact ? 0 : (parent.height - height) / 2
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    color: "#0f1520"
                    font.pixelSize: window.compact ? 30 : 40
                    font.weight: Font.DemiBold
                    text: setup.titles[setup.current] ?? qsTr("Set up Yu OS")
                }
                Flickable {
                    id: scroll
                    x: window.compact ? 0 : title.width + body.gap
                    width: parent.width - x
                    y: window.compact ? title.height + 16 : 0
                    height: Math.max(0, parent.height - y)
                    contentWidth: width
                    contentHeight: Math.max(height, work.implicitHeight)
                    boundsBehavior: Flickable.StopAtBounds
                    clip: true
                    Controls.ScrollBar.vertical: Controls.ScrollBar {}

                    Column {
                        id: work
                        y: Math.max(0, (scroll.height - implicitHeight) / 2)
                        width: scroll.width
                        spacing: 8
                        Loader {
                            id: page
                            width: work.width
                            sourceComponent: setup.components[setup.current] ?? null
                            enabled: !setup.busy && setup.phase !== "signIn"
                        }
                        Controls.Label {
                            width: work.width
                            visible: text !== ""
                            wrapMode: Text.Wrap
                            color: "#c0321f"
                            text: {
                                if (setup.phase === "signIn")
                                    return qsTr("Your account is ready, but automatic sign-in didn't finish. Sign in with your password.");
                                if (setup.state.backendUnavailable)
                                    return qsTr("Setup is unavailable. Restart this computer to try again.");
                                const error = setup.state.finishError;
                                if (!error || (setup.current === "account" && [
                                    "invalidUsername", "usernameTaken", "invalidPassword", "passwordRejected", "invalidAvatar"
                                ].includes(error)))
                                    return "";
                                return qsTr("Setup couldn't finish. Try again.");
                            }
                        }
                    }
                }
            }
            RowLayout {
                id: footer
                anchors.bottom: parent.bottom
                anchors.bottomMargin: window.vEdge
                width: parent.width
                height: 44
                spacing: 8

                SetupButton {
                    visible: setup.index > 0 && setup.phase !== "signIn"
                    enabled: !setup.busy
                    kind: "back"
                    symbol: "left"
                    text: qsTr("Back")
                    onClicked: {
                        if (setup.current === "models")
                            setup.send("discardModels");
                        setup.clearError();
                        setup.change(setup.steps[setup.index - 1]);
                    }
                }
                Item { Layout.fillWidth: true }
                SetupButton {
                    visible: setup.current === "models" && setup.phase !== "signIn"
                    enabled: !setup.busy && !setup.state.backendUnavailable
                    text: qsTr("Set up later")
                    onClicked: {
                        setup.send("discardModels");
                        setup.advance();
                    }
                }
                SetupButton {
                    id: next
                    objectName: "setupContinue"
                    visible: setup.index >= 0
                    kind: setup.current === "network" && !setup.connected ? "secondary" : "primary"
                    enabled: setup.phase === "signIn" || (!setup.busy && !setup.state.backendUnavailable
                        && (setup.current !== "language" || !!setup.language)
                        && (setup.current !== "keyboard" || (!!setup.keyboard && setup.state.networkState !== "checking"))
                        && (setup.current !== "models" || (!setup.oauthBusy && setup.state.agentState !== "loading"))
                        && (setup.current !== "account" || setup.validAccount)
                        && (setup.current !== "location" || !!setup.zone))
                    text: setup.phase === "signIn" ? qsTr("Go to sign-in")
                        : setup.state.agentState === "saving" ? qsTr("Saving…")
                        : setup.current === "network" && !setup.connected ? qsTr("Continue offline")
                        : setup.last && setup.current !== "models" ? qsTr("Finish setup") : qsTr("Continue")
                    onClicked: setup.phase === "signIn" ? Qt.quit() : setup.continueSetup()
                }
            }
        }

        SetupMark {
            id: mark
            readonly property real wallpaperUnit: 0.72 * Math.max(window.width / 3840, window.height / 2160)
            x: setup.handingOff ? (window.width - width) / 2 : surface.x
            y: setup.handingOff ? (window.height - height) / 2 : window.vEdge + 4
            width: setup.handingOff ? 563 * wallpaperUnit : 26
            height: width * 524 / 563
            ink: setup.handingOff && !setup.light ? "#eef2f7" : "#1e1e1e"
            working: setup.handingOff && setup.phase !== "welcomed" && !setup.reduced
            Behavior on x { NumberAnimation { duration: setup.reduced ? 0 : 400; easing.type: Easing.InOutCubic } }
            Behavior on y { NumberAnimation { duration: setup.reduced ? 0 : 400; easing.type: Easing.InOutCubic } }
            Behavior on width { NumberAnimation { duration: setup.reduced ? 0 : 400; easing.type: Easing.InOutCubic } }
        }
        Controls.Label {
            objectName: "welcomeName"
            anchors.top: mark.bottom
            anchors.topMargin: Math.min(40, window.height * 0.05)
            anchors.horizontalCenter: parent.horizontalCenter
            width: surface.width
            horizontalAlignment: Text.AlignHCenter
            color: setup.light ? "#0f1520" : "#eef2f7"
            font.weight: Font.DemiBold
            font.pixelSize: window.compact ? 30 : 40
            textFormat: Text.PlainText
            text: setup.phase === "welcomed" ? setup.greeting : ""
            opacity: setup.phase === "welcomed" ? 1 : 0
            visible: opacity > 0
            elide: Text.ElideRight
            Behavior on opacity { NumberAnimation { duration: setup.reduced ? 0 : 140 } }
        }
    }

    Component {
        id: languagePage
        SetupLanguage {
            installed: setup.languageChoices
            selected: setup.language
            availableHeight: scroll.height
            onChoose: value => setup.pickLanguage(value)
        }
    }
    Component {
        id: keyboardPage
        SetupKeyboard {
            layouts: setup.keyboardChoices
            selected: setup.keyboard
            onChoose: value => setup.keyboard = value
        }
    }
    Component {
        id: wifiPage
        SetupNetwork {
            snapshot: setup.state
            onConnectTo: (name, password) => {
                setup.state = Object.assign({}, setup.state, {wifiState: "connecting", wifiName: name});
                setup.send("wifi", {network: name, password: password});
            }
        }
    }
    Component {
        id: modelPage
        SetupModels {
            snapshot: setup.state
            onRequest: action => setup.send(action)
            onReveal: item => {
                const y = item.mapToItem(scroll.contentItem, 0, 0).y;
                scroll.contentY = Math.max(0, Math.min(y, scroll.contentHeight - scroll.height));
            }
        }
    }
    Component {
        id: accountPage
        SetupAccount {
            avatar: setup.avatar
            username: setup.username
            password: setup.password
            confirmation: setup.confirmation
            error: setup.state.finishError
            onChooseAvatar: value => { setup.avatar = value; setup.clearError(); }
            onEditName: value => { setup.username = value; setup.clearError(); }
            onEditPassword: value => { setup.password = value; setup.clearError(); }
            onEditConfirmation: value => setup.confirmation = value
            onSubmit: if (next.enabled) setup.advance()
        }
    }
    Component {
        id: locationPage
        SetupLocation {
            choices: setup.zoneChoices
            selected: setup.zone
            clock: setup.state.zoneClock
            localTime: setup.localTime
            onChoose: value => { setup.zone = value; setup.send("zone", {zone: value}); }
        }
    }
    Component {
        id: appearancePage
        SetupAppearance {
            light: setup.light
            currentTime: setup.localTime
            onChoose: value => setup.light = value
        }
    }
    Component {
        id: driverPage
        SetupHardware {
            status: setup.state.driverState
            packages: setup.hardwarePackages
            reduceMotion: setup.reduced
        }
    }
}
