import QtQuick
import QtQuick.Controls.Basic as Controls

SetupField {
    id: picker
    property var choices: []
    property string selected: ""
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

    onPick: pop.opened ? pop.close() : pop.open()
    onEdited: {
        results.list.currentIndex = -1;
        if (search && text !== "" && !pop.opened)
            pop.open();
    }
    onAccepted: if (pop.opened && filtered.length) select(filtered[Math.max(0, results.list.currentIndex)].id)
    onEditingChanged: if (search && editing && !pop.opened) pop.open()
    onBrowse: direction => {
        if (!pop.opened)
            pop.open();
        if (filtered.length) {
            const index = Math.max(0, Math.min(filtered.length - 1, results.list.currentIndex + direction));
            results.list.currentIndex = index;
            results.list.positionViewAtIndex(index, ListView.Contain);
        }
    }

    readonly property var filtered: {
        const query = search ? text.trim().toLowerCase() : "";
        return choices.filter(row => (row.title + " " + (row.meta ?? "") + " " + row.id).toLowerCase().includes(query));
    }

    Controls.Popup {
        id: pop
        parent: picker.anchorItem
        y: parent.height + 5
        width: parent.width
        height: results.implicitHeight
        margins: 8
        padding: 0
        closePolicy: Controls.Popup.CloseOnEscape | Controls.Popup.CloseOnPressOutsideParent
        background: null
        contentItem: SetupChoices {
            id: results
            items: picker.filtered
            maxHeight: 296
            selected: picker.selected
            onChoose: value => picker.select(value)
        }
    }
}
