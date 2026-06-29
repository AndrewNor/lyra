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

    height: 52

    // ── Background ──────────────────────────────────────────────────────────
    Rectangle {
        anchors.fill: parent
        color: {
            var hc = Kirigami.Theme.highlightColor
            var tc = Kirigami.Theme.textColor
            if (root.isCurrentTrack && hc) {
                return Qt.rgba(hc.r, hc.g, hc.b, 0.18)
            }
            if (delegateHover.containsMouse && tc) {
                return Qt.rgba(tc.r, tc.g, tc.b, 0.05)
            }
            return "transparent"
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
        anchors.leftMargin: 6
        anchors.rightMargin: Kirigami.Units.largeSpacing
        spacing: 0

        // Album art thumbnail
        Item {
            width: 46
            height: 46

            Rectangle {
                anchors.fill: parent
                radius: 4
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
            }

            // Playing indicator overlay
            Rectangle {
                anchors.fill: parent
                radius: 4
                color: {
                    var hc = Kirigami.Theme.highlightColor
                    return (hc && root.isCurrentTrack)
                           ? Qt.rgba(hc.r, hc.g, hc.b, 0.7)
                           : "transparent"
                }
                visible: root.isCurrentTrack

                Kirigami.Icon {
                    anchors.centerIn: parent
                    source: "media-playback-start"
                    width: 20
                    height: 20
                    color: Kirigami.Theme.highlightedTextColor || "#ffffff"
                }
            }
        }

        // Title + Artist
        ColumnLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 10
            spacing: 3

            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.trackData && root.trackData.title && root.trackData.title.trim().length > 0)
                      ? root.trackData.title
                      : "(untitled)"
                font.bold: root.isCurrentTrack
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.95
                color: Kirigami.Theme.textColor || "#000000"
            }
            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.trackData && root.trackData.artist) ? root.trackData.artist : ""
                color: Kirigami.Theme.disabledTextColor || "#888888"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.83
                visible: text.length > 0
            }
        }

        // Duration
        Controls.Label {
            Layout.preferredWidth: 48
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
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
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
