import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import ai.drivee.lyra

Kirigami.ApplicationWindow {
    id: root
    title: "Lyra"
    width: 800
    height: 600

    // ── QObject instances ────────────────────────────────────────────────────
    Library { id: library }
    Player  { id: player  }

    Component.onCompleted: library.loadAll()

    // ── Parsed track list — re-evaluated whenever results_json changes ───────
    // cxx-qt exposes properties in snake_case; use library.results_json (not camelCase).
    // Guard against undefined (during construction) or empty/invalid JSON.
    property var tracks: {
        var s = library.results_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    // ── Footer transport bar ─────────────────────────────────────────────────
    footer: Controls.ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Kirigami.Units.smallSpacing
            anchors.rightMargin: Kirigami.Units.smallSpacing

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: {
                    var t = player.current_title || ""
                    var a = player.current_artist || ""
                    if (t.length === 0 && a.length === 0) return "(nothing playing)"
                    if (t.length === 0) return a
                    if (a.length === 0) return t
                    return t + " — " + a
                }
            }

            Controls.Label {
                text: player.state_text || "Stopped"
                color: Kirigami.Theme.disabledTextColor
            }

            Controls.Button {
                text: "Pause"
                onClicked: player.pause()
            }
            Controls.Button {
                text: "Resume"
                onClicked: player.resume()
            }
            Controls.Button {
                text: "Stop"
                onClicked: player.stop()
            }
        }
    }

    // ── Main page ────────────────────────────────────────────────────────────
    pageStack.initialPage: Kirigami.ScrollablePage {
        id: mainPage
        title: "Library"

        // Scan action shown in the page toolbar
        actions: [
            Kirigami.Action {
                text: (library.scanning || false) ? "Scanning…" : "Scan"
                icon.name: "view-refresh"
                enabled: !(library.scanning || false)
                onTriggered: library.scan()
            }
        ]

        // ScrollablePage takes ONE flickable child (the ListView)
        ListView {
            id: trackList
            model: root.tracks

            // Inline header: search field + status line
            header: ColumnLayout {
                width: trackList.width
                spacing: 0

                Kirigami.SearchField {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.smallSpacing
                    placeholderText: "Search tracks…"
                    onAccepted: {
                        if (text.length === 0) library.loadAll()
                        else library.search(text)
                    }
                }

                Controls.Label {
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.smallSpacing
                    Layout.rightMargin: Kirigami.Units.smallSpacing
                    Layout.bottomMargin: Kirigami.Units.smallSpacing
                    color: Kirigami.Theme.disabledTextColor
                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.9
                    text: (library.status_text || "") + "   ·   " + (library.track_count || 0) + " tracks"
                }
            }

            headerPositioning: ListView.OverlayHeader

            delegate: Controls.ItemDelegate {
                width: trackList.width
                contentItem: ColumnLayout {
                    spacing: 2
                    Controls.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: (modelData.title && modelData.title.length > 0)
                              ? modelData.title
                              : "(untitled)"
                    }
                    Controls.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        color: Kirigami.Theme.disabledTextColor
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
                        text: modelData.artist || ""
                    }
                }
                onClicked: player.play(
                    modelData.path || "",
                    modelData.title || "",
                    modelData.artist || ""
                )
            }

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                visible: trackList.count === 0 && !(library.scanning || false)
                text: "No tracks found"
                explanation: "Click Scan to index your music library"
            }
        }
    }
}
