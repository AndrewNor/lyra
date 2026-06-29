// TrackDelegate.qml — premium track row (Apple Music / Spotify tier)

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property var trackData: null
    property int trackIndex: 0
    property bool isCurrentTrack: false
    // Playlists array passed from the parent view — [{id, name, track_count}]
    property var playlistsModel: []
    // The current playlist id when in playlist_detail view (-1 otherwise)
    property int currentPlaylistId: -1

    signal trackClicked(int idx)
    // Emitted when user picks "Add to playlist" from the context menu
    signal addToPlaylistRequested(int trackId, int playlistId)
    // Emitted when user picks "Remove from playlist"
    signal removeFromPlaylistRequested(int trackId, int playlistId)
    // Emitted when user picks "New playlist…" from the context menu
    signal newPlaylistRequested(int trackId)
    // Emitted when user saves tag edits — parent handles the library call
    signal saveTagsRequested(string path, string title, string artist, string album)

    height: 64

    // ── Background — hover + active tinting ───────────────────────────────
    Rectangle {
        anchors.fill: parent
        color: {
            var hc = Kirigami.Theme.highlightColor || "#3daee9"
            if (root.isCurrentTrack) {
                return Qt.rgba(hc.r, hc.g, hc.b, 0.14)
            }
            if (delegateHover.containsMouse) {
                return Qt.rgba(1, 1, 1, 0.05)
            }
            return "transparent"
        }

        Behavior on color { ColorAnimation { duration: 120 } }
    }

    // ── Left accent glow bar for now-playing ──────────────────────────────
    Rectangle {
        id: accentBar
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.topMargin: 10
        anchors.bottomMargin: 10
        width: 3
        radius: 2
        color: Kirigami.Theme.highlightColor || "#3daee9"
        opacity: root.isCurrentTrack ? 1.0 : 0.0

        // Soft glow behind bar
        Rectangle {
            anchors.centerIn: parent
            width: parent.width + 8
            height: parent.height + 4
            radius: 6
            color: Kirigami.Theme.highlightColor || "#3daee9"
            opacity: 0.28
            z: -1
        }

        Behavior on opacity { NumberAnimation { duration: 200 } }
    }

    // ── Bottom separator ──────────────────────────────────────────────────
    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 72
        height: 1
        color: Qt.rgba(1, 1, 1, 0.07)
    }

    // ── Content ───────────────────────────────────────────────────────────
    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing
        anchors.rightMargin: Kirigami.Units.largeSpacing
        spacing: 0

        // ── Album art thumbnail ───────────────────────────────────────────
        Item {
            width: 50
            height: 50

            // Multi-layer soft shadow
            Rectangle {
                anchors.centerIn: parent
                width: parent.width + 4
                height: parent.height + 4
                anchors.verticalCenterOffset: 5
                radius: 10
                color: "#000000"
                opacity: 0.40
            }
            Rectangle {
                anchors.centerIn: parent
                width: parent.width + 2
                height: parent.height + 2
                anchors.verticalCenterOffset: 2
                radius: 9
                color: "#000000"
                opacity: 0.20
            }

            Rectangle {
                id: artContainer
                anchors.fill: parent
                radius: 8
                color: Qt.rgba(1, 1, 1, 0.08)
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

                // Fallback icon
                Kirigami.Icon {
                    anchors.centerIn: parent
                    source: "audio-x-generic"
                    width: 22
                    height: 22
                    color: Qt.rgba(1, 1, 1, 0.30)
                    visible: !thumbImg.visible
                }

                // Now-playing overlay with animated equalizer bars
                Rectangle {
                    anchors.fill: parent
                    radius: 8
                    color: {
                        var hc = Kirigami.Theme.highlightColor || "#3daee9"
                        return root.isCurrentTrack
                               ? Qt.rgba(hc.r, hc.g, hc.b, 0.72)
                               : "transparent"
                    }
                    visible: root.isCurrentTrack

                    // Animated equalizer (3 bars) — centered, no anchor conflicts
                    Item {
                        anchors.centerIn: parent
                        width: 17   // 3 bars * 3px + 2 gaps * 4px
                        height: 28

                        Rectangle {
                            id: eqBar1
                            x: 0
                            width: 3
                            radius: 2
                            color: "white"
                            height: 16
                            anchors.bottom: parent.bottom

                            SequentialAnimation on height {
                                running: root.isCurrentTrack
                                loops: Animation.Infinite
                                NumberAnimation { to: 22; duration: 380; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 8;  duration: 320; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 18; duration: 410; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 12; duration: 290; easing.type: Easing.InOutSine }
                            }
                        }

                        Rectangle {
                            id: eqBar2
                            x: 7
                            width: 3
                            radius: 2
                            color: "white"
                            height: 20
                            anchors.bottom: parent.bottom

                            SequentialAnimation on height {
                                running: root.isCurrentTrack
                                loops: Animation.Infinite
                                NumberAnimation { to: 10; duration: 420; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 24; duration: 350; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 14; duration: 280; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 20; duration: 390; easing.type: Easing.InOutSine }
                            }
                        }

                        Rectangle {
                            id: eqBar3
                            x: 14
                            width: 3
                            radius: 2
                            color: "white"
                            height: 13
                            anchors.bottom: parent.bottom

                            SequentialAnimation on height {
                                running: root.isCurrentTrack
                                loops: Animation.Infinite
                                NumberAnimation { to: 20; duration: 310; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 8;  duration: 440; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 22; duration: 360; easing.type: Easing.InOutSine }
                                NumberAnimation { to: 15; duration: 330; easing.type: Easing.InOutSine }
                            }
                        }
                    }
                }
            }
        }

        // ── Title + Artist ─────────────────────────────────────────────────
        ColumnLayout {
            Layout.fillWidth: true
            Layout.leftMargin: Kirigami.Units.largeSpacing
            spacing: 3

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
                       : Qt.rgba(1, 1, 1, 0.92)

                Behavior on color { ColorAnimation { duration: 160 } }
            }
            Controls.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: (root.trackData && root.trackData.artist) ? root.trackData.artist : ""
                color: Qt.rgba(1, 1, 1, 0.45)
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                visible: text.length > 0
            }
        }

        // ── Duration — tabular mono ────────────────────────────────────────
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
            color: Qt.rgba(1, 1, 1, 0.38)
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
            font.features: { "tnum": 1 }
        }
    }

    // ── Click handler ─────────────────────────────────────────────────────
    MouseArea {
        id: delegateHover
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: function(mouse) {
            if (mouse.button === Qt.RightButton) {
                contextMenu.popup()
            } else {
                root.trackClicked(root.trackIndex)
            }
        }
        onDoubleClicked: function(mouse) {
            if (mouse.button === Qt.LeftButton)
                root.trackClicked(root.trackIndex)
        }
    }

    // ── Hover "+" quick add-to-playlist button ───────────────────────────
    Controls.ToolButton {
        id: addBtn
        anchors.right: parent.right
        anchors.rightMargin: 56  // leave room for duration label
        anchors.verticalCenter: parent.verticalCenter
        visible: delegateHover.containsMouse
        icon.name: "list-add"
        implicitWidth: 28
        implicitHeight: 28
        flat: true
        opacity: hovered ? 1.0 : 0.6
        Controls.ToolTip.visible: hovered
        Controls.ToolTip.text: "Add to playlist"
        Controls.ToolTip.delay: 300
        onClicked: quickAddMenu.popup()

        Behavior on opacity { NumberAnimation { duration: 100 } }
    }

    // ── Quick "Add to playlist" menu (opened by the + button) ─────────────
    Controls.Menu {
        id: quickAddMenu

        Controls.MenuItem {
            icon.name: "folder-add"
            text: "New playlist…"
            onTriggered: { if (root.trackData) root.newPlaylistRequested(root.trackData.id) }
        }

        Controls.MenuSeparator { visible: root.playlistsModel.length > 0 }

        Repeater {
            model: root.playlistsModel
            delegate: Controls.MenuItem {
                required property var modelData
                icon.name: "view-media-playlist"
                text: modelData ? (modelData.name || "Untitled") : ""
                onTriggered: {
                    if (root.trackData && modelData)
                        root.addToPlaylistRequested(root.trackData.id, modelData.id)
                }
            }
        }

        // Empty-state hint
        Controls.MenuItem {
            visible: root.playlistsModel.length === 0
            height: visible ? implicitHeight : 0
            enabled: false
            text: "No playlists yet — create one above"
        }
    }

    // ── Right-click context menu ──────────────────────────────────────────
    Controls.Menu {
        id: contextMenu

        Controls.MenuItem {
            icon.name: "media-playback-start"
            text: "Play"
            onTriggered: root.trackClicked(root.trackIndex)
        }

        Controls.MenuSeparator {}

        // Add to playlist — nested submenu (New playlist… + existing playlists)
        Controls.Menu {
            title: "Add to playlist"

            Controls.MenuItem {
                icon.name: "folder-add"
                text: "New playlist…"
                onTriggered: { if (root.trackData) root.newPlaylistRequested(root.trackData.id) }
            }

            Controls.MenuSeparator { visible: root.playlistsModel.length > 0 }

            Repeater {
                model: root.playlistsModel
                delegate: Controls.MenuItem {
                    required property var modelData
                    icon.name: "view-media-playlist"
                    text: modelData ? (modelData.name || "Untitled") : ""
                    onTriggered: {
                        if (root.trackData && modelData)
                            root.addToPlaylistRequested(root.trackData.id, modelData.id)
                    }
                }
            }

            Controls.MenuItem {
                visible: root.playlistsModel.length === 0
                height: visible ? implicitHeight : 0
                enabled: false
                text: "No playlists yet"
            }
        }

        // Remove from this playlist — only shown in playlist_detail view
        Controls.MenuItem {
            visible: root.currentPlaylistId >= 0
            height: visible ? implicitHeight : 0
            icon.name: "list-remove"
            text: "Remove from this playlist"
            onTriggered: {
                if (root.trackData && root.currentPlaylistId >= 0)
                    root.removeFromPlaylistRequested(root.trackData.id, root.currentPlaylistId)
            }
        }

        Controls.MenuSeparator {}

        Controls.MenuItem {
            icon.name: "document-edit"
            text: "Edit tags…"
            onTriggered: {
                if (root.trackData) {
                    editTitleField.text  = root.trackData.title  || ""
                    editArtistField.text = root.trackData.artist || ""
                    editAlbumField.text  = root.trackData.album  || ""
                    editTagsDialog.open()
                }
            }
        }
    }

    // ── Edit Tags dialog ──────────────────────────────────────────────────
    Controls.Dialog {
        id: editTagsDialog
        title: "Edit Tags"
        modal: true
        anchors.centerIn: Controls.Overlay.overlay
        standardButtons: Controls.Dialog.Save | Controls.Dialog.Cancel
        padding: 20
        spacing: 12

        contentItem: Column {
            spacing: 12
            width: 340

            Controls.Label {
                text: "Title"
                color: Qt.rgba(1, 1, 1, 0.60)
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
            }
            Controls.TextField {
                id: editTitleField
                width: parent.width
                placeholderText: "Title"
                background: Rectangle {
                    radius: 5
                    color: Qt.rgba(1, 1, 1, 0.07)
                    border.color: editTitleField.activeFocus
                                  ? (Kirigami.Theme.highlightColor || "#3daee9")
                                  : Qt.rgba(1, 1, 1, 0.14)
                    border.width: editTitleField.activeFocus ? 2 : 1
                }
                color: Qt.rgba(1, 1, 1, 0.92)
            }

            Controls.Label {
                text: "Artist"
                color: Qt.rgba(1, 1, 1, 0.60)
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
            }
            Controls.TextField {
                id: editArtistField
                width: parent.width
                placeholderText: "Artist"
                background: Rectangle {
                    radius: 5
                    color: Qt.rgba(1, 1, 1, 0.07)
                    border.color: editArtistField.activeFocus
                                  ? (Kirigami.Theme.highlightColor || "#3daee9")
                                  : Qt.rgba(1, 1, 1, 0.14)
                    border.width: editArtistField.activeFocus ? 2 : 1
                }
                color: Qt.rgba(1, 1, 1, 0.92)
            }

            Controls.Label {
                text: "Album"
                color: Qt.rgba(1, 1, 1, 0.60)
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
            }
            Controls.TextField {
                id: editAlbumField
                width: parent.width
                placeholderText: "Album"
                background: Rectangle {
                    radius: 5
                    color: Qt.rgba(1, 1, 1, 0.07)
                    border.color: editAlbumField.activeFocus
                                  ? (Kirigami.Theme.highlightColor || "#3daee9")
                                  : Qt.rgba(1, 1, 1, 0.14)
                    border.width: editAlbumField.activeFocus ? 2 : 1
                }
                color: Qt.rgba(1, 1, 1, 0.92)
            }
        }

        onAccepted: {
            if (root.trackData && root.trackData.path) {
                root.saveTagsRequested(
                    root.trackData.path,
                    editTitleField.text,
                    editArtistField.text,
                    editAlbumField.text
                )
            }
        }
    }
}
