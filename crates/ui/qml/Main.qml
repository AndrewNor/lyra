import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import QtQuick.Effects
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

    // ── Design tokens ──────────────────────────────────────────────────────────
    readonly property color bgBase:      "#0c0c12"
    readonly property color bgSidebar:   "#0f0f18"
    readonly property color bgContent:   "#0c0c12"
    readonly property color bgPanel:     "#0f0f18"
    readonly property color bgHeader:    "#0d0d16"
    readonly property color sepColor:    Qt.rgba(1, 1, 1, 0.08)
    readonly property color textPrimary: Qt.rgba(1, 1, 1, 0.92)
    readonly property color textDim:     Qt.rgba(1, 1, 1, 0.42)
    readonly property color textFaint:   Qt.rgba(1, 1, 1, 0.22)
    readonly property color accentColor: Kirigami.Theme.highlightColor || "#3daee9"

    // ── QObject instances ───────────────────────────────────────────────────
    Library { id: library }
    Player  { id: player  }

    Component.onCompleted: {
        library.loadAll()
        player.initMpris()
    }

    // ── View state machine ──────────────────────────────────────────────────
    property string view: "songs"
    property string detailName: ""

    // ── Position polling timer ──────────────────────────────────────────────
    Timer {
        id: positionTimer
        interval: 250
        running: player.state_text === "Playing"
        repeat: true
        onTriggered: player.refreshPosition()
    }

    // ── m:ss formatter ─────────────────────────────────────────────────────
    function fmtTime(s) {
        var n = s || 0
        if (isNaN(n) || n < 0) n = 0
        var totalSec = Math.floor(n)
        var minutes  = Math.floor(totalSec / 60)
        var seconds  = totalSec % 60
        return minutes + ":" + (seconds < 10 ? "0" : "") + seconds
    }

    property bool nowPlayingVisible: true

    // ── Parsed lists ────────────────────────────────────────────────────────
    property var tracks: {
        var s = library.results_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    property var albums: {
        var s = library.albums_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    property var artists: {
        var s = library.artists_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    property var genres: {
        var s = library.genres_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    property var queueTracks: {
        var s = player.queue_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    // ── Top header ──────────────────────────────────────────────────────────
    header: Controls.ToolBar {
        id: topBar
        height: 54

        background: Rectangle {
            color: root.bgHeader

            // Bottom border line
            Rectangle {
                anchors.bottom: parent.bottom
                width: parent.width
                height: 1
                color: root.sepColor
            }
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 18
            anchors.rightMargin: 14
            spacing: 14

            // App name wordmark
            Controls.Label {
                text: "Lyra"
                font.bold: true
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.22
                color: root.accentColor
                letterSpacing: -0.5
            }

            Rectangle {
                width: 1
                height: 18
                color: root.sepColor
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
                color: root.textDim
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.83
            }

            Controls.Label {
                text: library.status_text || ""
                color: root.textDim
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.80
                elide: Text.ElideRight
                Layout.maximumWidth: 200
                visible: {
                    var st = library.status_text || ""
                    if (st.length === 0) return false
                    var countStr = (library.track_count || 0) + " tracks"
                    if (st === countStr) return false
                    if (st === "Ready" || st === "Idle") return false
                    return true
                }
            }

            Rectangle {
                width: 1
                height: 18
                color: root.sepColor
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

    // ── Bottom transport bar ────────────────────────────────────────────────
    footer: Controls.ToolBar {
        id: transportBar
        height: 82

        background: Rectangle {
            color: root.bgHeader

            // Top border
            Rectangle {
                anchors.top: parent.top
                width: parent.width
                height: 1
                color: root.sepColor
            }

            // Subtle top gradient gleam
            Rectangle {
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                height: 40
                opacity: 0.05
                gradient: Gradient {
                    GradientStop { position: 0.0; color: root.accentColor }
                    GradientStop { position: 1.0; color: "transparent" }
                }
            }
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 18
            anchors.rightMargin: 18
            spacing: 16

            // ── Left: current track info ─────────────────────────────────────
            RowLayout {
                Layout.preferredWidth: 270
                spacing: 12

                // Current cover thumbnail with shadow
                Item {
                    width: 54
                    height: 54

                    // Shadow layers
                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width + 4
                        height: parent.height + 4
                        anchors.verticalCenterOffset: 6
                        radius: 11
                        color: "#000000"
                        opacity: 0.50
                    }
                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width + 2
                        height: parent.height + 2
                        anchors.verticalCenterOffset: 3
                        radius: 10
                        color: "#000000"
                        opacity: 0.25
                    }

                    Rectangle {
                        anchors.fill: parent
                        radius: 9
                        color: Qt.rgba(1, 1, 1, 0.08)
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

                        // Fallback
                        Kirigami.Icon {
                            anchors.centerIn: parent
                            source: "media-optical-audio"
                            width: 26
                            height: 26
                            color: Qt.rgba(1, 1, 1, 0.25)
                            visible: !transportCover.visible
                        }
                    }
                }

                ColumnLayout {
                    spacing: 3
                    Layout.fillWidth: true

                    Controls.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: player.current_title || "(nothing playing)"
                        font.bold: (player.current_title || "").length > 0
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.95
                        color: (player.current_title || "").length > 0
                               ? root.textPrimary
                               : root.textDim
                    }
                    Controls.Label {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: player.current_artist || ""
                        color: root.textDim
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.83
                        visible: text.length > 0
                    }
                }
            }

            Item { Layout.fillWidth: true }

            // ── Center: playback controls ────────────────────────────────────
            RowLayout {
                spacing: 6

                Controls.ToolButton {
                    icon.name: "media-playlist-shuffle"
                    opacity: 0.30
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

                // ── Play/Pause — accent glow button ───────────────────────────
                Item {
                    width: 52
                    height: 52

                    // Outer glow halo
                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width + 14
                        height: parent.height + 14
                        radius: (parent.width + 14) / 2
                        color: root.accentColor
                        opacity: playBtnArea.containsMouse ? 0.22 : 0.10
                        z: -1

                        Behavior on opacity { NumberAnimation { duration: 150 } }
                    }

                    // Button circle with gradient
                    Rectangle {
                        id: playBtnCircle
                        anchors.fill: parent
                        radius: parent.width / 2
                        gradient: Gradient {
                            GradientStop {
                                position: 0.0
                                color: Qt.lighter(root.accentColor, 1.15)
                            }
                            GradientStop {
                                position: 1.0
                                color: root.accentColor
                            }
                        }
                        scale: playBtnArea.pressed ? 0.93 : 1.0

                        Behavior on scale { NumberAnimation { duration: 80; easing.type: Easing.OutCubic } }

                        Kirigami.Icon {
                            anchors.centerIn: parent
                            source: (player.state_text === "Playing")
                                    ? "media-playback-pause"
                                    : "media-playback-start"
                            width: 22
                            height: 22
                            color: "white"
                        }
                    }

                    MouseArea {
                        id: playBtnArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (player.state_text === "Playing")
                                player.pause()
                            else
                                player.resume()
                        }
                    }

                    Controls.ToolTip {
                        visible: playBtnArea.containsMouse
                        text: (player.state_text === "Playing") ? "Pause" : "Play"
                        delay: 400
                    }
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
                    opacity: 0.30
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.text: "Repeat — coming soon"
                    Controls.ToolTip.delay: 400
                }
            }

            Item { Layout.fillWidth: true }

            // ── Right: seek + volume ─────────────────────────────────────────
            ColumnLayout {
                Layout.preferredWidth: 270
                spacing: 6

                // Seek bar row
                RowLayout {
                    spacing: 8

                    Controls.Label {
                        id: posLabel
                        text: root.fmtTime(player.position_secs)
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.77
                        font.features: { "tnum": 1 }
                        color: root.textDim
                    }

                    Controls.Slider {
                        id: seekBar
                        Layout.fillWidth: true
                        from: 0
                        to: 1
                        value: (!seekBar.pressed && player.duration_secs > 0)
                               ? (player.position_secs / player.duration_secs)
                               : seekBar.value
                        enabled: player.duration_secs > 0

                        onPressedChanged: {
                            if (!pressed && player.duration_secs > 0) {
                                player.seek(seekBar.value)
                            }
                        }

                        background: Rectangle {
                            x: seekBar.leftPadding
                            y: seekBar.topPadding + seekBar.availableHeight / 2 - height / 2
                            width: seekBar.availableWidth
                            height: 3
                            radius: 2
                            color: Qt.rgba(1, 1, 1, 0.14)

                            Rectangle {
                                width: seekBar.visualPosition * parent.width
                                height: parent.height
                                radius: 2
                                gradient: Gradient {
                                    orientation: Gradient.Horizontal
                                    GradientStop { position: 0.0; color: Qt.lighter(root.accentColor, 1.1) }
                                    GradientStop { position: 1.0; color: root.accentColor }
                                }
                            }
                        }

                        handle: Rectangle {
                            x: seekBar.leftPadding + seekBar.visualPosition * (seekBar.availableWidth - width)
                            y: seekBar.topPadding + seekBar.availableHeight / 2 - height / 2
                            width: seekBar.pressed || seekBar.hovered ? 14 : 10
                            height: seekBar.pressed || seekBar.hovered ? 14 : 10
                            radius: width / 2
                            color: "white"
                            // Subtle shadow behind handle
                            Rectangle {
                                anchors.centerIn: parent
                                width: parent.width + 6
                                height: parent.height + 6
                                radius: (parent.width + 6) / 2
                                color: root.accentColor
                                opacity: 0.30
                                z: -1
                            }

                            Behavior on width  { NumberAnimation { duration: 100 } }
                            Behavior on height { NumberAnimation { duration: 100 } }
                        }
                    }

                    Controls.Label {
                        id: durLabel
                        text: (player.duration_secs > 0)
                              ? root.fmtTime(player.duration_secs)
                              : "—"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.77
                        font.features: { "tnum": 1 }
                        color: root.textDim
                    }
                }

                // Volume row
                RowLayout {
                    spacing: 7

                    Kirigami.Icon {
                        source: "audio-volume-medium"
                        width: 13
                        height: 13
                        color: root.textDim
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

                        background: Rectangle {
                            x: volumeSlider.leftPadding
                            y: volumeSlider.topPadding + volumeSlider.availableHeight / 2 - height / 2
                            width: volumeSlider.availableWidth
                            height: 3
                            radius: 2
                            color: Qt.rgba(1, 1, 1, 0.12)

                            Rectangle {
                                width: volumeSlider.visualPosition * parent.width
                                height: parent.height
                                radius: 2
                                color: Qt.rgba(1, 1, 1, 0.35)
                            }
                        }

                        handle: Rectangle {
                            x: volumeSlider.leftPadding + volumeSlider.visualPosition * (volumeSlider.availableWidth - width)
                            y: volumeSlider.topPadding + volumeSlider.availableHeight / 2 - height / 2
                            width: 10
                            height: 10
                            radius: 5
                            color: Qt.rgba(1, 1, 1, 0.80)
                        }
                    }
                }
            }
        }
    }

    // ── Main layout ─────────────────────────────────────────────────────────
    pageStack.initialPage: Kirigami.Page {
        id: mainPage
        globalToolBarStyle: Kirigami.ApplicationHeaderStyle.None
        leftPadding: 0
        rightPadding: 0
        topPadding: 0
        bottomPadding: 0

        // Full-window dark base gradient (depth, not flat)
        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0.0; color: "#10101a" }
                GradientStop { position: 0.5; color: root.bgBase }
                GradientStop { position: 1.0; color: "#0a0a10" }
            }
        }

        RowLayout {
            anchors.fill: parent
            spacing: 0

            // ── Left sidebar ────────────────────────────────────────────────
            Rectangle {
                id: sidebar
                Layout.preferredWidth: 216
                Layout.fillHeight: true

                gradient: Gradient {
                    GradientStop { position: 0.0; color: "#12121e" }
                    GradientStop { position: 1.0; color: "#0e0e18" }
                }

                // Right border
                Rectangle {
                    anchors.right: parent.right
                    width: 1
                    height: parent.height
                    color: root.sepColor
                }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.topMargin: 14
                    anchors.bottomMargin: 8
                    spacing: 0

                    // Library section header
                    Controls.Label {
                        Layout.leftMargin: 20
                        Layout.bottomMargin: 4
                        Layout.topMargin: 4
                        text: "Library"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                        color: root.textFaint
                        font.capitalization: Font.AllUppercase
                        letterSpacing: 1.4
                        font.weight: Font.Medium
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
                        Layout.topMargin: 8
                        Layout.bottomMargin: 8
                        Layout.leftMargin: 14
                        Layout.rightMargin: 14
                        height: 1
                        color: root.sepColor
                    }

                    // Playlists section header
                    Controls.Label {
                        Layout.leftMargin: 20
                        Layout.bottomMargin: 4
                        Layout.topMargin: 4
                        text: "Playlists"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                        color: root.textFaint
                        font.capitalization: Font.AllUppercase
                        letterSpacing: 1.4
                        font.weight: Font.Medium
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
                        Layout.topMargin: 8
                        Layout.bottomMargin: 8
                        Layout.leftMargin: 14
                        Layout.rightMargin: 14
                        height: 1
                        color: root.sepColor
                    }

                    // Sources section header
                    Controls.Label {
                        Layout.leftMargin: 20
                        Layout.bottomMargin: 4
                        Layout.topMargin: 4
                        text: "Sources"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                        color: Qt.rgba(1, 1, 1, 0.15)
                        font.capitalization: Font.AllUppercase
                        letterSpacing: 1.4
                        font.weight: Font.Medium
                    }

                    SidebarItem {
                        iconName: "podcast"
                        label: "Podcasts"
                        enabled: false
                        opacity: 0.35
                    }
                    SidebarItem {
                        iconName: "network-wireless"
                        label: "Radio"
                        enabled: false
                        opacity: 0.35
                    }
                    SidebarItem {
                        iconName: "internet-web-browser"
                        label: "YouTube"
                        enabled: false
                        opacity: 0.35
                    }

                    Item { Layout.fillHeight: true }
                }
            }

            // ── Main content area ────────────────────────────────────────────
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                // ── Songs / detail track lists ───────────────────────────────
                Item {
                    anchors.fill: parent
                    visible: root.view === "songs"
                             || root.view === "album_detail"
                             || root.view === "artist_detail"
                             || root.view === "genre_detail"

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 0

                        // Detail header (drill-down views)
                        Item {
                            Layout.fillWidth: true
                            height: (root.view === "album_detail"
                                     || root.view === "artist_detail"
                                     || root.view === "genre_detail")
                                    ? 48 : 0
                            visible: root.view === "album_detail"
                                     || root.view === "artist_detail"
                                     || root.view === "genre_detail"
                            clip: true

                            Rectangle {
                                anchors.fill: parent
                                color: Qt.rgba(1, 1, 1, 0.03)
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 16
                                anchors.rightMargin: 16
                                spacing: 10

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
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.08
                                    color: root.textPrimary
                                }
                            }

                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: 1
                                color: root.sepColor
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
                                    height: 36

                                    Rectangle {
                                        anchors.fill: parent
                                        color: Qt.rgba(1, 1, 1, 0.03)
                                    }

                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.leftMargin: 16
                                        anchors.rightMargin: 16
                                        spacing: 0

                                        Item { width: 62 }

                                        Controls.Label {
                                            Layout.fillWidth: true
                                            Layout.leftMargin: Kirigami.Units.largeSpacing
                                            text: "Title / Artist"
                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                            color: root.textFaint
                                            font.capitalization: Font.AllUppercase
                                            letterSpacing: 1.0
                                            font.weight: Font.Medium
                                        }

                                        Controls.Label {
                                            Layout.preferredWidth: 50
                                            horizontalAlignment: Text.AlignRight
                                            text: "Time"
                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                            color: root.textFaint
                                            font.capitalization: Font.AllUppercase
                                            letterSpacing: 1.0
                                            font.weight: Font.Medium
                                        }
                                    }

                                    Rectangle {
                                        anchors.bottom: parent.bottom
                                        width: parent.width
                                        height: 1
                                        color: root.sepColor
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

                                // Empty / scanning placeholders
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

                // ── Albums grid view ─────────────────────────────────────────
                Item {
                    anchors.fill: parent
                    visible: root.view === "albums"

                    GridView {
                        id: albumGrid
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.largeSpacing + 4
                        model: root.albums
                        clip: true

                        property int cellTargetWidth: 168
                        property int cols: Math.max(2, Math.floor(width / cellTargetWidth))
                        cellWidth: Math.floor(width / cols)
                        cellHeight: cellWidth + 86

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        delegate: Item {
                            width: albumGrid.cellWidth
                            height: albumGrid.cellHeight

                            AlbumCard {
                                anchors.fill: parent
                                anchors.margins: 5
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

                // ── Artists list view ────────────────────────────────────────
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
                            height: 36

                            Rectangle {
                                anchors.fill: parent
                                color: Qt.rgba(1, 1, 1, 0.03)
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 16
                                anchors.rightMargin: 16
                                spacing: 0

                                Controls.Label {
                                    Layout.fillWidth: true
                                    text: "Artist"
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                    color: root.textFaint
                                    font.capitalization: Font.AllUppercase
                                    letterSpacing: 1.0
                                    font.weight: Font.Medium
                                }

                                Controls.Label {
                                    text: "Albums · Tracks"
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                    color: root.textFaint
                                    font.capitalization: Font.AllUppercase
                                    letterSpacing: 1.0
                                    font.weight: Font.Medium
                                }
                            }

                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: 1
                                color: root.sepColor
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

                // ── Genres list view ─────────────────────────────────────────
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
                            height: 36

                            Rectangle {
                                anchors.fill: parent
                                color: Qt.rgba(1, 1, 1, 0.03)
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 16
                                anchors.rightMargin: 16
                                spacing: 0

                                Controls.Label {
                                    Layout.fillWidth: true
                                    text: "Genre"
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                    color: root.textFaint
                                    font.capitalization: Font.AllUppercase
                                    letterSpacing: 1.0
                                    font.weight: Font.Medium
                                }

                                Controls.Label {
                                    text: "Tracks"
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                    color: root.textFaint
                                    font.capitalization: Font.AllUppercase
                                    letterSpacing: 1.0
                                    font.weight: Font.Medium
                                }
                            }

                            Rectangle {
                                anchors.bottom: parent.bottom
                                width: parent.width
                                height: 1
                                color: root.sepColor
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

            // ── Right Now-Playing + queue panel ─────────────────────────────
            Rectangle {
                id: nowPlayingPanel
                Layout.preferredWidth: root.nowPlayingVisible ? 296 : 0
                Layout.fillHeight: true
                clip: true
                visible: root.nowPlayingVisible

                // Base color — will be overlaid by ambient backdrop
                color: "#0d0d1a"

                Behavior on Layout.preferredWidth {
                    NumberAnimation { duration: 200; easing.type: Easing.InOutQuad }
                }

                // Left border
                Rectangle {
                    anchors.left: parent.left
                    width: 1
                    height: parent.height
                    color: root.sepColor
                    z: 10
                }

                // ── Ambient album-art backdrop ───────────────────────────────
                // A blurred version of the cover fills the panel and glows with
                // the song's own colours.
                Item {
                    id: ambientBackdrop
                    anchors.fill: parent
                    clip: true
                    opacity: (player.current_cover_thumb || "").length > 0 ? 1.0 : 0.0

                    Behavior on opacity { NumberAnimation { duration: 600; easing.type: Easing.InOutCubic } }

                    // Source image (invisible — fed into MultiEffect)
                    Image {
                        id: backdropSource
                        anchors.fill: parent
                        source: (player.current_cover_thumb || "").length > 0
                                ? "file://" + player.current_cover_thumb
                                : ""
                        fillMode: Image.PreserveAspectCrop
                        visible: false
                        asynchronous: true
                    }

                    // MultiEffect blur
                    MultiEffect {
                        source: backdropSource
                        anchors.fill: backdropSource
                        blurEnabled: true
                        blurMax: 64
                        blur: 1.0
                        autoPaddingEnabled: false
                        opacity: 0.35
                    }

                    // Dark gradient overlay for legibility — covers most of the panel
                    Rectangle {
                        anchors.fill: parent
                        gradient: Gradient {
                            GradientStop { position: 0.0; color: Qt.rgba(0.05, 0.05, 0.10, 0.55) }
                            GradientStop { position: 0.45; color: Qt.rgba(0.05, 0.05, 0.10, 0.80) }
                            GradientStop { position: 1.0; color: Qt.rgba(0.05, 0.05, 0.10, 0.95) }
                        }
                    }
                }

                // ── Idle gradient (shown when no track) ──────────────────────
                Rectangle {
                    anchors.fill: parent
                    opacity: (player.current_cover_thumb || "").length > 0 ? 0.0 : 1.0
                    gradient: Gradient {
                        GradientStop { position: 0.0; color: "#14142a" }
                        GradientStop { position: 0.5; color: "#0d0d1a" }
                        GradientStop { position: 1.0; color: "#090912" }
                    }

                    Behavior on opacity { NumberAnimation { duration: 600 } }
                }

                // ── Panel content ────────────────────────────────────────────
                ColumnLayout {
                    anchors.fill: parent
                    anchors.topMargin: 18
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    anchors.bottomMargin: 10
                    spacing: 10

                    // ── Large cover with multi-layer shadow ──────────────────
                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: width

                        // Outermost shadow
                        Rectangle {
                            anchors.centerIn: parent
                            width: parent.width - 10
                            height: parent.height - 10
                            anchors.verticalCenterOffset: 18
                            radius: 20
                            color: "#000000"
                            opacity: 0.60
                        }
                        // Middle shadow
                        Rectangle {
                            anchors.centerIn: parent
                            width: parent.width - 4
                            height: parent.height - 4
                            anchors.verticalCenterOffset: 10
                            radius: 18
                            color: "#000000"
                            opacity: 0.35
                        }

                        Rectangle {
                            id: npCoverFrame
                            anchors.fill: parent
                            radius: 16
                            color: Qt.rgba(1, 1, 1, 0.07)
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

                            // Idle state — styled gradient fallback
                            Rectangle {
                                anchors.fill: parent
                                visible: !npCover.visible
                                radius: 16
                                gradient: Gradient {
                                    GradientStop { position: 0.0; color: "#1e1e3a" }
                                    GradientStop { position: 0.6; color: "#14142a" }
                                    GradientStop { position: 1.0; color: "#0d0d1e" }
                                }

                                // Abstract music note icon
                                Kirigami.Icon {
                                    anchors.centerIn: parent
                                    source: "media-optical-audio"
                                    width: 72
                                    height: 72
                                    color: Qt.rgba(1, 1, 1, 0.14)
                                }
                            }

                            // Subtle inner specular on top
                            Rectangle {
                                anchors.top: parent.top
                                anchors.left: parent.left
                                anchors.right: parent.right
                                height: parent.height * 0.3
                                radius: 16
                                gradient: Gradient {
                                    GradientStop { position: 0.0; color: Qt.rgba(1, 1, 1, 0.06) }
                                    GradientStop { position: 1.0; color: "transparent" }
                                }
                                z: 2
                            }
                        }
                    }

                    // Track title
                    Controls.Label {
                        Layout.fillWidth: true
                        text: player.current_title || "Nothing Playing"
                        font.bold: true
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.16
                        elide: Text.ElideRight
                        color: (player.current_title || "").length > 0
                               ? root.textPrimary
                               : root.textDim
                    }

                    // Artist
                    Controls.Label {
                        Layout.fillWidth: true
                        text: player.current_artist || ""
                        color: root.textDim
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.92
                        elide: Text.ElideRight
                        visible: text.length > 0
                    }

                    // State badge
                    Item {
                        Layout.fillWidth: true
                        height: stateBadge.height

                        Rectangle {
                            id: stateBadgeBg
                            anchors.left: parent.left
                            height: stateBadge.height + 6
                            width: stateBadge.width + 16
                            radius: height / 2
                            color: (player.state_text === "Playing")
                                   ? Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.18)
                                   : Qt.rgba(1, 1, 1, 0.07)
                        }

                        Controls.Label {
                            id: stateBadge
                            anchors.left: stateBadgeBg.left
                            anchors.leftMargin: 8
                            anchors.verticalCenter: stateBadgeBg.verticalCenter
                            text: player.state_text || "Stopped"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                            font.capitalization: Font.AllUppercase
                            letterSpacing: 0.9
                            color: (player.state_text === "Playing")
                                   ? root.accentColor
                                   : root.textDim
                            font.weight: Font.Medium
                        }
                    }

                    // Thin separator
                    Rectangle {
                        Layout.fillWidth: true
                        height: 1
                        color: root.sepColor
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
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                            flat: true
                            checkable: false
                            Layout.fillWidth: true
                            onClicked: tabRow.activeTab = "queue"
                            opacity: tabRow.activeTab === "queue" ? 1.0 : 0.40
                        }

                        Rectangle {
                            width: 1
                            height: 14
                            color: root.sepColor
                        }

                        Controls.ToolButton {
                            id: tabLyricsBtn
                            text: "Lyrics"
                            font.bold: tabRow.activeTab === "lyrics"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                            flat: true
                            checkable: false
                            Layout.fillWidth: true
                            onClicked: tabRow.activeTab = "lyrics"
                            opacity: tabRow.activeTab === "lyrics" ? 1.0 : 0.40
                        }
                    }

                    // Active tab underline
                    Item {
                        Layout.fillWidth: true
                        height: 2

                        Rectangle {
                            y: 0
                            x: tabRow.activeTab === "queue" ? 0 : parent.width * 0.5
                            width: parent.width * 0.5
                            height: 2
                            radius: 1
                            color: root.accentColor
                            // Glow on underline
                            Rectangle {
                                anchors.centerIn: parent
                                width: parent.width
                                height: 6
                                radius: 3
                                color: root.accentColor
                                opacity: 0.30
                                z: -1
                            }

                            Behavior on x { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
                        }
                    }

                    // ── Up Next queue ────────────────────────────────────────
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
                            height: 56

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 2
                                anchors.rightMargin: 4
                                spacing: 10

                                // Mini cover
                                Rectangle {
                                    width: 40
                                    height: 40
                                    radius: 7
                                    color: Qt.rgba(1, 1, 1, 0.07)
                                    clip: true

                                    // Shadow
                                    Rectangle {
                                        anchors.centerIn: parent
                                        width: parent.width + 4
                                        height: parent.height + 4
                                        anchors.verticalCenterOffset: 3
                                        radius: 9
                                        color: "#000000"
                                        opacity: 0.30
                                        z: -1
                                    }

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
                                        color: Qt.rgba(1, 1, 1, 0.25)
                                        visible: !qCoverImg.visible
                                    }
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 3

                                    Controls.Label {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: (modelData && modelData.title) ? modelData.title : "(untitled)"
                                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.87
                                        color: root.textPrimary
                                    }
                                    Controls.Label {
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                        text: (modelData && modelData.artist) ? modelData.artist : ""
                                        color: root.textDim
                                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.77
                                        visible: text.length > 0
                                    }
                                }
                            }

                            Rectangle {
                                anchors.bottom: parent.bottom
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.leftMargin: 50
                                height: 1
                                color: Qt.rgba(1, 1, 1, 0.06)
                            }
                        }

                        Controls.Label {
                            anchors.centerIn: parent
                            visible: queueList.count === 0
                            text: "Queue is empty"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                            color: root.textFaint
                        }
                    }

                    // ── Lyrics panel ─────────────────────────────────────────
                    Item {
                        id: lyricsPanel
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: tabRow.activeTab === "lyrics"
                        clip: true

                        property var lyricsData: {
                            var s = player.lyrics_json
                            if (!s || s.length === 0) return { synced: false, lines: [] }
                            try { return JSON.parse(s) } catch(e) { return { synced: false, lines: [] } }
                        }

                        property bool hasLines: lyricsData && lyricsData.lines && lyricsData.lines.length > 0

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

                        onActiveLineIndexChanged: {
                            if (activeLineIndex >= 0 && lyricsData && lyricsData.synced) {
                                syncedLyricsList.positionViewAtIndex(activeLineIndex, ListView.Center)
                            }
                        }

                        Controls.Label {
                            anchors.centerIn: parent
                            visible: !lyricsPanel.hasLines
                            text: "No lyrics"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.88
                            color: root.textFaint
                        }

                        // Synced lyrics list
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
                                height: lyricLineLabel.implicitHeight + 18

                                property bool isActive: index === lyricsPanel.activeLineIndex

                                Controls.Label {
                                    id: lyricLineLabel
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    anchors.leftMargin: 6
                                    anchors.rightMargin: 6
                                    text: (modelData && modelData.text) ? modelData.text : ""
                                    wrapMode: Text.WordWrap
                                    font.pointSize: parent.isActive
                                                    ? Kirigami.Theme.defaultFont.pointSize * 0.98
                                                    : Kirigami.Theme.defaultFont.pointSize * 0.86
                                    font.bold: parent.isActive
                                    color: parent.isActive
                                           ? root.accentColor
                                           : root.textPrimary
                                    opacity: parent.isActive ? 1.0 : 0.40

                                    Behavior on opacity { NumberAnimation { duration: 130 } }
                                    Behavior on font.pointSize { NumberAnimation { duration: 130 } }
                                }
                            }
                        }

                        // Unsynced lyrics
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
                                color: root.textPrimary
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
