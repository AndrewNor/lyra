// ArtistRow.qml — premium artist row (Apple Music / Spotify tier)

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property var artistData: null

    signal rowClicked()

    height: 68

    // ── Background hover ──────────────────────────────────────────────────
    Rectangle {
        anchors.fill: parent
        color: rowHover.containsMouse ? Qt.rgba(0, 0, 0, 0.05) : "transparent"

        Behavior on color { ColorAnimation { duration: 120 } }
    }

    // ── Bottom separator ──────────────────────────────────────────────────
    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Kirigami.Units.largeSpacing + 58
        height: 1
        color: "transparent"
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing
        anchors.rightMargin: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        // ── Avatar circle ──────────────────────────────────────────────────
        Item {
            width: 48
            height: 48

            // Shadow layers
            Rectangle {
                anchors.centerIn: parent
                width: parent.width + 4
                height: parent.height + 4
                anchors.verticalCenterOffset: 4
                radius: (parent.width + 4) / 2
                color: "#000000"
                opacity: 0.14
            }

            Rectangle {
                anchors.fill: parent
                radius: parent.width / 2
                color: Qt.rgba(0, 0, 0, 0.06)

                Kirigami.Icon {
                    anchors.centerIn: parent
                    source: "user-identity"
                    width: 24
                    height: 24
                    color: "#b0b0b6"
                }
            }
        }

        // ── Name + stats ───────────────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 3

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.artistData && root.artistData.name)
                      ? root.artistData.name
                      : "(unknown)"
                font.weight: Font.SemiBold
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.00
                color: "#1d1d1f"
            }

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: {
                    if (!root.artistData) return ""
                    var ac = root.artistData.album_count || 0
                    var tc = root.artistData.track_count || 0
                    return ac + (ac === 1 ? " album" : " albums")
                           + "  ·  "
                           + tc + (tc === 1 ? " track" : " tracks")
                }
                color: "#86868b"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
            }
        }

        // ── Disclosure chevron ─────────────────────────────────────────────
        Kirigami.Icon {
            source: "arrow-right"
            width: 14
            height: 14
            color: rowHover.containsMouse ? "#3a3a3c" : "#b0b0b6"

            Behavior on color { ColorAnimation { duration: 120 } }
        }
    }

    MouseArea {
        id: rowHover
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.rowClicked()
    }
}
