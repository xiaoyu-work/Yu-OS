import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Window

SetupField {
    id: choiceField
    property var choices: []
    property string selected: ""
    property bool browseOnFocus: true
    property int resultLimit: 0
    property string preferred: ""
    readonly property bool popupOpen: pop.opened
    signal chosen(string value)
    picker: true

    function select(value) {
        if (value !== "") {
            chosen(value);
            if (search)
                text = "";
        }
        pop.close();
    }

    onPick: {
        if (pop.opened) {
            pop.close();
        } else {
            focusInput();
            pop.open();
        }
    }
    onEdited: {
        results.focusRow(0);
        if (choiceField.picker && (browseOnFocus || text.trim() !== "") && !pop.opened)
            pop.open();
        else if (!browseOnFocus && text.trim() === "")
            pop.close();
    }
    onAccepted: if (pop.opened && filtered.length) select(filtered[results.activeIndex]?.id ?? filtered[0].id)
    onEditingChanged: {
        if (choiceField.picker && editing && browseOnFocus && !pop.opened)
            pop.open();
        else if (!editing && pop.opened)
            Qt.callLater(() => { if (!editing && !pop.activeFocus) pop.close(); });
    }
    onBrowse: direction => {
        if (choiceField.picker && !pop.opened && (browseOnFocus || text.trim() !== "")) {
            pop.open();
        } else if (pop.opened) {
            results.moveFocus(direction);
        }
    }

    readonly property var filtered: {
        const query = text.trim().toLowerCase();
        if (!search) {
            if (!query || choices.some(row => row.id.toLowerCase() === query))
                return choices;
            return choices.filter(row => row.id.toLowerCase().includes(query));
        }
        const searchable = row => (row.title + " " + (row.detail ?? "") + " " + (row.search ?? row.meta ?? "")).toLowerCase();
        const normalize = value => value.toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "");
        const needle = normalize(query);
        const startsWord = value => normalize(value).split(/[\s,()/_]+/).some(word => word.startsWith(needle));
        const results = [[], [], []];
        for (const row of choices) {
            if (needle && !startsWord(searchable(row)))
                continue;
            const rank = row.id === preferred ? 0 : normalize(row.title).startsWith(needle) ? 1 : 2;
            results[rank].push(row);
        }
        const ordered = results[0].concat(results[1], results[2]);
        return resultLimit ? ordered.slice(0, resultLimit) : ordered;
    }

    Controls.Popup {
        id: pop
        parent: choiceField.anchorItem
        y: {
            const windowHeight = choiceField.Window.window?.height ?? 800;
            const top = parent.mapToItem(null, 0, 0).y;
            const below = parent.height + 8;
            if (top + below + height > windowHeight - Math.min(124, windowHeight * 0.155)
                    && top >= height + 8)
                return -height - 8;
            return below;
        }
        width: parent.width
        height: results.implicitHeight
        margins: 8
        padding: 0
        closePolicy: Controls.Popup.CloseOnEscape | Controls.Popup.CloseOnPressOutsideParent
        onOpened: results.focusRow(0)
        background: null
        contentItem: SetupChoices {
            id: results
            floating: true
            items: choiceField.filtered
            maxHeight: 296
            selected: choiceField.selected
            onChoose: value => choiceField.select(value)
        }
    }
}
