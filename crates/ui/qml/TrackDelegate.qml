// TrackDelegate.qml — a single row in the main track list.
// Shows album art thumbnail, title, artist, and duration (m:ss).

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property var trackData: null
    property int trackIndex: 0
    property bool isCurrentTrack: false

    signal trackClicked(int idx)

    height: 60

    // ── Background ──────────────────────────────────────────────────────────
    Rectangle {
        anchors.fill: parent
        color: {
            var hc = Kirigami.Theme.highlightColor
            var tc = Kirigami.Theme.textColor
            if (root.isCurrentTrack && hc) {
                return Qt.rgba(hc.r, hc.g, hc.b, 0.12)
            }
            if (delegateHover.containsMouse && tc) {
                return Qt.rgba(tc.r, tc.g, tc.b, 0.05)
            }
            return "transparent"
        }

        Behavior on color { ColorAnimation { duration: 120 } }
    }

    // ── Left accent bar for now-playing ────────────────────────────────────
    Rectangle {
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: 3
        radius: 1.5
        color: Kirigami.Theme.highlightColor || "#3daee9"
        opacity: root.isCurrentTrack ? 1.0 : 0.0

        Behavior on opacity { NumberAnimation { duration: 150 } }
    }

    // ── Bottom separator ────────────────────────────────────────────────────
    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Kirigami.Units.largeSpacing
        height: 1
        color: Kirigami.Theme.separatorColor || "#e0e0e0"
        opacity: 0.35
    }

    // ── Content ─────────────────────────────────────────────────────────────
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing
        anchors.rightMargin: Kirigami.Units.largeSpacing
        spacing: 0

        // Album art thumbnail
        Item {
            width: 48
            height: 48

            // Shadow simulation: a slightly larger, darker rectangle underneath
            Rectangle {
                anchors.centerIn: parent
                width: parent.width + 2
                height: parent.height + 2
                radius: Kirigami.Units.smallSpacing + 1
                color: {
                    var tc = Kirigami.Theme.textColor
                    return tc ? Qt.rgba(tc.r, tc.g, tc.b, 0.12) : "#00000012"
                }
                visible: thumbImg.status === Image.Ready || true
            }

            Rectangle {
                anchors.fill: parent
                radius: Kirigami.Units.smallSpacing
                color: Kirigami.Theme.alternateBackgroundColor || "#f0f0f0"
                clip: true

                Image {
                    id: thumbImg
                    anchors.fill: parent
                    source: (root.trackData && root.trackData.cover_thumb && root.trackData.cover_thumb.length > 0)
                            ? "file://" + root.trackData.cover_thumb
                            : ""
                    fillMode: Image.PreserveAspectCrop
                    clip: true
                    visible: status === Image.Ready
                    asynchronous: true
                }

                Kirigami.Icon {
                    anchors.centerIn: parent
                    source: "audio-x-generic"
                    width: 22
                    height: 22
                    color: Kirigami.Theme.disabledTextColor || "#888888"
                    visible: !thumbImg.visible
                }

                // Playing indicator overlay
                Rectangle {
                    anchors.fill: parent
                    radius: Kirigami.Units.smallSpacing
                    color: {
                        var hc = Kirigami.Theme.highlightColor
                        return (hc && root.isCurrentTrack)
                               ? Qt.rgba(hc.r, hc.g, hc.b, 0.65)
                               : "transparent"
                    }
                    visible: root.isCurrentTrack

                    Kirigami.Icon {
                        anchors.centerIn: parent
                        source: "media-playback-start"
                        width: 20
                        height: 20
                        color: "white"
                    }
                }
            }
        }

        // Title + Artist
        ColumnLayout {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            spacing: 2

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.trackData && root.trackData.title && root.trackData.title.trim().length > 0)
                      ? root.trackData.title
                      : "(untitled)"
                font.bold: root.isCurrentTrack
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.97
                color: root.isCurrentTrack
                       ? (Kirigami.Theme.highlightColor || "#3daee9")
                       : (Kirigami.Theme.textColor || "#000000")

                Behavior on color { ColorAnimation { duration: 150 } }
            }
            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.trackData && root.trackData.artist) ? root.trackData.artist : ""
                color: Kirigami.Theme.disabledTextColor || "#888888"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                visible: text.length > 0
            }
        }

        // Duration — fixed-width tabular style
        Controls.Label {
            Layout.preferredWidth: 44
            horizontalAlignment: Text.AlignRight
            text: {
                var ms = (root.trackData && root.trackData.durationMs) ? root.trackData.durationMs : 0
                if (!ms || ms <= 0) return ""
                var totalSec = Math.floor(ms / 1000)
                var minutes = Math.floor(totalSec / 60)
                var seconds = totalSec % 60
                return minutes + ":" + (seconds < 10 ? "0" : "") + seconds
            }
            color: Kirigami.Theme.disabledTextColor || "#888888"
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
            font.features: { "tnum": 1 }
        }
    }

    // ── Click handler ───────────────────────────────────────────────────────
    MouseArea {
        id: delegateHover
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.trackClicked(root.trackIndex)
        onDoubleClicked: root.trackClicked(root.trackIndex)
    }
}
