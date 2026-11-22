import QtQuick

Column {
    id: languages
    required property var installed
    required property string selected
    property int availableHeight: 508
    signal choose(string value)
    signal searchEdited()
    spacing: 12

    SetupField {
        id: search
        width: parent.width
        search: true
        name: qsTr("Search languages")
        placeholder: qsTr("Search")
        onEdited: languages.searchEdited()
        onAccepted: if (results.items.length) languages.choose(results.items[0].id)
    }
    SetupChoices {
        id: results
        width: parent.width
        maxHeight: Math.max(160, Math.min(452, languages.availableHeight - 56))
        showOverflowCue: true
        selected: languages.selected
        items: {
            const query = search.text.trim().toLowerCase();
            return languages.installed.filter(row => !query || (row.title + " " + row.meta + " " + row.id)
                .toLowerCase().split(/[\s,()/_]+/).some(word => word.startsWith(query)));
        }
        onChoose: value => languages.choose(value)
    }
}
