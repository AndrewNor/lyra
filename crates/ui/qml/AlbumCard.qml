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
        anchors.margins: Kirigami.Units.smallSpacing
        spacing: Kirigami.Units.smallSpacing

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: width

            // Shadow layer underneath
            Rectangle {
                anchors.centerIn: parent
                width: parent.width - 4
                height: parent.height - 4
                anchors.verticalCenterOffset: 4
                radius: Kirigami.Units.gridUnit
                color: {
                    var tc = Kirigami.Theme.textColor
                    return tc ? Qt.rgba(tc.r, tc.g, tc.b, 0.18) : "#00000020"
                }
                // Blur is not available, so we use opacity trick
                opacity: cardHover.containsMouse ? 0.25 : 0.15

                Behavior on opacity { NumberAnimation { duration: 150 } }
            }

            Rectangle {
                anchors.fill: parent
                radius: Kirigami.Units.gridUnit
                color: Kirigami.Theme.alternateBackgroundColor || "#f0f0f0"
                clip: true

                // Hover wash on top
                Rectangle {
                    anchors.fill: parent
                    radius: Kirigami.Units.gridUnit
                    color: {
                        var tc = Kirigami.Theme.textColor
                        return (cardHover.containsMouse && tc)
                               ? Qt.rgba(tc.r, tc.g, tc.b, 0.08)
                               : "transparent"
                    }
                    z: 2

                    Behavior on color { ColorAnimation { duration: 120 } }
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

            // Lift animation: slight scale on hover
            transform: Scale {
                origin.x: parent.width / 2
                origin.y: parent.height / 2
                xScale: cardHover.containsMouse ? 1.02 : 1.0
                yScale: cardHover.containsMouse ? 1.02 : 1.0

                Behavior on xScale { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
                Behavior on yScale { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
            }
        }

        // ── Text block below the cover ────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.albumData && root.albumData.title)
                      ? root.albumData.title
                      : "(untitled)"
                font.weight: Font.Medium
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.90
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
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.75
                opacity: 0.7
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
