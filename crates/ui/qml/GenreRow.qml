// GenreRow.qml — a single row in the Genres list view.
// Shows genre name and track count.

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property var genreData: null

    signal rowClicked()

    height: 52

    // ── Background ──────────────────────────────────────────────────────────
    Rectangle {
        anchors.fill: parent
        color: {
            var tc = Kirigami.Theme.textColor
            return (rowHover.containsMouse && tc)
                   ? Qt.rgba(tc.r, tc.g, tc.b, 0.05)
                   : "transparent"
        }
    }

    // ── Bottom separator ────────────────────────────────────────────────────
    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: 1
        color: Kirigami.Theme.separatorColor || "#e0e0e0"
        opacity: 0.4
    }

    // ── Content ─────────────────────────────────────────────────────────────
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing
        anchors.rightMargin: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.smallSpacing

        // Genre icon circle
        Rectangle {
            width: 36
            height: 36
            radius: 18
            color: Kirigami.Theme.alternateBackgroundColor || "#f0f0f0"
            border.color: Kirigami.Theme.separatorColor || "#d0d0d0"
            border.width: 1

            Kirigami.Icon {
                anchors.centerIn: parent
                source: "tag"
                width: 20
                height: 20
                color: Kirigami.Theme.disabledTextColor || "#888888"
            }
        }

        // Name + track count
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.genreData && root.genreData.name)
                      ? root.genreData.name
                      : "(unknown)"
                font.bold: true
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.95
                color: Kirigami.Theme.textColor || "#000000"
            }

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: {
                    if (!root.genreData) return ""
                    var tc = root.genreData.track_count || 0
                    return tc + (tc === 1 ? " track" : " tracks")
                }
                color: Kirigami.Theme.disabledTextColor || "#888888"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
            }
        }

        // Disclosure chevron
        Kirigami.Icon {
            source: "arrow-right"
            width: 16
            height: 16
            color: Kirigami.Theme.disabledTextColor || "#888888"
        }
    }

    // ── Click / hover handler ───────────────────────────────────────────────
    MouseArea {
        id: rowHover
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.rowClicked()
    }
}
