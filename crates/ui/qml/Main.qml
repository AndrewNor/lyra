import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import ai.drivee.lyra

// ── Layout-B: sidebar · content list · collapsible now-playing panel · transport ──
Kirigami.ApplicationWindow {
    id: root
    title: "Lyra"
    width: 1100
    height: 700
    minimumWidth: 700
    minimumHeight: 480

    // ── QObject instances ────────────────────────────────────────────────────
    Library { id: library }
    Player  { id: player  }

    Component.onCompleted: {
        library.loadAll()
        player.initMpris()
    }

    // ── View state machine ───────────────────────────────────────────────────
    // Possible values: "songs" | "albums" | "album_detail"
    //                  "artists" | "artist_detail"
    //                  "genres" | "genre_detail"
    property string view: "songs"

    // Name of the album/artist currently being drilled into (for the header)
    property string detailName: ""

    // ── Position polling timer (250 ms while Playing) ────────────────────────
    Timer {
        id: positionTimer
        interval: 250
        running: player.state_text === "Playing"
        repeat: true
        onTriggered: player.refreshPosition()
    }

    // ── m:ss formatter (guards NaN / negative) ───────────────────────────────
    function fmtTime(s) {
        var n = s || 0
        if (isNaN(n) || n < 0) n = 0
        var totalSec = Math.floor(n)
        var minutes  = Math.floor(totalSec / 60)
        var seconds  = totalSec % 60
        return minutes + ":" + (seconds < 10 ? "0" : "") + seconds
    }

    // ── State: right panel visibility ────────────────────────────────────────
    property bool nowPlayingVisible: true

    // ── Parsed track list — re-evaluated whenever results_json changes ───────
    property var tracks: {
        var s = library.results_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    // ── Parsed albums list ───────────────────────────────────────────────────
    property var albums: {
        var s = library.albums_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    // ── Parsed artists list ──────────────────────────────────────────────────
    property var artists: {
        var s = library.artists_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    // ── Parsed genres list ───────────────────────────────────────────────────
    property var genres: {
        var s = library.genres_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    // ── Parsed queue — re-evaluated whenever queue_json changes ─────────────
    property var queueTracks: {
        var s = player.queue_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    // ── Top header ───────────────────────────────────────────────────────────
    header: Controls.ToolBar {
        id: topBar
        height: 52

        background: Rectangle {
            color: Kirigami.Theme.backgroundColor || "#ffffff"
            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: Kirigami.Theme.separatorColor || "#d0d0d0"
                opacity: 0.7
            }
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Kirigami.Units.largeSpacing
            anchors.rightMargin: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.largeSpacing

            Controls.Label {
                text: "Lyra"
                font.bold: true
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.18
                color: Kirigami.Theme.highlightColor || Kirigami.Theme.textColor || "#000000"
            }

            Rectangle {
                width: 1
                height: 18
                color: Kirigami.Theme.separatorColor || "#d0d0d0"
                opacity: 0.6
            }

            Kirigami.SearchField {
                id: searchField
                Layout.preferredWidth: 280
                placeholderText: "Search tracks…"
                onAccepted: {
                    if (text.trim().length === 0) {
                        library.loadAll()
                        root.view = "songs"
                    } else {
                        library.search(text)
                        root.view = "songs"
                    }
                }
            }

            Item { Layout.fillWidth: true }

            Controls.Label {
                text: (library.track_count || 0) + " tracks"
                color: Kirigami.Theme.disabledTextColor || "#888888"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
                opacity: 0.8
            }

            // Only show status_text when it carries transient/meaningful info
            // (e.g. while scanning or right after a scan), not when it merely
            // duplicates the track count already shown above.
            Controls.Label {
                text: library.status_text || ""
                color: Kirigami.Theme.disabledTextColor || "#888888"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                elide: Text.ElideRight
                Layout.maximumWidth: 200
                visible: {
                    var st = library.status_text || ""
                    if (st.length === 0) return false
                    // Hide if it looks like "N tracks" (same as the count label)
                    var countStr = (library.track_count || 0) + " tracks"
                    if (st === countStr) return false
                    // Hide generic idle states
                    if (st === "Ready" || st === "Idle") return false
                    return true
                }
            }

            Rectangle {
                width: 1
                height: 18
                color: Kirigami.Theme.separatorColor || "#d0d0d0"
                opacity: 0.6
            }

            Controls.ToolButton {
                text: (library.scanning || false) ? "Scanning…" : "Scan"
                icon.name: "view-refresh"
                enabled: !(library.scanning || false)
                onClicked: library.scan()
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.text: "Scan ~/Music for new tracks"
                Controls.ToolTip.delay: 600
            }

            Controls.ToolButton {
                icon.name: root.nowPlayingVisible ? "sidebar-collapse-right" : "sidebar-expand-right"
                onClicked: root.nowPlayingVisible = !root.nowPlayingVisible
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.text: root.nowPlayingVisible ? "Hide Now Playing panel" : "Show Now Playing panel"
                Controls.ToolTip.delay: 600
            }
        }
    }

    // ── Bottom transport bar ─────────────────────────────────────────────────
    footer: Controls.ToolBar {
        id: transportBar
        height: 76

        background: Rectangle {
            color: Kirigami.Theme.backgroundColor || "#ffffff"
            Rectangle {
                anchors.top: parent.top
                width: parent.width
                height: 1
                color: Kirigami.Theme.separatorColor || "#d0d0d0"
                opacity: 0.7
            }
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Kirigami.Units.largeSpacing
            anchors.rightMargin: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.largeSpacing

            // Current track info (left side)
            RowLayout {
                Layout.preferredWidth: 260
                spacing: Kirigami.Units.smallSpacing + 2

                // Current cover art thumbnail
                Item {
                    width: 52
                    height: 52

                    // Shadow
                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width + 2
                        height: parent.height + 2
                        anchors.verticalCenterOffset: 3
                        radius: Kirigami.Units.smallSpacing + 1
                        color: {
                            var tc = Kirigami.Theme.textColor
                            return tc ? Qt.rgba(tc.r, tc.g, tc.b, 0.15) : "#00000015"
                        }
                    }

                    Rectangle {
                        anchors.fill: parent
                        radius: Kirigami.Units.smallSpacing
                        color: Kirigami.Theme.alternateBackgroundColor || "#f5f5f5"
                        clip: true

                        Image {
                            id: transportCover
                            anchors.fill: parent
                            source: player.current_cover_thumb
                                    ? "file://" + player.current_cover_thumb
                                    : ""
                            fillMode: Image.PreserveAspectCrop
                            visible: status === Image.Ready
                        }

                        Kirigami.Icon {
                            anchors.centerIn: parent
                            source: "media-optical-audio"
                            width: 24
                            height: 24
                            color: Kirigami.Theme.disabledTextColor || "#888888"
                            visible: !transportCover.visible
                        }
                    }
                }

                ColumnLayout {
                    spacing: 2
                    Layout.fillWidth: true

                    Controls.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: player.current_title || "(nothing playing)"
                        font.bold: (player.current_title || "").length > 0
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.95
                        color: (player.current_title || "").length > 0
                               ? (Kirigami.Theme.textColor || "#000000")
                               : (Kirigami.Theme.disabledTextColor || "#888888")
                    }
                    Controls.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: player.current_artist || ""
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
                        visible: text.length > 0
                    }
                }
            }

            Item { Layout.fillWidth: true }

            // Playback controls (center)
            RowLayout {
                spacing: Kirigami.Units.smallSpacing

                Controls.ToolButton {
                    icon.name: "media-playlist-shuffle"
                    opacity: 0.4
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.text: "Shuffle — coming soon"
                    Controls.ToolTip.delay: 400
                }

                Controls.ToolButton {
                    icon.name: "media-skip-backward"
                    onClicked: player.prev()
                    enabled: (player.state_text || "Stopped") !== "Stopped"
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.text: "Previous"
                    Controls.ToolTip.delay: 400
                }

                // Emphasized play/pause button
                Controls.RoundButton {
                    width: 48
                    height: 48
                    icon.name: (player.state_text === "Playing")
                               ? "media-playback-pause"
                               : "media-playback-start"
                    icon.width: 22
                    icon.height: 22
                    palette.button: Kirigami.Theme.highlightColor || "#3daee9"
                    palette.buttonText: Kirigami.Theme.highlightedTextColor || "#ffffff"
                    onClicked: {
                        if (player.state_text === "Playing")
                            player.pause()
                        else
                            player.resume()
                    }
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.text: (player.state_text === "Playing") ? "Pause" : "Play"
                    Controls.ToolTip.delay: 400
                }

                Controls.ToolButton {
                    icon.name: "media-skip-forward"
                    onClicked: player.next()
                    enabled: (player.state_text || "Stopped") !== "Stopped"
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.text: "Next"
                    Controls.ToolTip.delay: 400
                }

                Controls.ToolButton {
                    icon.name: "media-playlist-repeat"
                    opacity: 0.4
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.text: "Repeat — coming soon"
                    Controls.ToolTip.delay: 400
                }
            }

            Item { Layout.fillWidth: true }

            // Progress + volume (right side)
            ColumnLayout {
                Layout.preferredWidth: 260
                spacing: 6

                // Live position / seek bar
                RowLayout {
                    spacing: Kirigami.Units.smallSpacing

                    Controls.Label {
                        id: posLabel
                        text: root.fmtTime(player.position_secs)
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.78
                        font.features: { "tnum": 1 }
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                    }

                    Controls.Slider {
                        id: seekBar
                        Layout.fillWidth: true
                        from: 0
                        to: 1
                        // Only update the slider position from the engine when
                        // the user is not dragging — avoids a binding loop.
                        value: (!seekBar.pressed && player.duration_secs > 0)
                               ? (player.position_secs / player.duration_secs)
                               : seekBar.value
                        enabled: player.duration_secs > 0

                        // Seek when the user releases the handle.
                        onPressedChanged: {
                            if (!pressed && player.duration_secs > 0) {
                                player.seek(seekBar.value)
                            }
                        }
                    }

                    Controls.Label {
                        id: durLabel
                        text: (player.duration_secs > 0)
                              ? root.fmtTime(player.duration_secs)
                              : "—"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.78
                        font.features: { "tnum": 1 }
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                    }
                }

                // Volume control (visual only)
                RowLayout {
                    spacing: Kirigami.Units.smallSpacing

                    Kirigami.Icon {
                        source: "audio-volume-medium"
                        width: 14
                        height: 14
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                    }

                    Controls.Slider {
                        id: volumeSlider
                        Layout.fillWidth: true
                        from: 0
                        to: 100
                        value: 80
                        Controls.ToolTip.visible: hovered
                        Controls.ToolTip.text: "Volume — visual only (engine API coming soon)"
                        Controls.ToolTip.delay: 400
                    }
                }
            }
        }
    }

    // ── Main layout: sidebar + content + now-playing panel ──────────────────
    // Use pageStack.initialPage with titleVisible:false and a custom layout.
    pageStack.initialPage: Kirigami.Page {
        id: mainPage
        // Suppress the page's auto-generated toolbar (Kirigami 6 API)
        globalToolBarStyle: Kirigami.ApplicationHeaderStyle.None
        leftPadding: 0
        rightPadding: 0
        topPadding: 0
        bottomPadding: 0

        RowLayout {
            anchors.fill: parent
            spacing: 0

            // ── Left sidebar ─────────────────────────────────────────────────
            Rectangle {
                id: sidebar
                Layout.preferredWidth: 210
                Layout.fillHeight: true
                color: Kirigami.Theme.alternateBackgroundColor || "#f5f5f5"

                Rectangle {
                    anchors.right: parent.right
                    width: 1
                    height: parent.height
                    color: Kirigami.Theme.separatorColor || "#d0d0d0"
                    opacity: 0.7
                }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.topMargin: Kirigami.Units.largeSpacing
                    anchors.bottomMargin: Kirigami.Units.smallSpacing
                    spacing: 0

                    // Library section header
                    Controls.Label {
                        Layout.leftMargin: Kirigami.Units.largeSpacing + 2
                        Layout.bottomMargin: 2
                        Layout.topMargin: 2
                        text: "Library"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 1.2
                        font.weight: Font.Medium
                        opacity: 0.8
                    }

                    SidebarItem {
                        iconName: "audio-x-generic"
                        label: "Songs"
                        active: root.view === "songs"
                        onActivated: {
                            root.view = "songs"
                            library.loadAll()
                        }
                    }
                    SidebarItem {
                        iconName: "media-album-cover"
                        label: "Albums"
                        active: root.view === "albums" || root.view === "album_detail"
                        onActivated: {
                            root.view = "albums"
                            library.loadAlbums()
                        }
                    }
                    SidebarItem {
                        iconName: "user-identity"
                        label: "Artists"
                        active: root.view === "artists" || root.view === "artist_detail"
                        onActivated: {
                            root.view = "artists"
                            library.loadArtists()
                        }
                    }
                    SidebarItem {
                        iconName: "tag"
                        label: "Genres"
                        active: root.view === "genres" || root.view === "genre_detail"
                        onActivated: {
                            root.view = "genres"
                            library.loadGenres()
                        }
                    }
                    SidebarItem {
                        iconName: "view-calendar-recent-events"
                        label: "Recently Added"
                        enabled: false
                    }

                    // Separator
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.topMargin: Kirigami.Units.smallSpacing + 2
                        Layout.bottomMargin: Kirigami.Units.smallSpacing + 2
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        height: 1
                        color: Kirigami.Theme.separatorColor || "#d0d0d0"
                        opacity: 0.5
                    }

                    // Playlists section header
                    Controls.Label {
                        Layout.leftMargin: Kirigami.Units.largeSpacing + 2
                        Layout.bottomMargin: 2
                        Layout.topMargin: 2
                        text: "Playlists"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 1.2
                        font.weight: Font.Medium
                        opacity: 0.8
                    }

                    SidebarItem {
                        iconName: "view-media-playlist"
                        label: "Favourites"
                        enabled: false
                    }
                    SidebarItem {
                        iconName: "view-media-playlist"
                        label: "Mix 1"
                        enabled: false
                    }

                    // Separator
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.topMargin: Kirigami.Units.smallSpacing + 2
                        Layout.bottomMargin: Kirigami.Units.smallSpacing + 2
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        height: 1
                        color: Kirigami.Theme.separatorColor || "#d0d0d0"
                        opacity: 0.5
                    }

                    // Sources — coming soon
                    Controls.Label {
                        Layout.leftMargin: Kirigami.Units.largeSpacing + 2
                        Layout.bottomMargin: 2
                        Layout.topMargin: 2
                        text: "Sources"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 1.2
                        font.weight: Font.Medium
                        opacity: 0.45
                    }

                    SidebarItem {
                        iconName: "podcast"
                        label: "Podcasts"
                        enabled: false
                        opacity: 0.38
                    }
                    SidebarItem {
                        iconName: "network-wireless"
                        label: "Radio"
                        enabled: false
                        opacity: 0.38
                    }
                    SidebarItem {
                        iconName: "internet-web-browser"
                        label: "YouTube"
                        enabled: false
                        opacity: 0.38
                    }

                    Item { Layout.fillHeight: true }
                }
            }

            // ── Main content area — switches on root.view ─────────────────────
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                // ── Songs view (+ detail track lists) ──────────────────────
                // Visible for "songs", "album_detail", "artist_detail", "genre_detail"
                Item {
                    anchors.fill: parent
                    visible: root.view === "songs"
                             || root.view === "album_detail"
                             || root.view === "artist_detail"
                             || root.view === "genre_detail"

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 0

                        // Detail header — shown only in drill-down views
                        Item {
                            Layout.fillWidth: true
                            height: (root.view === "album_detail"
                                     || root.view === "artist_detail"
                                     || root.view === "genre_detail")
                                    ? 44 : 0
                            visible: root.view === "album_detail"
                                     || root.view === "artist_detail"
                                     || root.view === "genre_detail"
                            clip: true

                            Rectangle {
                                anchors.fill: parent
                                color: Kirigami.Theme.backgroundColor || "#ffffff"
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: Kirigami.Units.largeSpacing
                                anchors.rightMargin: Kirigami.Units.largeSpacing
                                spacing: Kirigami.Units.smallSpacing

                                Controls.ToolButton {
                                    text: root.view === "album_detail"
                                          ? "‹ Albums"
                                          : root.view === "genre_detail"
                                            ? "‹ Genres"
                                            : "‹ Artists"
                                    onClicked: {
                                        if (root.view === "album_detail")
                                            root.view = "albums"
                                        else if (root.view === "genre_detail")
                                            root.view = "genres"
                                        else
                                            root.view = "artists"
                                    }
                                }

                                Controls.Label {
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                    text: root.detailName
                                    font.bold: true
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.05
                                    color: Kirigami.Theme.textColor || "#000000"
                                }
                            }

                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: 1
                                color: Kirigami.Theme.separatorColor || "#d0d0d0"
                            }
                        }

                        // Track list
                        Item {
                            Layout.fillWidth: true
                            Layout.fillHeight: true

                            ListView {
                                id: trackList
                                anchors.fill: parent
                                model: root.tracks
                                clip: true
                                reuseItems: true

                                Controls.ScrollBar.vertical: Controls.ScrollBar {
                                    policy: Controls.ScrollBar.AsNeeded
                                }

                                header: Item {
                                    width: trackList.width
                                    height: 34

                                    Rectangle {
                                        anchors.fill: parent
                                        color: Kirigami.Theme.backgroundColor || "#ffffff"
                                    }

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: Kirigami.Units.largeSpacing
                                        anchors.rightMargin: Kirigami.Units.largeSpacing
                                        spacing: 0

                                        Item { width: 56 }

                                        Controls.Label {
                                            Layout.fillWidth: true
                                            Layout.leftMargin: Kirigami.Units.largeSpacing
                                            text: "Title / Artist"
                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                                            color: Kirigami.Theme.disabledTextColor || "#888888"
                                            font.capitalization: Font.AllUppercase
                                            font.letterSpacing: 1.0
                                            font.weight: Font.Medium
                                        }

                                        Controls.Label {
                                            Layout.preferredWidth: 50
                                            horizontalAlignment: Text.AlignRight
                                            text: "Time"
                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                                            color: Kirigami.Theme.disabledTextColor || "#888888"
                                            font.capitalization: Font.AllUppercase
                                            font.letterSpacing: 1.0
                                            font.weight: Font.Medium
                                        }
                                    }

                                    Rectangle {
                                        anchors.bottom: parent.bottom
                                        width: parent.width
                                        height: 1
                                        color: Kirigami.Theme.separatorColor || "#d0d0d0"
                                        opacity: 0.5
                                    }
                                }

                                headerPositioning: ListView.OverlayHeader

                                delegate: TrackDelegate {
                                    width: trackList.width
                                    trackData: modelData
                                    trackIndex: index
                                    isCurrentTrack: {
                                        var title = player.current_title || ""
                                        return title.length > 0
                                               && modelData
                                               && modelData.title === title
                                    }
                                    onTrackClicked: function(idx) {
                                        player.playFromList(library.results_json, idx)
                                    }
                                }

                                Kirigami.PlaceholderMessage {
                                    anchors.centerIn: parent
                                    visible: trackList.count === 0 && !(library.scanning || false)
                                    text: "No tracks found"
                                    explanation: "Click Scan to index your music library, or try a different search."
                                    icon.name: "audio-x-generic"
                                }

                                Kirigami.PlaceholderMessage {
                                    anchors.centerIn: parent
                                    visible: library.scanning || false
                                    text: "Scanning library…"
                                    explanation: library.status_text || ""
                                    icon.name: "view-refresh"
                                }
                            }
                        }
                    }
                }

                // ── Albums grid view ────────────────────────────────────────
                Item {
                    anchors.fill: parent
                    visible: root.view === "albums"

                    GridView {
                        id: albumGrid
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.largeSpacing
                        model: root.albums
                        clip: true

                        // Responsive cell size: aim for ~160 px, at least 140
                        property int cellTargetWidth: 160
                        property int cols: Math.max(2, Math.floor(width / cellTargetWidth))
                        cellWidth: Math.floor(width / cols)
                        // Height = cover square + text block (~80 px) + padding
                        cellHeight: cellWidth + 80

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        delegate: Item {
                            width: albumGrid.cellWidth
                            height: albumGrid.cellHeight

                            AlbumCard {
                                anchors.fill: parent
                                anchors.margins: 4
                                albumData: modelData
                                onCardClicked: {
                                    if (!modelData) return
                                    root.detailName = modelData.title || ""
                                    library.loadAlbumTracks(modelData.id)
                                    root.view = "album_detail"
                                }
                            }
                        }

                        Kirigami.PlaceholderMessage {
                            anchors.centerIn: parent
                            visible: albumGrid.count === 0 && !(library.scanning || false)
                            text: "No albums found"
                            explanation: "Click Scan to index your music library."
                            icon.name: "media-album-cover"
                        }
                    }
                }

                // ── Artists list view ───────────────────────────────────────
                Item {
                    anchors.fill: parent
                    visible: root.view === "artists"

                    ListView {
                        id: artistList
                        anchors.fill: parent
                        model: root.artists
                        clip: true
                        reuseItems: true

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        header: Item {
                            width: artistList.width
                            height: 34

                            Rectangle {
                                anchors.fill: parent
                                color: Kirigami.Theme.backgroundColor || "#ffffff"
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: Kirigami.Units.largeSpacing
                                anchors.rightMargin: Kirigami.Units.largeSpacing
                                spacing: 0

                                Controls.Label {
                                    Layout.fillWidth: true
                                    text: "Artist"
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                                    color: Kirigami.Theme.disabledTextColor || "#888888"
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 1.0
                                    font.weight: Font.Medium
                                }

                                Controls.Label {
                                    text: "Albums · Tracks"
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                                    color: Kirigami.Theme.disabledTextColor || "#888888"
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 1.0
                                    font.weight: Font.Medium
                                }
                            }

                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: 1
                                color: Kirigami.Theme.separatorColor || "#d0d0d0"
                                opacity: 0.5
                            }
                        }

                        headerPositioning: ListView.OverlayHeader

                        delegate: ArtistRow {
                            width: artistList.width
                            artistData: modelData
                            onRowClicked: {
                                if (!modelData) return
                                root.detailName = modelData.name || ""
                                library.loadArtistTracks(modelData.id)
                                root.view = "artist_detail"
                            }
                        }

                        Kirigami.PlaceholderMessage {
                            anchors.centerIn: parent
                            visible: artistList.count === 0 && !(library.scanning || false)
                            text: "No artists found"
                            explanation: "Click Scan to index your music library."
                            icon.name: "user-identity"
                        }
                    }
                }

                // ── Genres list view ────────────────────────────────────────
                Item {
                    anchors.fill: parent
                    visible: root.view === "genres"

                    ListView {
                        id: genreList
                        anchors.fill: parent
                        model: root.genres
                        clip: true
                        reuseItems: true

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        header: Item {
                            width: genreList.width
                            height: 34

                            Rectangle {
                                anchors.fill: parent
                                color: Kirigami.Theme.backgroundColor || "#ffffff"
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: Kirigami.Units.largeSpacing
                                anchors.rightMargin: Kirigami.Units.largeSpacing
                                spacing: 0

                                Controls.Label {
                                    Layout.fillWidth: true
                                    text: "Genre"
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                                    color: Kirigami.Theme.disabledTextColor || "#888888"
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 1.0
                                    font.weight: Font.Medium
                                }

                                Controls.Label {
                                    text: "Tracks"
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                                    color: Kirigami.Theme.disabledTextColor || "#888888"
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 1.0
                                    font.weight: Font.Medium
                                }
                            }

                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: 1
                                color: Kirigami.Theme.separatorColor || "#d0d0d0"
                                opacity: 0.5
                            }
                        }

                        headerPositioning: ListView.OverlayHeader

                        delegate: GenreRow {
                            width: genreList.width
                            genreData: modelData
                            onRowClicked: {
                                if (!modelData) return
                                root.detailName = modelData.name || ""
                                library.loadGenreTracks(modelData.name || "")
                                root.view = "genre_detail"
                            }
                        }

                        Kirigami.PlaceholderMessage {
                            anchors.centerIn: parent
                            visible: genreList.count === 0 && !(library.scanning || false)
                            text: "No genres found"
                            explanation: "Click Scan to index your music library."
                            icon.name: "tag"
                        }
                    }
                }
            }

            // ── Right Now-Playing + queue panel ──────────────────────────────
            Rectangle {
                id: nowPlayingPanel
                Layout.preferredWidth: root.nowPlayingVisible ? 290 : 0
                Layout.fillHeight: true
                clip: true
                visible: root.nowPlayingVisible
                color: Kirigami.Theme.alternateBackgroundColor || "#f5f5f5"

                Behavior on Layout.preferredWidth {
                    NumberAnimation { duration: 180; easing.type: Easing.InOutQuad }
                }

                // Left border separator
                Rectangle {
                    anchors.left: parent.left
                    width: 1
                    height: parent.height
                    color: Kirigami.Theme.separatorColor || "#d0d0d0"
                    opacity: 0.7
                }

                // Subtle ambient gradient tinted by highlight at the top
                Rectangle {
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    height: parent.height * 0.45
                    opacity: 0.06
                    gradient: Gradient {
                        GradientStop {
                            position: 0.0
                            color: Kirigami.Theme.highlightColor || "#3daee9"
                        }
                        GradientStop {
                            position: 1.0
                            color: "transparent"
                        }
                    }
                }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.topMargin: Kirigami.Units.largeSpacing
                    anchors.leftMargin: Kirigami.Units.largeSpacing - 2
                    anchors.rightMargin: Kirigami.Units.largeSpacing - 2
                    anchors.bottomMargin: Kirigami.Units.smallSpacing
                    spacing: Kirigami.Units.smallSpacing

                    // ── Large cover with shadow ──────────────────────────────
                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: width

                        // Shadow
                        Rectangle {
                            anchors.centerIn: parent
                            width: parent.width - 8
                            height: parent.height - 8
                            anchors.verticalCenterOffset: 8
                            radius: Kirigami.Units.gridUnit + 2
                            color: {
                                var tc = Kirigami.Theme.textColor
                                return tc ? Qt.rgba(tc.r, tc.g, tc.b, 0.20) : "#00000020"
                            }
                        }

                        Rectangle {
                            anchors.fill: parent
                            radius: Kirigami.Units.gridUnit
                            color: Kirigami.Theme.backgroundColor || "#ffffff"
                            clip: true

                            Image {
                                id: npCover
                                anchors.fill: parent
                                source: player.current_cover_thumb
                                        ? "file://" + player.current_cover_thumb
                                        : ""
                                fillMode: Image.PreserveAspectCrop
                                visible: status === Image.Ready
                            }

                            Kirigami.Icon {
                                anchors.centerIn: parent
                                source: "media-optical-audio"
                                width: 64
                                height: 64
                                color: Kirigami.Theme.disabledTextColor || "#888888"
                                visible: !npCover.visible
                            }
                        }
                    }

                    // Spacing between cover and text
                    Item { height: Kirigami.Units.smallSpacing }

                    // Track title — bigger, bolder
                    Controls.Label {
                        Layout.fillWidth: true
                        text: player.current_title || "Nothing Playing"
                        font.bold: true
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.12
                        elide: Text.ElideRight
                        color: (player.current_title || "").length > 0
                               ? (Kirigami.Theme.textColor || "#000000")
                               : (Kirigami.Theme.disabledTextColor || "#888888")
                    }

                    // Artist
                    Controls.Label {
                        Layout.fillWidth: true
                        text: player.current_artist || ""
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.92
                        elide: Text.ElideRight
                        visible: text.length > 0
                    }

                    // State badge
                    Controls.Label {
                        text: player.state_text || "Stopped"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.76
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 0.8
                        color: (player.state_text === "Playing")
                               ? (Kirigami.Theme.positiveTextColor || "#27ae60")
                               : (Kirigami.Theme.disabledTextColor || "#888888")
                        opacity: 0.85
                    }

                    // Thin separator
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        Layout.bottomMargin: 2
                        height: 1
                        color: Kirigami.Theme.separatorColor || "#d0d0d0"
                        opacity: 0.5
                    }

                    // ── Tab toggle: Up Next / Lyrics ─────────────────────────
                    RowLayout {
                        id: tabRow
                        Layout.fillWidth: true
                        spacing: 0

                        property string activeTab: "queue"

                        Controls.ToolButton {
                            id: tabQueueBtn
                            text: "Up Next"
                            font.bold: tabRow.activeTab === "queue"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.83
                            flat: true
                            checkable: false
                            Layout.fillWidth: true
                            onClicked: tabRow.activeTab = "queue"
                            opacity: tabRow.activeTab === "queue" ? 1.0 : 0.50
                        }

                        Rectangle {
                            width: 1
                            height: 14
                            color: Kirigami.Theme.separatorColor || "#d0d0d0"
                            opacity: 0.6
                        }

                        Controls.ToolButton {
                            id: tabLyricsBtn
                            text: "Lyrics"
                            font.bold: tabRow.activeTab === "lyrics"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.83
                            flat: true
                            checkable: false
                            Layout.fillWidth: true
                            onClicked: tabRow.activeTab = "lyrics"
                            opacity: tabRow.activeTab === "lyrics" ? 1.0 : 0.50
                        }
                    }

                    // Active tab underline accent
                    Rectangle {
                        Layout.fillWidth: true
                        height: 2
                        radius: 1
                        color: Kirigami.Theme.highlightColor || "#3daee9"
                        // Inset to approximate the active button width
                        Layout.leftMargin: tabRow.activeTab === "queue" ? 0 : parent.width * 0.5
                        Layout.rightMargin: tabRow.activeTab === "queue" ? parent.width * 0.5 : 0
                        opacity: 0.85

                        Behavior on Layout.leftMargin { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
                        Behavior on Layout.rightMargin { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
                    }

                    // ── Up Next queue (shown when tab = "queue") ─────────────
                    ListView {
                        id: queueList
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        model: root.queueTracks
                        clip: true
                        visible: tabRow.activeTab === "queue"

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        delegate: Item {
                            width: queueList.width
                            height: 52

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 2
                                anchors.rightMargin: 4
                                spacing: Kirigami.Units.smallSpacing + 2

                                // Mini cover
                                Rectangle {
                                    width: 38
                                    height: 38
                                    radius: Kirigami.Units.smallSpacing - 1
                                    color: Kirigami.Theme.backgroundColor || "#ffffff"
                                    clip: true

                                    Image {
                                        id: qCoverImg
                                        anchors.fill: parent
                                        source: (modelData && modelData.cover_thumb && modelData.cover_thumb.length > 0)
                                                ? "file://" + modelData.cover_thumb
                                                : ""
                                        fillMode: Image.PreserveAspectCrop
                                        visible: status === Image.Ready
                                        asynchronous: true
                                    }

                                    Kirigami.Icon {
                                        anchors.centerIn: parent
                                        source: "audio-x-generic"
                                        width: 18
                                        height: 18
                                        color: Kirigami.Theme.disabledTextColor || "#888888"
                                        visible: !qCoverImg.visible
                                    }
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2

                                    Controls.Label {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: (modelData && modelData.title) ? modelData.title : "(untitled)"
                                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.88
                                        color: Kirigami.Theme.textColor || "#000000"
                                    }
                                    Controls.Label {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: (modelData && modelData.artist) ? modelData.artist : ""
                                        color: Kirigami.Theme.disabledTextColor || "#888888"
                                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.78
                                        visible: text.length > 0
                                    }
                                }
                            }

                            Rectangle {
                                anchors.bottom: parent.bottom
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.leftMargin: 46
                                height: 1
                                color: Kirigami.Theme.separatorColor || "#d0d0d0"
                                opacity: 0.35
                            }
                        }

                        Controls.Label {
                            anchors.centerIn: parent
                            visible: queueList.count === 0
                            text: "Queue empty"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                            color: Kirigami.Theme.disabledTextColor || "#888888"
                            opacity: 0.7
                        }
                    }

                    // ── Lyrics panel (shown when tab = "lyrics") ─────────────
                    Item {
                        id: lyricsPanel
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: tabRow.activeTab === "lyrics"
                        clip: true

                        // Parse lyrics JSON reactively.
                        property var lyricsData: {
                            var s = player.lyrics_json
                            if (!s || s.length === 0) return { synced: false, lines: [] }
                            try { return JSON.parse(s) } catch(e) { return { synced: false, lines: [] } }
                        }

                        property bool hasLines: lyricsData && lyricsData.lines && lyricsData.lines.length > 0

                        // Active line index for synced lyrics: last line with t <= position_secs.
                        property int activeLineIndex: {
                            if (!lyricsData || !lyricsData.synced || !lyricsData.lines) return -1
                            var lines = lyricsData.lines
                            var pos = player.position_secs
                            var best = -1
                            for (var i = 0; i < lines.length; i++) {
                                var t = lines[i].t
                                if (t !== null && t !== undefined && t <= pos) best = i
                            }
                            return best
                        }

                        // Scroll synced list to keep the active line visible.
                        onActiveLineIndexChanged: {
                            if (activeLineIndex >= 0 && lyricsData && lyricsData.synced) {
                                syncedLyricsList.positionViewAtIndex(activeLineIndex, ListView.Center)
                            }
                        }

                        // ── Empty state ───────────────────────────────────────
                        Controls.Label {
                            anchors.centerIn: parent
                            visible: !lyricsPanel.hasLines
                            text: "No lyrics"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.88
                            color: Kirigami.Theme.disabledTextColor || "#888888"
                            opacity: 0.7
                        }

                        // ── Synced lyrics (scrolling ListView) ────────────────
                        ListView {
                            id: syncedLyricsList
                            anchors.fill: parent
                            visible: lyricsPanel.hasLines && lyricsPanel.lyricsData && lyricsPanel.lyricsData.synced
                            model: (lyricsPanel.lyricsData && lyricsPanel.lyricsData.synced)
                                   ? lyricsPanel.lyricsData.lines
                                   : []
                            clip: true

                            Controls.ScrollBar.vertical: Controls.ScrollBar {
                                policy: Controls.ScrollBar.AsNeeded
                            }

                            delegate: Item {
                                width: syncedLyricsList.width
                                height: lyricLineLabel.implicitHeight + 16

                                property bool isActive: index === lyricsPanel.activeLineIndex

                                Controls.Label {
                                    id: lyricLineLabel
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    anchors.leftMargin: Kirigami.Units.smallSpacing
                                    anchors.rightMargin: Kirigami.Units.smallSpacing
                                    text: (modelData && modelData.text) ? modelData.text : ""
                                    wrapMode: Text.WordWrap
                                    font.pointSize: parent.isActive
                                                    ? Kirigami.Theme.defaultFont.pointSize * 0.98
                                                    : Kirigami.Theme.defaultFont.pointSize * 0.86
                                    font.bold: parent.isActive
                                    color: parent.isActive
                                           ? (Kirigami.Theme.highlightColor || "#3daee9")
                                           : (Kirigami.Theme.textColor || "#000000")
                                    opacity: parent.isActive ? 1.0 : 0.50

                                    Behavior on opacity { NumberAnimation { duration: 120 } }
                                    Behavior on font.pointSize { NumberAnimation { duration: 120 } }
                                }
                            }
                        }

                        // ── Unsynced lyrics (plain scrollable text) ───────────
                        Flickable {
                            id: unsyncedFlickable
                            anchors.fill: parent
                            visible: lyricsPanel.hasLines && lyricsPanel.lyricsData && !lyricsPanel.lyricsData.synced
                            contentWidth: width
                            contentHeight: unsyncedText.implicitHeight + Kirigami.Units.largeSpacing
                            clip: true

                            Controls.ScrollBar.vertical: Controls.ScrollBar {
                                policy: Controls.ScrollBar.AsNeeded
                            }

                            Controls.Label {
                                id: unsyncedText
                                width: unsyncedFlickable.width
                                text: {
                                    if (!lyricsPanel.lyricsData || !lyricsPanel.lyricsData.lines) return ""
                                    var lines = lyricsPanel.lyricsData.lines
                                    var parts = []
                                    for (var i = 0; i < lines.length; i++) {
                                        if (lines[i] && lines[i].text) parts.push(lines[i].text)
                                    }
                                    return parts.join("\n")
                                }
                                wrapMode: Text.WordWrap
                                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.88
                                color: Kirigami.Theme.textColor || "#000000"
                                topPadding: Kirigami.Units.smallSpacing
                                leftPadding: Kirigami.Units.smallSpacing
                                rightPadding: Kirigami.Units.smallSpacing
                            }
                        }
                    }
                }
            }
        }
    }
}
