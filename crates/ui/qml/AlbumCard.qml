// AlbumCard.qml — premium album tile (Apple Music / Spotify tier)

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property var albumData: null

    signal cardClicked()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.smallSpacing + 2
        spacing: Kirigami.Units.smallSpacing + 1

        // ── Cover square ────────────────────────────────────────────────
        Item {
            id: coverItem
            Layout.fillWidth: true
            Layout.preferredHeight: width

            // Multi-layer soft shadow
            Rectangle {
                anchors.centerIn: parent
                width: parent.width - 6
                height: parent.height - 6
                anchors.verticalCenterOffset: 10
                radius: 14
                color: "#000000"
                opacity: cardHover.containsMouse ? 0.18 : 0.14

                Behavior on opacity { NumberAnimation { duration: 180 } }
            }
            Rectangle {
                anchors.centerIn: parent
                width: parent.width - 2
                height: parent.height - 2
                anchors.verticalCenterOffset: 5
                radius: 13
                color: "#000000"
                opacity: cardHover.containsMouse ? 0.12 : 0.08

                Behavior on opacity { NumberAnimation { duration: 180 } }
            }

            Rectangle {
                id: coverFrame
                anchors.fill: parent
                radius: 12
                color: Qt.rgba(0, 0, 0, 0.06)
                clip: true

                // Hover scale applied to this item via transform
                transform: Scale {
                    origin.x: coverFrame.width / 2
                    origin.y: coverFrame.height / 2
                    xScale: cardHover.containsMouse ? 1.035 : 1.0
                    yScale: cardHover.containsMouse ? 1.035 : 1.0

                    Behavior on xScale { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                    Behavior on yScale { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
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

                // Fallback gradient + icon
                Rectangle {
                    anchors.fill: parent
                    visible: !coverImg.visible
                    color: Qt.rgba(0, 0, 0, 0.06)

                    Kirigami.Icon {
                        anchors.centerIn: parent
                        source: "media-album-cover"
                        width: 40
                        height: 40
                        color: "#b0b0b6"
                    }
                }

                // Hover shimmer overlay
                Rectangle {
                    anchors.fill: parent
                    radius: 12
                    color: Qt.rgba(0, 0, 0, cardHover.containsMouse ? 0.05 : 0.0)
                    z: 2

                    Behavior on color { ColorAnimation { duration: 150 } }
                }
            }
        }

        // ── Text block ──────────────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.albumData && root.albumData.title)
                      ? root.albumData.title
                      : "(untitled)"
                font.weight: Font.SemiBold
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.90
                color: "#1d1d1f"
            }

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.albumData && root.albumData.artist)
                      ? root.albumData.artist
                      : ""
                color: "#86868b"
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
                color: "#b0b0b6"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.74
            }
        }
    }

    MouseArea {
        id: cardHover
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.cardClicked()
        onDoubleClicked: root.cardClicked()
    }
}
