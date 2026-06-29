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
        height: 48

        background: Rectangle {
            color: Kirigami.Theme.backgroundColor || "#ffffff"
            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: Kirigami.Theme.separatorColor || "#d0d0d0"
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
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.15
                color: Kirigami.Theme.textColor || "#000000"
            }

            Rectangle {
                width: 1
                height: 20
                color: Kirigami.Theme.separatorColor || "#d0d0d0"
            }

            Kirigami.SearchField {
                id: searchField
                Layout.preferredWidth: 260
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
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.9
            }

            // Only show status_text when it carries transient/meaningful info
            // (e.g. while scanning or right after a scan), not when it merely
            // duplicates the track count already shown above.
            Controls.Label {
                text: library.status_text || ""
                color: Kirigami.Theme.disabledTextColor || "#888888"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
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
                height: 20
                color: Kirigami.Theme.separatorColor || "#d0d0d0"
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
        height: 72

        background: Rectangle {
            color: Kirigami.Theme.backgroundColor || "#ffffff"
            Rectangle {
                anchors.top: parent.top
                width: parent.width
                height: 1
                color: Kirigami.Theme.separatorColor || "#d0d0d0"
            }
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Kirigami.Units.largeSpacing
            anchors.rightMargin: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.largeSpacing

            // Current track info (left side)
            RowLayout {
                Layout.preferredWidth: 240
                spacing: Kirigami.Units.smallSpacing

                Rectangle {
                    width: 48
                    height: 48
                    radius: 4
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

                ColumnLayout {
                    spacing: 2
                    Layout.fillWidth: true

                    Controls.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: player.current_title || "(nothing playing)"
                        font.bold: (player.current_title || "").length > 0
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.95
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
                spacing: 4

                Controls.ToolButton {
                    icon.name: "media-playlist-shuffle"
                    opacity: 0.5
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

                Controls.RoundButton {
                    width: 44
                    height: 44
                    icon.name: (player.state_text === "Playing")
                               ? "media-playback-pause"
                               : "media-playback-start"
                    icon.width: 20
                    icon.height: 20
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
                    opacity: 0.5
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.text: "Repeat — coming soon"
                    Controls.ToolTip.delay: 400
                }
            }

            Item { Layout.fillWidth: true }

            // Progress + volume (right side)
            ColumnLayout {
                Layout.preferredWidth: 240
                spacing: 4

                // Live position / seek bar
                RowLayout {
                    spacing: 6

                    Controls.Label {
                        id: posLabel
                        text: root.fmtTime(player.position_secs)
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.75
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
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.75
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                    }
                }

                // Volume control (visual only)
                RowLayout {
                    spacing: 6

                    Kirigami.Icon {
                        source: "audio-volume-medium"
                        width: 16
                        height: 16
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
                Layout.preferredWidth: 200
                Layout.fillHeight: true
                color: Kirigami.Theme.alternateBackgroundColor || "#f5f5f5"

                Rectangle {
                    anchors.right: parent.right
                    width: 1
                    height: parent.height
                    color: Kirigami.Theme.separatorColor || "#d0d0d0"
                }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.topMargin: Kirigami.Units.largeSpacing
                    spacing: 0

                    // Library section
                    Controls.Label {
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.bottomMargin: 4
                        text: "Library"
                        font.bold: true
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.8
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 0.5
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

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.topMargin: Kirigami.Units.smallSpacing
                        Layout.bottomMargin: Kirigami.Units.smallSpacing
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        height: 1
                        color: Kirigami.Theme.separatorColor || "#d0d0d0"
                    }

                    // Playlists section
                    Controls.Label {
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.bottomMargin: 4
                        text: "Playlists"
                        font.bold: true
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.8
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 0.5
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

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.topMargin: Kirigami.Units.smallSpacing
                        Layout.bottomMargin: Kirigami.Units.smallSpacing
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.rightMargin: Kirigami.Units.largeSpacing
                        height: 1
                        color: Kirigami.Theme.separatorColor || "#d0d0d0"
                    }

                    // Sources · soon
                    Controls.Label {
                        Layout.leftMargin: Kirigami.Units.largeSpacing
                        Layout.bottomMargin: 4
                        text: "Sources · soon"
                        font.bold: true
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.8
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                        opacity: 0.6
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 0.5
                    }

                    SidebarItem {
                        iconName: "podcast"
                        label: "Podcasts"
                        enabled: false
                        opacity: 0.45
                    }
                    SidebarItem {
                        iconName: "network-wireless"
                        label: "Radio"
                        enabled: false
                        opacity: 0.45
                    }
                    SidebarItem {
                        iconName: "internet-web-browser"
                        label: "YouTube"
                        enabled: false
                        opacity: 0.45
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
                                    height: 32

                                    Rectangle {
                                        anchors.fill: parent
                                        color: Kirigami.Theme.backgroundColor || "#ffffff"
                                    }

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: 6
                                        anchors.rightMargin: Kirigami.Units.largeSpacing
                                        spacing: 0

                                        Item { width: 52 }

                                        Controls.Label {
                                            Layout.fillWidth: true
                                            Layout.leftMargin: 10
                                            text: "Title / Artist"
                                            font.bold: true
                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                                            color: Kirigami.Theme.disabledTextColor || "#888888"
                                            font.capitalization: Font.AllUppercase
                                            font.letterSpacing: 0.4
                                        }

                                        Controls.Label {
                                            Layout.preferredWidth: 50
                                            horizontalAlignment: Text.AlignRight
                                            text: "Time"
                                            font.bold: true
                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                                            color: Kirigami.Theme.disabledTextColor || "#888888"
                                            font.capitalization: Font.AllUppercase
                                            font.letterSpacing: 0.4
                                        }
                                    }

                                    Rectangle {
                                        anchors.bottom: parent.bottom
                                        width: parent.width
                                        height: 1
                                        color: Kirigami.Theme.separatorColor || "#d0d0d0"
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
                            height: 32

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
                                    font.bold: true
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                                    color: Kirigami.Theme.disabledTextColor || "#888888"
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 0.4
                                }

                                Controls.Label {
                                    text: "Albums · Tracks"
                                    font.bold: true
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                                    color: Kirigami.Theme.disabledTextColor || "#888888"
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 0.4
                                }
                            }

                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: 1
                                color: Kirigami.Theme.separatorColor || "#d0d0d0"
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
                            height: 32

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
                                    font.bold: true
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                                    color: Kirigami.Theme.disabledTextColor || "#888888"
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 0.4
                                }

                                Controls.Label {
                                    text: "Tracks"
                                    font.bold: true
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                                    color: Kirigami.Theme.disabledTextColor || "#888888"
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 0.4
                                }
                            }

                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: 1
                                color: Kirigami.Theme.separatorColor || "#d0d0d0"
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
                Layout.preferredWidth: root.nowPlayingVisible ? 280 : 0
                Layout.fillHeight: true
                clip: true
                visible: root.nowPlayingVisible
                color: Kirigami.Theme.alternateBackgroundColor || "#f5f5f5"

                Behavior on Layout.preferredWidth {
                    NumberAnimation { duration: 180; easing.type: Easing.InOutQuad }
                }

                Rectangle {
                    anchors.left: parent.left
                    width: 1
                    height: parent.height
                    color: Kirigami.Theme.separatorColor || "#d0d0d0"
                }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.topMargin: Kirigami.Units.largeSpacing
                    anchors.leftMargin: Kirigami.Units.largeSpacing
                    anchors.rightMargin: Kirigami.Units.largeSpacing
                    spacing: Kirigami.Units.smallSpacing

                    // Large cover
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: width
                        radius: 8
                        color: Kirigami.Theme.backgroundColor || "#ffffff"
                        border.color: Kirigami.Theme.separatorColor || "#d0d0d0"
                        border.width: 1
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

                    // Track title
                    Controls.Label {
                        Layout.fillWidth: true
                        text: player.current_title || "Nothing Playing"
                        font.bold: true
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.05
                        elide: Text.ElideRight
                        color: Kirigami.Theme.textColor || "#000000"
                    }

                    // Artist
                    Controls.Label {
                        Layout.fillWidth: true
                        text: player.current_artist || ""
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.9
                        elide: Text.ElideRight
                        visible: text.length > 0
                    }

                    // State
                    Controls.Label {
                        text: player.state_text || "Stopped"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.8
                        color: (player.state_text === "Playing")
                               ? (Kirigami.Theme.positiveTextColor || "#27ae60")
                               : (Kirigami.Theme.disabledTextColor || "#888888")
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        height: 1
                        color: Kirigami.Theme.separatorColor || "#d0d0d0"
                    }

                    Controls.Label {
                        text: "Up Next"
                        font.bold: true
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                        color: Kirigami.Theme.disabledTextColor || "#888888"
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 0.5
                    }

                    // Queue list
                    ListView {
                        id: queueList
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        model: root.queueTracks
                        clip: true

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        delegate: Item {
                            width: queueList.width
                            height: 48

                            RowLayout {
                                anchors.fill: parent
                                spacing: 8

                                Rectangle {
                                    width: 36
                                    height: 36
                                    radius: 3
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
                                width: parent.width
                                height: 1
                                color: Kirigami.Theme.separatorColor || "#d0d0d0"
                                opacity: 0.5
                            }
                        }

                        Controls.Label {
                            anchors.centerIn: parent
                            visible: queueList.count === 0
                            text: "Queue empty"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                            color: Kirigami.Theme.disabledTextColor || "#888888"
                        }
                    }
                }
            }
        }
    }
}
