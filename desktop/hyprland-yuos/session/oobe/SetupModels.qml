pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts
import Quickshell.Io

Column {
    id: models
    required property var snapshot
    property var drafts: ({})
    property var keys: ({})
    property string opened: "text"
    readonly property var roles: [
        ["text", qsTr("Conversation"), ""],
        ["stt", qsTr("Speech to text"), ""],
        ["tts", qsTr("Text to speech"), ""],
        ["imagegen", qsTr("Image generation"), ""],
        ["embed", qsTr("Embedding"), qsTr("Memory search")],
        ["decision", qsTr("Decision"), qsTr("Quick choices and scoring")]
    ]
    signal request(string action)
    signal reveal(Item item)
    spacing: 12

    function catalogue(role) {
        return snapshot.catalogue[role] ?? {};
    }
    function providers(role) {
        return catalogue(role).providers ?? [];
    }
    function draft(role) {
        return drafts[role] ?? {
            provider: snapshot.savedModels[role]?.provider ?? (providers(role).some(p => p.name === "openai") ? "openai"
                : catalogue(role).default_provider ?? providers(role)[0]?.name ?? ""),
            model: snapshot.savedModels[role]?.model ?? "", extra: {}, dirty: false
        };
    }
    function providerLabel(provider) {
        return ({
            azure: qsTr("Azure OpenAI"), copilot: qsTr("GitHub Copilot"), openai: "OpenAI",
            anthropic: "Anthropic", gemini: "Google Gemini", ollama: "Ollama", edge: qsTr("Edge TTS")
        })[provider.name] ?? provider.label ?? provider.name;
    }
    function providerFor(role) {
        const list = providers(role);
        const name = draft(role).provider;
        return list.find(item => item.name === name) ?? list[0];
    }
    function edit(role, change) {
        drafts = Object.assign({}, drafts, {[role]: Object.assign({}, draft(role), change, {dirty: true})});
    }
    function editExtra(role, key, value) {
        edit(role, {extra: Object.assign({}, draft(role).extra, {[key]: value})});
    }
    function scope(role) {
        const provider = providerFor(role);
        if (!provider?.needs_credential || provider.auth_kind === "oauth_device")
            return "";
        const endpoint = (draft(role).extra.base_url ?? "").trim().replace(/\/+$/, "");
        if (provider.extra_fields?.some(field => field.key === "base_url" && field.required) && !endpoint)
            return "";
        // Never reuse a key across providers, credential types, or different resource roots.
        return [provider.name, provider.default_env || role, endpoint].join("\u0000");
    }
    function keyFor(role) {
        return keys[scope(role)] ?? "";
    }
    function editKey(role, value) {
        const id = scope(role);
        if (!id)
            return;
        keys = Object.assign({}, keys, {[id]: value});
        edit(role, {});
    }
    function suggestions(role) {
        const provider = providerFor(role);
        if (!provider)
            return [];
        return provider.name === "copilot" && snapshot.copilotModels.length ? snapshot.copilotModels
            : (provider.models ?? []).map(item => item.name);
    }
    function summary(role) {
        const saved = snapshot.savedModels[role];
        const value = ready(role) ? {provider: providerFor(role).name, model: draft(role).model} : saved;
        if (!value)
            return qsTr("Not set up");
        const provider = providers(role).find(item => item.name === value.provider);
        return role === "decision" ? value.model
            : (provider ? providerLabel(provider) : value.provider) + " · " + value.model;
    }
    function ready(role) {
        const value = draft(role);
        const provider = providerFor(role);
        if (!value.dirty || !provider || !value.model.trim())
            return false;
        if (provider.auth_kind === "oauth_device")
            return snapshot.signedIn;
        if (provider.needs_credential && !keyFor(role).trim())
            return false;
        return !(provider.extra_fields ?? []).some(field => field.required && !(value.extra[field.key] ?? "").trim());
    }
    function entries() {
        const entries = {};
        for (const [role] of roles) {
            if (ready(role)) {
                const value = draft(role);
                entries[role] = {provider: providerFor(role).name, model: value.model, extra: value.extra, apiKey: keyFor(role)};
            }
        }
        return entries;
    }
    function settle(saved) {
        const next = Object.assign({}, drafts);
        for (const role of Object.keys(saved))
            delete next[role];
        drafts = next;
    }
    function openError(role) {
        if (!role)
            return;
        opened = role;
        Qt.callLater(() => {
            const index = roles.findIndex(item => item[0] === role);
            const row = list.itemAt(index);
            if (row)
                reveal(row);
        });
    }
    function toggle(role) {
        opened = opened === role ? "" : role;
        if (opened !== "text" && ["starting", "authorizing"].includes(snapshot.oauthState))
            request("cancelSignIn");
    }

    Process {
        id: clipboard
        command: ["/usr/bin/env", "-u", "GREETD_SOCK", "/usr/bin/wl-copy"]
        stdinEnabled: true
        onStarted: {
            write(models.snapshot.userCode);
            stdinEnabled = false;
        }
    }
    Controls.Label {
        visible: ["loading", "unavailable"].includes(models.snapshot.agentState)
            || (models.snapshot.agentState === "saveFailed" && !models.snapshot.modelErrorModality)
        text: models.snapshot.agentState === "loading" ? qsTr("Loading providers…") : models.snapshot.agentState === "unavailable"
            ? qsTr("Model settings are unavailable. You can set them up later.") : qsTr("Couldn't apply model settings. Try again.")
        width: parent.width
        wrapMode: Text.Wrap
        color: models.snapshot.agentState === "loading" ? "#667085" : "#c0321f"
    }
    SetupGroup {
        width: parent.width
        height: rows.implicitHeight + 2
        Column {
            id: rows
            x: 1
            y: 1
            width: parent.width - 2
            Repeater {
                id: list
                model: models.roles
                Column {
                    id: card
                    required property int index
                    required property var modelData
                    width: rows.width
                    readonly property string role: modelData[0]
                    readonly property bool expanded: models.opened === role
                    readonly property var draft: models.draft(role)
                    readonly property var provider: models.providerFor(role)
                    readonly property bool oauth: provider?.auth_kind === "oauth_device"
                    readonly property bool failed: models.snapshot.agentState === "saveFailed"
                        && models.snapshot.modelErrorModality === role
                    readonly property var specifications: provider?.extra_fields ?? []
                    readonly property string modelLabel: role === "decision" ? qsTr("System One model")
                        : provider?.name === "azure" ? qsTr("Deployment") : role === "tts" ? qsTr("Voice") : qsTr("Model")

                    RowLayout {
                        width: parent.width
                        spacing: 0
                        SetupRow {
                            Layout.fillWidth: true
                            text: card.modelData[1]
                            detail: card.modelData[2]
                            strong: card.expanded
                            divided: card.index > 0
                            arrow: card.expanded ? "" : "down"
                            trailing: card.expanded ? "" : models.summary(card.role)
                            onClicked: models.toggle(card.role)
                        }
                        SetupButton {
                            id: providerButton
                            visible: card.expanded && !!card.provider && card.role !== "decision"
                            Layout.rightMargin: 16
                            implicitHeight: 36
                            text: card.provider ? models.providerLabel(card.provider) + "  ⌄" : ""
                            onClicked: providerMenu.open()
                            Controls.Popup {
                                id: providerMenu
                                parent: providerButton
                                x: parent.width - width
                                y: parent.height + 5
                                width: Math.min(320, card.width - 32)
                                height: menu.implicitHeight
                                margins: 8
                                padding: 0
                                focus: true
                                background: null
                                contentItem: SetupChoices {
                                    id: menu
                                    maxHeight: 296
                                    items: models.providers(card.role).map(item => ({
                                        id: item.name, title: models.providerLabel(item),
                                        meta: item.auth_kind === "oauth_device" ? qsTr("GitHub sign-in") : ""
                                    }))
                                    selected: card.provider?.name ?? ""
                                    onChoose: value => {
                                        if (card.oauth && ["starting", "authorizing"].includes(models.snapshot.oauthState))
                                            models.request("cancelSignIn");
                                        if (value !== card.provider?.name)
                                            models.edit(card.role, {provider: value, model: "", extra: {}});
                                        providerMenu.close();
                                    }
                                }
                            }
                        }
                        Controls.Label {
                            visible: card.expanded && card.role === "decision"
                            Layout.rightMargin: 16
                            text: qsTr("Local")
                            font.pixelSize: 13
                            color: "#667085"
                        }
                    }
                    Loader {
                        x: 16
                        width: parent.width - 32
                        active: card.expanded && !!card.provider
                        visible: active
                        sourceComponent: editor
                    }
                    Component {
                        id: editor
                        Column {
                            spacing: 8
                            GridLayout {
                                width: parent.width
                                columns: width < 410 ? 1 : 2
                                columnSpacing: 12
                                rowSpacing: 14
                                visible: !card.oauth || models.snapshot.signedIn
                                Repeater {
                                    model: card.specifications
                                    SetupField {
                                        required property int index
                                        required property var modelData
                                        Layout.fillWidth: true
                                        Layout.preferredWidth: index % 2 === 0 ? 2 : 1
                                        label: modelData.key === "base_url" ? qsTr("Endpoint")
                                            : modelData.key === "api_version" ? qsTr("API version") : modelData.label
                                        placeholder: modelData.required ? modelData.placeholder : qsTr("Optional")
                                        text: card.draft.extra[modelData.key] ?? ""
                                        invalid: card.failed && models.snapshot.modelError === modelData.key
                                        maxLength: 256
                                        onEdited: models.editExtra(card.role, modelData.key, text)
                                    }
                                }
                                SetupField {
                                    visible: !!card.provider?.needs_credential && !card.oauth
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 2
                                    label: qsTr("API key")
                                    secret: true
                                    enabled: models.scope(card.role) !== ""
                                    text: models.keyFor(card.role)
                                    invalid: card.failed && models.snapshot.modelError === "apiKey"
                                    maxLength: 4096
                                    onEdited: models.editKey(card.role, text)
                                }
                                Controls.Label {
                                    visible: card.oauth && models.snapshot.signedIn
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 2
                                    text: qsTr("Signed in")
                                    color: "#0d7a53"
                                    topPadding: 20
                                }
                                SetupPicker {
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 1
                                    Layout.columnSpan: card.role === "decision" || (!card.oauth && !card.provider?.needs_credential)
                                        ? parent.columns : 1
                                    label: card.modelLabel
                                    picker: choices.length > 0
                                    choices: models.suggestions(card.role).map(model => ({id: model, title: model}))
                                    selected: text
                                    text: card.draft.model
                                    placeholder: card.provider?.default_model ?? ""
                                    invalid: card.failed && models.snapshot.modelError === "model"
                                    maxLength: card.role === "decision" ? 128 : 256
                                    onEdited: models.edit(card.role, {model: text})
                                    onChosen: value => models.edit(card.role, {model: value})
                                }
                            }
                            Controls.Label {
                                visible: card.role === "decision"
                                text: qsTr("Only the name is saved. Nothing is downloaded or started.")
                                width: parent.width
                                wrapMode: Text.Wrap
                                font.pixelSize: 13
                                color: "#667085"
                            }
                            SetupButton {
                                visible: card.oauth && !models.snapshot.signedIn && models.snapshot.userCode === ""
                                kind: "secondary"
                                enabled: ["idle", "failed"].includes(models.snapshot.oauthState)
                                text: enabled ? qsTr("Sign in with GitHub") : qsTr("Starting sign-in…")
                                onClicked: models.request("signIn")
                            }
                            Column {
                                visible: card.oauth && models.snapshot.userCode !== ""
                                width: parent.width
                                spacing: 8
                                Controls.Label {
                                    width: parent.width
                                    wrapMode: Text.Wrap
                                    text: qsTr("Enter this code at github.com/login/device")
                                    color: "#4b5566"
                                }
                                RowLayout {
                                    width: parent.width
                                    spacing: 8
                                    Controls.Label {
                                        text: models.snapshot.userCode
                                        textFormat: Text.PlainText
                                        font.family: "monospace"
                                        font.pixelSize: 24
                                        font.weight: Font.DemiBold
                                    }
                                    SetupButton {
                                        visible: parent.width > 330
                                        kind: "secondary"
                                        text: qsTr("Copy code")
                                        onClicked: {
                                            clipboard.stdinEnabled = true;
                                            clipboard.running = true;
                                        }
                                    }
                                    SetupButton {
                                        text: qsTr("Cancel")
                                        onClicked: models.request("cancelSignIn")
                                    }
                                }
                                Controls.Label {
                                    text: qsTr("Waiting for sign-in…")
                                    color: "#667085"
                                    font.pixelSize: 13
                                }
                            }
                            Controls.Label {
                                visible: card.failed || (card.oauth && models.snapshot.oauthState === "failed")
                                width: parent.width
                                text: card.oauth && models.snapshot.oauthState === "failed" ? qsTr("Couldn't sign in. Try again.")
                                    : models.snapshot.modelError === "base_url" ? qsTr("Enter the HTTPS resource endpoint, without a deployment path.")
                                    : models.snapshot.modelError === "api_version" ? qsTr("Check the API version.")
                                    : card.role === "decision" ? qsTr("Couldn't save this System One model name.")
                                    : qsTr("Couldn't apply these settings. Check the fields and try again.")
                                font.pixelSize: 13
                                color: "#c0321f"
                                wrapMode: Text.Wrap
                            }
                            Item { width: 1; height: 8 }
                        }
                    }
                }
            }
        }
    }
}
