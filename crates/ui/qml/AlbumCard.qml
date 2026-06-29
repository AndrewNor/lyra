// AlbumCard.qml — a single album tile in the Albums grid view.
// Shows a square cover thumbnail, album title, artist, and track count.

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property var albumData: null

    signal cardClicked()

    // ── Cover square ────────────────────────────────────────────────────────
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 6
        spacing: 6

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: width
            radius: 6
            color: Kirigami.Theme.alternateBackgroundColor || "#f0f0f0"
            clip: true
            border.color: Kirigami.Theme.separatorColor || "#d0d0d0"
            border.width: 1

            // Hover shadow overlay
            Rectangle {
                anchors.fill: parent
                radius: 6
                color: {
                    var tc = Kirigami.Theme.textColor
                    return (cardHover.containsMouse && tc)
                           ? Qt.rgba(tc.r, tc.g, tc.b, 0.08)
                           : "transparent"
                }
                z: 2
            }

            Image {
                id: coverImg
                anchors.fill: parent
                source: (root.albumData && root.albumData.cover_thumb
                         && root.albumData.cover_thumb.length > 0)
                        ? "file://" + root.albumData.cover_thumb
                        : ""
                fillMode: Image.PreserveAspectCrop
                clip: true
                visible: status === Image.Ready
                asynchronous: true
            }

            Kirigami.Icon {
                anchors.centerIn: parent
                source: "media-album-cover"
                width: 40
                height: 40
                color: Kirigami.Theme.disabledTextColor || "#888888"
                visible: !coverImg.visible
            }
        }

        // ── Text block below the cover ────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.albumData && root.albumData.title)
                      ? root.albumData.title
                      : "(untitled)"
                font.bold: true
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.88
                color: Kirigami.Theme.textColor || "#000000"
            }

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.albumData && root.albumData.artist)
                      ? root.albumData.artist
                      : ""
                color: Kirigami.Theme.disabledTextColor || "#888888"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.80
                visible: text.length > 0
            }

            Controls.Label {
                Layout.fillWidth: true
                text: {
                    var n = (root.albumData && root.albumData.track_count)
                            ? root.albumData.track_count : 0
                    return n + (n === 1 ? " track" : " tracks")
                }
                color: Kirigami.Theme.disabledTextColor || "#888888"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.76
            }
        }
    }

    // ── Click / hover handler ───────────────────────────────────────────────
    MouseArea {
        id: cardHover
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.cardClicked()
        onDoubleClicked: root.cardClicked()
    }
}
