import QtQuick

Column {
    id: languages
    required property var installed
    required property string selected
    property int availableHeight: 508
    signal choose(string value)
    spacing: 12

    SetupField {
        id: search
        width: parent.width
        search: true
        name: qsTr("Search languages")
        placeholder: qsTr("Search")
        onAccepted: if (results.items.length) languages.choose(results.items[0].id)
    }
    SetupChoices {
        id: results
        width: parent.width
        maxHeight: Math.max(160, Math.min(452, languages.availableHeight - 56))
        selected: languages.selected
        items: languages.installed.filter(row =>
            (row.title + " " + row.meta + " " + row.id).toLowerCase().includes(search.text.trim().toLowerCase()))
        onChoose: value => languages.choose(value)
    }
}
