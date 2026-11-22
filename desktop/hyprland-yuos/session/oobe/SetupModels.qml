pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts
import Quickshell.Io

Column {
    id: models
    required property var snapshot
    required property bool canOpenLinks
    required property bool reduceMotion
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
    readonly property var providerOrder: ({
        text: ["openai", "anthropic", "gemini", "copilot", "ollama", "azure", "bedrock", "deepseek", "openrouter", "xai"],
        stt: ["openai", "azure", "groq", "mistral"]
    })
    signal request(string action)
    signal reveal(Item item)
    signal clearSaveError()
    signal beforeExpansion()
    spacing: 12

    function catalogue(role) {
        return snapshot.catalogue[role] ?? {};
    }
    function providers(role) {
        return catalogue(role).providers ?? [];
    }
    function menuProviders(role) {
        const order = providerOrder[role];
        const list = providers(role);
        if (!order)
            return list;
        return order.map(name => list.find(item => item.name === name)).filter(item => item)
            .concat(list.filter(item => !order.includes(item.name)));
    }
    function draft(role) {
        if (drafts[role])
            return drafts[role];
        const available = providers(role);
        const saved = snapshot.savedModels[role];
        const name = saved?.provider ?? (role === "embed" && available.some(p => p.name === "local") ? "local"
            : catalogue(role).default_provider ?? menuProviders(role)[0]?.name ?? "");
        const provider = available.find(item => item.name === name);
        return {provider: name, model: saved?.model ?? provider?.default_model ?? "", extra: {}, dirty: false};
    }
    function providerLabel(provider) {
        return ({
            azure: qsTr("Azure OpenAI"), copilot: qsTr("GitHub Copilot"), openai: "OpenAI",
            anthropic: "Anthropic", gemini: "Google Gemini", ollama: "Ollama", edge: "Microsoft Edge",
            bedrock: "Amazon Bedrock", deepseek: "DeepSeek", openrouter: "OpenRouter", xai: "xAI",
            groq: "Groq", mistral: "Mistral", elevenlabs: "ElevenLabs", minimax: "MiniMax", fal: "fal.ai",
            local: qsTr("Local")
        })[provider.name] ?? provider.label ?? provider.name;
    }
    function providerFor(role) {
        const list = providers(role);
        const name = draft(role).provider;
        return list.find(item => item.name === name) ?? list[0];
    }
    function edit(role, change) {
        drafts = Object.assign({}, drafts, {[role]: Object.assign({}, draft(role), change, {dirty: true})});
        if (snapshot.agentState === "saveFailed")
            clearSaveError();
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
        return [provider.name, provider.name === "azure" ? role : provider.default_env || role, endpoint].join("\u0000");
    }
    function keyFor(role) {
        return draft(role).pendingKey ?? keys[scope(role)] ?? "";
    }
    function editKey(role, value) {
        const id = scope(role);
        if (!id) {
            edit(role, {pendingKey: value});
            return;
        }
        keys = Object.assign({}, keys, {[id]: value});
        edit(role, {pendingKey: undefined});
    }
    function bindPendingKey(role) {
        const key = draft(role).pendingKey;
        if (key && scope(role))
            editKey(role, key);
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
                bindPendingKey(role);
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
            if (row) {
                reveal(row);
                row.focusError();
            }
        });
    }
    function toggle(role) {
        beforeExpansion();
        opened = role;
        if (snapshot.agentState === "saveFailed")
            clearSaveError();
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
        height: rows.implicitHeight
        Column {
            id: rows
            width: parent.width
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
                    readonly property bool ollama: provider?.name === "ollama"
                    readonly property string failure: models.snapshot.agentState === "saveFailed"
                        && models.snapshot.modelErrorModality === role ? models.snapshot.modelError : ""
                    readonly property var specifications: provider?.extra_fields ?? []
                    readonly property string modelLabel: provider?.name === "azure" ? qsTr("Deployment")
                        : provider?.name === "edge" ? qsTr("Voice") : qsTr("Model")
                    function focusError() {
                        fields.item?.focusError();
                    }

                    Item {
                        width: parent.width
                        height: modelRow.implicitHeight
                        SetupRow {
                            id: modelRow
                            width: parent.width
                            text: card.modelData[1]
                            detail: card.modelData[2]
                            strong: card.expanded
                            divided: card.index > 0
                            roundTop: card.index === 0
                            roundBottom: card.index + 1 === models.roles.length && !card.expanded
                            interactive: !card.expanded
                            arrow: card.expanded ? "" : "down"
                            trailing: card.expanded ? (card.role === "decision" ? qsTr("Local") : "") : models.summary(card.role)
                            trailingInk: !card.expanded && (models.ready(card.role) || models.snapshot.savedModels[card.role])
                                ? "#4b5566" : "#667085"
                            onClicked: {
                                models.toggle(card.role);
                                Qt.callLater(() => {
                                    if (providerButton.visible)
                                        providerButton.forceActiveFocus(Qt.MouseFocusReason);
                                    else
                                        fields.item?.focusModel();
                                });
                            }
                        }
                        SetupButton {
                            id: providerButton
                            visible: card.expanded && !!card.provider && card.role !== "decision"
                            anchors.right: parent.right
                            anchors.rightMargin: 16
                            anchors.verticalCenter: parent.verticalCenter
                            kind: "menu"
                            symbol: "down"
                            menuOpen: providerMenu.opened
                            text: card.provider ? models.providerLabel(card.provider) : ""
                            Accessible.name: qsTr("%1 provider: %2").arg(card.modelData[1]).arg(text)
                            onClicked: providerMenu.opened ? providerMenu.close() : providerMenu.open()
                            Controls.Popup {
                                id: providerMenu
                                parent: providerButton
                                x: parent.width - width - 10
                                y: {
                                    const windowHeight = models.Window.window?.height ?? 800;
                                    const top = parent.mapToItem(null, 0, 0).y;
                                    const below = parent.height + 8;
                                    return top + below + height > windowHeight - Math.min(124, windowHeight * 0.155)
                                        && top >= height + 8 ? -height - 8 : below;
                                }
                                width: Math.min(320, card.width - 32)
                                height: menu.implicitHeight
                                margins: 8
                                padding: 0
                                focus: true
                                closePolicy: Controls.Popup.CloseOnEscape | Controls.Popup.CloseOnPressOutsideParent
                                onOpened: menu.focusRow(Math.max(0, menu.items.findIndex(item => item.id === menu.selected)))
                                background: null
                                contentItem: SetupChoices {
                                    id: menu
                                    focus: true
                                    floating: true
                                    maxHeight: 296
                                    Keys.onDownPressed: moveFocus(1)
                                    Keys.onUpPressed: moveFocus(-1)
                                    Keys.onReturnPressed: if (activeIndex >= 0) choose(items[activeIndex].id)
                                    Keys.onEnterPressed: if (activeIndex >= 0) choose(items[activeIndex].id)
                                    items: models.menuProviders(card.role).map(item => ({
                                        id: item.name, title: models.providerLabel(item),
                                        meta: item.auth_kind === "oauth_device" ? qsTr("GitHub sign-in")
                                            : item.name === "ollama" ? qsTr("Local")
                                            : ["edge", "local"].includes(item.name) ? qsTr("Free") : ""
                                    }))
                                    selected: card.provider?.name ?? ""
                                    onChoose: value => {
                                        if (card.oauth && ["starting", "authorizing"].includes(models.snapshot.oauthState))
                                            models.request("cancelSignIn");
                                        if (value !== card.provider?.name) {
                                            models.beforeExpansion();
                                            const selected = models.providers(card.role).find(provider => provider.name === value);
                                            models.edit(card.role, {provider: value, model: selected?.default_model ?? "",
                                                extra: {}, pendingKey: undefined});
                                        }
                                        providerMenu.close();
                                    }
                                }
                            }
                        }
                    }
                    Loader {
                        id: fields
                        x: 16
                        width: parent.width - 32
                        active: card.expanded && !!card.provider
                        visible: active
                        sourceComponent: editor
                    }
                    Component {
                        id: editor
                        Column {
                            function focusModel() {
                                modelPicker.focusInput();
                            }

                            function focusError() {
                                for (let i = 0; i < extraFields.count; i++) {
                                    const field = extraFields.itemAt(i);
                                    if (field.errorText) {
                                        field.focusInput();
                                        return;
                                    }
                                }
                                if (apiKey.errorText)
                                    apiKey.focusInput();
                                else if (modelPicker.errorText)
                                    modelPicker.focusInput();
                            }
                            spacing: 8
                            GridLayout {
                                width: parent.width
                                columns: width < 410 ? 1 : 2
                                columnSpacing: 12
                                rowSpacing: 20
                                visible: !card.oauth || models.snapshot.signedIn
                                Repeater {
                                    id: extraFields
                                    model: card.specifications
                                    SetupField {
                                        required property int index
                                        required property var modelData
                                        Layout.fillWidth: true
                                        Layout.alignment: Qt.AlignTop
                                        Layout.preferredWidth: !card.ollama && index % 2 === 0 ? 2 : 1
                                        Layout.row: card.ollama ? (parent.columns === 1 ? 1 : 0) : -1
                                        Layout.column: card.ollama ? parent.columns - 1 : -1
                                        label: modelData.key === "base_url" ? qsTr("Endpoint")
                                            : modelData.key === "api_version" ? qsTr("API version") : modelData.label
                                        placeholder: modelData.key === "base_url" && card.provider?.name === "azure"
                                            ? "https://name.openai.azure.com"
                                            : card.ollama || modelData.required ? modelData.placeholder : qsTr("Optional")
                                        text: card.draft.extra[modelData.key] ?? ""
                                        errorText: card.failure !== modelData.key ? "" : modelData.key === "api_version"
                                            ? qsTr("Check the API version.") : card.provider?.name === "azure"
                                                ? qsTr("Enter the HTTPS resource endpoint, without a deployment path.")
                                                : qsTr("Enter a valid endpoint.")
                                        maxLength: 256
                                        onEdited: models.editExtra(card.role, modelData.key, text)
                                        onEditingChanged: if (modelData.key === "base_url" && !editing) models.bindPendingKey(card.role)
                                    }
                                }
                                SetupField {
                                    id: apiKey
                                    visible: !!card.provider?.needs_credential && !card.oauth
                                    Layout.fillWidth: true
                                    Layout.alignment: Qt.AlignTop
                                    Layout.preferredWidth: 2
                                    label: qsTr("API key")
                                    secret: true
                                    revealScope: models.scope(card.role)
                                    text: models.keyFor(card.role)
                                    errorText: card.failure === "apiKey" || (card.failure === "write" && visible)
                                        ? qsTr("Couldn't save the model. Check the details.") : ""
                                    maxLength: 4096
                                    onEdited: models.editKey(card.role, text)
                                }
                                Column {
                                    visible: card.oauth && models.snapshot.signedIn
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 2
                                    spacing: 6
                                    Controls.Label {
                                        text: qsTr("GitHub")
                                        color: "#4b5566"
                                        font.pixelSize: 13
                                        font.weight: Font.Medium
                                        height: 18
                                        verticalAlignment: Text.AlignVCenter
                                    }
                                    Row {
                                        height: 44
                                        spacing: 8
                                        SetupIcon {
                                            anchors.verticalCenter: parent.verticalCenter
                                            width: 18
                                            height: 18
                                            name: "check"
                                            ink: "#0d7a53"
                                        }
                                        Controls.Label {
                                            anchors.verticalCenter: parent.verticalCenter
                                            text: qsTr("Signed in")
                                            color: "#0d7a53"
                                        }
                                    }
                                }
                                SetupPicker {
                                    id: modelPicker
                                    Layout.fillWidth: true
                                    Layout.alignment: Qt.AlignTop
                                    Layout.preferredWidth: 1
                                    Layout.row: card.ollama ? 0 : -1
                                    Layout.column: card.ollama ? 0 : -1
                                    Layout.columnSpan: card.role === "decision" || (!card.oauth && !card.ollama && !card.provider?.needs_credential)
                                        ? parent.columns : 1
                                    label: card.modelLabel
                                    picker: choices.length > 0
                                    choices: models.suggestions(card.role).map(model => ({id: model, title: model}))
                                    selected: text
                                    text: card.draft.model
                                    placeholder: card.provider?.name === "openrouter" ? "provider/model"
                                        : card.provider?.name === "bedrock" ? qsTr("Model ID") : ""
                                    errorText: card.failure === "model" ? qsTr("Check the model name.") : ""
                                    maxLength: card.role === "decision" ? 128 : 256
                                    onEdited: models.edit(card.role, {model: text})
                                    onChosen: value => models.edit(card.role, {model: value})
                                }
                            }
                            Controls.Label {
                                visible: card.role === "decision"
                                text: qsTr("Only the name is saved. Nothing is downloaded or started.")
                                width: parent.width
                                topPadding: 2
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
                                visible: card.oauth && !models.snapshot.signedIn && models.snapshot.userCode !== ""
                                width: parent.width
                                spacing: 12
                                Controls.Label {
                                    width: parent.width
                                    wrapMode: Text.Wrap
                                    text: qsTr("Enter this code at github.com/login/device")
                                    color: "#4b5566"
                                    font.pixelSize: 13
                                    font.weight: Font.Medium
                                    height: Math.max(18, implicitHeight)
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
                                        font.letterSpacing: 2.88
                                        rightPadding: 8
                                    }
                                    SetupButton {
                                        kind: "square"
                                        symbol: "copy"
                                        Accessible.name: qsTr("Copy code")
                                        onClicked: {
                                            clipboard.stdinEnabled = true;
                                            clipboard.running = true;
                                        }
                                    }
                                    SetupButton {
                                        visible: models.canOpenLinks && parent.width > 330
                                        kind: "secondary"
                                        symbol: "external"
                                        trailingSymbol: true
                                        text: qsTr("Open GitHub")
                                        onClicked: Qt.openUrlExternally("https://github.com/login/device")
                                    }
                                }
                                SetupHint {
                                    width: parent.width
                                    text: qsTr("Waiting for sign-in…")
                                    busy: true
                                    reduceMotion: models.reduceMotion
                                }
                            }
                            SetupHint {
                                width: parent.width
                                error: true
                                text: card.oauth && models.snapshot.oauthState === "failed" ? qsTr("Couldn't sign in. Try again.")
                                    : ["", "base_url", "api_version", "apiKey", "model"].includes(card.failure) || apiKey.errorText ? ""
                                    : card.role === "decision" ? qsTr("Couldn't save this System One model name.")
                                    : qsTr("Couldn't save the model. Check the details.")
                            }
                            Item { width: 1; height: 8 }
                        }
                    }
                }
            }
        }
    }
}
