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

    height: 60

    // ── Background ──────────────────────────────────────────────────────────
    Rectangle {
        anchors.fill: parent
        color: {
            var tc = Kirigami.Theme.textColor
            return (rowHover.containsMouse && tc)
                   ? Qt.rgba(tc.r, tc.g, tc.b, 0.05)
                   : "transparent"
        }

        Behavior on color { ColorAnimation { duration: 120 } }
    }

    // ── Bottom separator ────────────────────────────────────────────────────
    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Kirigami.Units.largeSpacing + 54
        height: 1
        color: Kirigami.Theme.separatorColor || "#e0e0e0"
        opacity: 0.35
    }

    // ── Content ─────────────────────────────────────────────────────────────
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing
        anchors.rightMargin: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing - 2

        // Genre icon circle
        Item {
            width: 44
            height: 44

            // Shadow
            Rectangle {
                anchors.centerIn: parent
                width: parent.width + 2
                height: parent.height + 2
                anchors.verticalCenterOffset: 2
                radius: (parent.width + 2) / 2
                color: {
                    var tc = Kirigami.Theme.textColor
                    return tc ? Qt.rgba(tc.r, tc.g, tc.b, 0.10) : "#00000010"
                }
            }

            Rectangle {
                anchors.fill: parent
                radius: parent.width / 2
                color: Kirigami.Theme.alternateBackgroundColor || "#f0f0f0"

                Kirigami.Icon {
                    anchors.centerIn: parent
                    source: "tag"
                    width: 20
                    height: 20
                    color: Kirigami.Theme.disabledTextColor || "#888888"
                }
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
                font.weight: Font.Medium
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.97
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
            width: 14
            height: 14
            color: Kirigami.Theme.disabledTextColor || "#888888"
            opacity: rowHover.containsMouse ? 0.9 : 0.45

            Behavior on opacity { NumberAnimation { duration: 120 } }
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
