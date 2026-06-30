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

    // ── Design tokens — light & airy; accent pulled from the current cover ──────
    // Soft neutral canvas so the white now-playing panel + artwork read as the
    // bright focal points, instead of one big flat white field.
    readonly property color bgBase:      "#f2f2f5"
    readonly property color bgSidebar:   "#e9e9ee"
    readonly property color bgContent:   "#f2f2f5"
    readonly property color bgPanel:     "#ffffff"
    readonly property color bgHeader:    "#f2f2f5"
    // No divider lines — separation is by spacing. Kept transparent so any
    // remaining separator rectangles that reference it simply vanish.
    readonly property color sepColor:    "transparent"
    readonly property color textPrimary: "#1d1d1f"
    readonly property color textDim:     "#86868b"
    readonly property color textFaint:   "#b0b0b6"
    // Accent + ambient tint are sampled from the current album art (Rust side).
    readonly property color accentColor: player.current_accent
    readonly property color accentSoft:  Qt.rgba(accentColor.r, accentColor.g, accentColor.b, 0.12)
    readonly property color hoverColor:  Qt.rgba(0, 0, 0, 0.045)

    // Force a light colour scheme so Kirigami-styled controls (switches, combos,
    // scrollbars, menus) match, and route the dynamic accent through the
    // standard highlight colour so child components pick it up automatically.
    Kirigami.Theme.inherit: false
    Kirigami.Theme.colorSet: Kirigami.Theme.View
    Kirigami.Theme.backgroundColor: "#ffffff"
    Kirigami.Theme.textColor: "#1d1d1f"
    Kirigami.Theme.highlightColor: player.current_accent

    // ── QObject instances ───────────────────────────────────────────────────
    Library { id: library }
    Player  { id: player  }

    Component.onCompleted: {
        library.loadAll()
        library.loadPlaylists()
        library.loadSmartPlaylists()
        player.initMpris()
        // Restore last session: reloads queue + current track, loaded but paused.
        // Falls back to first library tracks if no session file exists.
        player.restoreSession()
    }

    // ── View state machine ──────────────────────────────────────────────────
    property string view: "songs"
    property string detailName: ""
    property int detailPlaylistId: -1
    property int detailSmartPlaylistId: -1

    // ── Position polling timer ──────────────────────────────────────────────
    Timer {
        id: positionTimer
        interval: 250
        running: player.state_text === "Playing"
        repeat: true
        onTriggered: player.refreshPosition()
    }

    // ── Spectrum analyzer polling timer ─────────────────────────────────────
    // Polls at ~30 fps while playing; stops when paused/stopped so levels
    // decay naturally to zero on the analyzer thread.
    property var spectrumLevels: Array(24).fill(0)

    Timer {
        id: spectrumTimer
        interval: 33
        running: player.state_text === "Playing"
        repeat: true
        onTriggered: {
            var raw = player.spectrumLevels()
            if (!raw || raw.length === 0) return
            try {
                var parsed = JSON.parse(raw)
                if (Array.isArray(parsed) && parsed.length === 24) {
                    root.spectrumLevels = parsed
                }
            } catch(e) {
                // Guard: ignore bad JSON
            }
        }
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

    property var playlists: {
        var s = library.playlists_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    property var smartPlaylists: {
        var s = library.smart_playlists_json
        if (!s || s.length === 0) return []
        try { return JSON.parse(s) } catch(e) { return [] }
    }

    // ── New playlist dialog ──────────────────────────────────────────────────
    property string newPlaylistName: ""
    // Track id to add after creating a new playlist (-1 = none)
    property int _pendingAddTrackId: -1

    // Inline create-playlist dialog (Kirigami PromptDialog equivalent using
    // Controls.Dialog since Kirigami.PromptDialog may not be available in all
    // installed Kirigami versions).
    Controls.Dialog {
        id: newPlaylistDialog
        title: "New Playlist"
        modal: true
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        implicitWidth: 340
        standardButtons: Controls.Dialog.Ok | Controls.Dialog.Cancel

        background: Rectangle {
            color: "#ffffff"
            radius: 14
            border.color: Qt.rgba(0, 0, 0, 0.10)
            border.width: 1
        }
        header: Controls.Label {
            text: newPlaylistDialog.title
            font.bold: true
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.05
            color: root.textPrimary
            padding: 18
            bottomPadding: 4
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 8

            Controls.Label {
                text: "Playlist name:"
                color: root.textDim
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
            }

            Controls.TextField {
                id: newPlaylistField
                Layout.fillWidth: true
                placeholderText: "My Playlist"
                color: root.textPrimary
                background: Rectangle {
                    color: Qt.rgba(0, 0, 0, 0.05)
                    radius: 6
                }
                Keys.onReturnPressed: newPlaylistDialog.accept()
            }
        }

        onAccepted: {
            var n = newPlaylistField.text.trim()
            if (n.length > 0) {
                library.createPlaylist(n)
                // If a track was queued to add, find the new playlist id after
                // playlists_json updates and add the track.
                if (root._pendingAddTrackId >= 0) {
                    var tid = root._pendingAddTrackId
                    // Use a short timer to let playlists_json update first.
                    Qt.callLater(function() {
                        var pls = root.playlists
                        // Find the playlist matching the name we just created.
                        for (var i = 0; i < pls.length; i++) {
                            if (pls[i].name === n) {
                                library.addToPlaylist(pls[i].id, tid)
                                break
                            }
                        }
                    })
                }
            }
            newPlaylistField.text = ""
            root._pendingAddTrackId = -1
        }
        onRejected: {
            newPlaylistField.text = ""
            root._pendingAddTrackId = -1
        }
    }

    // ── Rename playlist dialog ───────────────────────────────────────────────
    property int renamePlaylistId: -1

    Controls.Dialog {
        id: renamePlaylistDialog
        title: "Rename Playlist"
        modal: true
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        implicitWidth: 340
        standardButtons: Controls.Dialog.Ok | Controls.Dialog.Cancel

        background: Rectangle {
            color: "#ffffff"
            radius: 14
            border.color: Qt.rgba(0, 0, 0, 0.10)
            border.width: 1
        }
        header: Controls.Label {
            text: renamePlaylistDialog.title
            font.bold: true
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.05
            color: root.textPrimary
            padding: 18
            bottomPadding: 4
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 8

            Controls.Label {
                text: "New name:"
                color: root.textDim
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
            }

            Controls.TextField {
                id: renamePlaylistField
                Layout.fillWidth: true
                placeholderText: "Playlist name"
                color: root.textPrimary
                background: Rectangle {
                    color: Qt.rgba(0, 0, 0, 0.05)
                    radius: 6
                }
                Keys.onReturnPressed: renamePlaylistDialog.accept()
            }
        }

        onAccepted: {
            var n = renamePlaylistField.text.trim()
            if (n.length > 0 && root.renamePlaylistId >= 0) {
                library.renamePlaylist(root.renamePlaylistId, n)
                root.detailName = n
            }
            renamePlaylistField.text = ""
            root.renamePlaylistId = -1
        }
        onRejected: {
            renamePlaylistField.text = ""
            root.renamePlaylistId = -1
        }
    }

    // ── Delete-playlist confirmation dialog ──────────────────────────────────
    property int deletePlaylistId: -1
    property bool deleteIsSmart: false
    property string deletePlaylistName: ""

    Controls.Dialog {
        id: deletePlaylistDialog
        modal: true
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        implicitWidth: 360
        padding: 0

        background: Rectangle {
            color: "#ffffff"
            radius: 14
            border.color: Qt.rgba(0, 0, 0, 0.10)
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 14

            // Header with destructive icon
            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 20
                Layout.leftMargin: 20
                Layout.rightMargin: 20
                spacing: 12

                Rectangle {
                    width: 38; height: 38; radius: 19
                    color: Qt.rgba(0.93, 0.30, 0.30, 0.16)
                    Kirigami.Icon {
                        anchors.centerIn: parent
                        source: "edit-delete"
                        width: 20; height: 20
                        color: "#e85d5d"
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2
                    Controls.Label {
                        text: "Delete playlist?"
                        font.bold: true
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.05
                        color: root.textPrimary
                    }
                    Controls.Label {
                        Layout.fillWidth: true
                        text: "“" + root.deletePlaylistName + "” will be removed. This can't be undone."
                        wrapMode: Text.WordWrap
                        color: root.textDim
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.85
                    }
                }
            }

            // Action buttons
            RowLayout {
                Layout.fillWidth: true
                Layout.bottomMargin: 16
                Layout.leftMargin: 20
                Layout.rightMargin: 20
                spacing: 8

                Item { Layout.fillWidth: true }

                Controls.Button {
                    text: "Cancel"
                    flat: true
                    onClicked: deletePlaylistDialog.close()
                }

                Controls.Button {
                    text: "Delete"
                    highlighted: true
                    onClicked: deletePlaylistDialog.accept()
                    contentItem: Controls.Label {
                        text: "Delete"
                        color: "white"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        radius: 7
                        color: parent.down ? "#c0392b" : (parent.hovered ? "#e74c3c" : "#d64541")
                        implicitWidth: 84
                        implicitHeight: 32
                    }
                }
            }
        }

        onAccepted: {
            if (root.deletePlaylistId >= 0) {
                if (root.deleteIsSmart)
                    library.deleteSmartPlaylist(root.deletePlaylistId)
                else
                    library.deletePlaylist(root.deletePlaylistId)
                root.view = "songs"
                root.detailPlaylistId = -1
                root.detailSmartPlaylistId = -1
                root.detailName = ""
            }
            root.deletePlaylistId = -1
            root.deletePlaylistName = ""
        }
    }

    // ── Smart Playlist rule-builder dialog ────────────────────────────────────

    // Internal rule model: [{field, op, value}]
    property var _spRules: [{"field":"genre","op":"is","value":""}]
    property bool _spMatchAll: true

    Controls.Dialog {
        id: newSmartPlaylistDialog
        title: "New Smart Playlist"
        modal: true
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        width: 480
        standardButtons: Controls.Dialog.Ok | Controls.Dialog.Cancel

        ColumnLayout {
            width: parent.width
            spacing: 10

            // Name field
            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                Controls.Label { text: "Name:"; color: root.textPrimary; Layout.preferredWidth: 60 }
                Controls.TextField {
                    id: spNameField
                    Layout.fillWidth: true
                    placeholderText: "Playlist name"
                    color: root.textPrimary
                    background: Rectangle { color: Qt.rgba(0,0,0,0.05); radius: 6 }
                }
            }

            // Match all / any
            RowLayout {
                Layout.fillWidth: true
                spacing: 8
                Controls.Label { text: "Match:"; color: root.textPrimary; Layout.preferredWidth: 60 }
                Controls.ComboBox {
                    id: spMatchCombo
                    model: ["All rules (AND)", "Any rule (OR)"]
                    currentIndex: 0
                    implicitWidth: 160
                    onCurrentIndexChanged: root._spMatchAll = (currentIndex === 0)
                }
            }

            // Rule rows (up to 4)
            Repeater {
                id: spRuleRepeater
                model: root._spRules

                delegate: RowLayout {
                    required property var modelData
                    required property int index
                    Layout.fillWidth: true
                    spacing: 6

                    Controls.ComboBox {
                        id: spFieldCombo
                        model: ["title","artist","album","genre","year"]
                        currentIndex: {
                            var f = modelData ? modelData.field : "genre"
                            var idx = model.indexOf(f)
                            return idx >= 0 ? idx : 3
                        }
                        implicitWidth: 90
                        onActivated: {
                            var rules = root._spRules.slice()
                            rules[index] = {"field": currentText, "op": rules[index].op, "value": rules[index].value}
                            root._spRules = rules
                        }
                    }

                    Controls.ComboBox {
                        id: spOpCombo
                        model: {
                            var f = modelData ? modelData.field : "genre"
                            if (f === "year") return ["is",">","<"]
                            return ["contains","is"]
                        }
                        currentIndex: {
                            var op = modelData ? modelData.op : "is"
                            var ops = spOpCombo.model
                            var idx = ops.indexOf(op)
                            return idx >= 0 ? idx : 0
                        }
                        implicitWidth: 90
                        onActivated: {
                            var rules = root._spRules.slice()
                            var opVal = currentText === ">" ? "gt" : (currentText === "<" ? "lt" : currentText)
                            rules[index] = {"field": rules[index].field, "op": opVal, "value": rules[index].value}
                            root._spRules = rules
                        }
                    }

                    Controls.TextField {
                        id: spValueField
                        Layout.fillWidth: true
                        text: modelData ? (modelData.value || "") : ""
                        placeholderText: "value"
                        color: root.textPrimary
                        background: Rectangle { color: Qt.rgba(0,0,0,0.05); radius: 6 }
                        onTextChanged: {
                            var rules = root._spRules.slice()
                            rules[index] = {"field": rules[index].field, "op": rules[index].op, "value": text}
                            root._spRules = rules
                        }
                    }

                    Controls.ToolButton {
                        text: "×"
                        font.bold: true
                        visible: root._spRules.length > 1
                        flat: true
                        padding: 2
                        implicitWidth: 22
                        implicitHeight: 22
                        onClicked: {
                            var rules = root._spRules.slice()
                            rules.splice(index, 1)
                            root._spRules = rules
                        }
                    }
                }
            }

            // Add rule button (max 4)
            Controls.Button {
                text: "+ Add Rule"
                visible: root._spRules.length < 4
                flat: true
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                onClicked: {
                    var rules = root._spRules.slice()
                    rules.push({"field":"genre","op":"is","value":""})
                    root._spRules = rules
                }
            }
        }

        onOpened: {
            spNameField.text = ""
            spMatchCombo.currentIndex = 0
            root._spMatchAll = true
            root._spRules = [{"field":"genre","op":"is","value":""}]
        }

        onAccepted: {
            var n = spNameField.text.trim()
            if (n.length === 0) return

            // Build rules_json — map display op back to canonical op strings
            var rules = []
            for (var i = 0; i < root._spRules.length; i++) {
                var r = root._spRules[i]
                if (!r || !r.value || r.value.trim() === "") continue
                var op = r.op
                // map display values to canonical
                if (op === ">") op = "gt"
                else if (op === "<") op = "lt"
                rules.push({"field": r.field.toLowerCase(), "op": op.toLowerCase(), "value": r.value})
            }
            if (rules.length === 0) return
            var rulesJson = JSON.stringify(rules)
            library.createSmartPlaylist(n, rulesJson, root._spMatchAll)
        }
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

        // ToolBars otherwise adopt a system colorSet (dark on a dark Plasma
        // theme), turning default icons/text white — invisible on the light bar.
        // Force the light scheme so default-coloured icons render dark.
        Kirigami.Theme.inherit: false
        Kirigami.Theme.colorSet: Kirigami.Theme.View
        Kirigami.Theme.backgroundColor: root.bgHeader
        Kirigami.Theme.textColor: root.textPrimary
        Kirigami.Theme.highlightColor: player.current_accent

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
                font.letterSpacing: -0.5
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
                color: root.textPrimary
                placeholderTextColor: root.textDim
                background: Rectangle {
                    radius: 9
                    color: Qt.rgba(0, 0, 0, 0.05)
                }
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

            Controls.ToolButton {
                icon.name: "configure"
                checkable: false
                highlighted: root.view === "settings"
                icon.color: root.view === "settings" ? root.accentColor : Kirigami.Theme.textColor
                onClicked: root.view = root.view === "settings" ? "songs" : "settings"
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.text: "Settings"
                Controls.ToolTip.delay: 600
            }
        }
    }

    // ── Bottom transport bar ────────────────────────────────────────────────
    footer: Controls.ToolBar {
        id: transportBar
        height: 82

        // Force light scheme (see header) so transport icons render dark, not white.
        Kirigami.Theme.inherit: false
        Kirigami.Theme.colorSet: Kirigami.Theme.View
        Kirigami.Theme.backgroundColor: root.bgHeader
        Kirigami.Theme.textColor: root.textPrimary
        Kirigami.Theme.highlightColor: player.current_accent

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
                        opacity: 0.14
                    }
                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width + 2
                        height: parent.height + 2
                        anchors.verticalCenterOffset: 3
                        radius: 10
                        color: "#000000"
                        opacity: 0.10
                    }

                    Rectangle {
                        anchors.fill: parent
                        radius: 9
                        color: Qt.rgba(0, 0, 0, 0.05)
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
                            color: root.textFaint
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
                    opacity: player.shuffle ? 1.0 : 0.9
                    icon.color: player.shuffle ? root.accentColor : "#5e5e66"
                    onClicked: player.toggleShuffle()
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.text: player.shuffle ? "Shuffle: On" : "Shuffle: Off"
                    Controls.ToolTip.delay: 400
                }

                Controls.ToolButton {
                    icon.name: "media-skip-backward"
                    icon.color: "#5e5e66"
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
                            if (player.state_text === "Playing") {
                                player.pause()
                            } else if (player.state_text === "Paused") {
                                player.resume()
                            } else {
                                // Stopped but a track is loaded (e.g. restored session):
                                // start from the saved position.
                                player.playCurrent()
                            }
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
                    icon.color: "#5e5e66"
                    onClicked: player.next()
                    enabled: (player.state_text || "Stopped") !== "Stopped"
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.text: "Next"
                    Controls.ToolTip.delay: 400
                }

                Controls.ToolButton {
                    icon.name: (player.repeat_mode === "one")
                                ? "media-playlist-repeat-song"
                                : "media-playlist-repeat"
                    opacity: (player.repeat_mode !== "off") ? 1.0 : 0.9
                    icon.color: (player.repeat_mode !== "off")
                                 ? root.accentColor
                                 : "#5e5e66"
                    onClicked: player.cycleRepeat()
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.text: {
                        if (player.repeat_mode === "all") return "Repeat: All"
                        if (player.repeat_mode === "one") return "Repeat: One"
                        return "Repeat: Off"
                    }
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
                        // Custom background/handle have no implicit height, so the
                        // slider would collapse to height 0 (no hit area, undraggable).
                        // Give it an explicit interactive height.
                        Layout.preferredHeight: 20
                        implicitHeight: 20
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
                            color: Qt.rgba(0, 0, 0, 0.10)

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
                        // See seekBar: avoid collapsing to height 0 (undraggable).
                        Layout.preferredHeight: 20
                        implicitHeight: 20
                        from: 0
                        to: 1
                        value: player.volume
                        Controls.ToolTip.visible: hovered
                        Controls.ToolTip.text: "Volume: " + Math.round(player.volume * 100) + "%"
                        Controls.ToolTip.delay: 400
                        onMoved: player.changeVolume(value)

                        background: Rectangle {
                            x: volumeSlider.leftPadding
                            y: volumeSlider.topPadding + volumeSlider.availableHeight / 2 - height / 2
                            width: volumeSlider.availableWidth
                            height: 3
                            radius: 2
                            color: Qt.rgba(0, 0, 0, 0.10)

                            Rectangle {
                                width: volumeSlider.visualPosition * parent.width
                                height: parent.height
                                radius: 2
                                color: root.accentColor
                            }
                        }

                        handle: Rectangle {
                            x: volumeSlider.leftPadding + volumeSlider.visualPosition * (volumeSlider.availableWidth - width)
                            y: volumeSlider.topPadding + volumeSlider.availableHeight / 2 - height / 2
                            width: 10
                            height: 10
                            radius: 5
                            color: root.accentColor
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

        // Full-window light base
        Rectangle {
            anchors.fill: parent
            color: root.bgBase
        }

        RowLayout {
            anchors.fill: parent
            spacing: 0

            // ── Left sidebar ────────────────────────────────────────────────
            Rectangle {
                id: sidebar
                Layout.preferredWidth: 216
                Layout.fillHeight: true

                color: root.bgSidebar

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
                        font.letterSpacing: 1.4
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
                        active: root.view === "recently"
                        onActivated: {
                            root.view = "recently"
                            library.loadRecentlyAdded()
                        }
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

                    // Playlists section header + New button
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.leftMargin: 20
                        Layout.rightMargin: 8
                        Layout.topMargin: 4
                        Layout.bottomMargin: 4
                        spacing: 0

                        Controls.Label {
                            Layout.fillWidth: true
                            text: "Playlists"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                            color: root.textFaint
                            font.capitalization: Font.AllUppercase
                            font.letterSpacing: 1.4
                            font.weight: Font.Medium
                        }

                        Controls.ToolButton {
                            text: "+"
                            font.bold: true
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.80
                            implicitHeight: 20
                            implicitWidth: 22
                            opacity: 0.60
                            flat: true
                            padding: 0
                            Controls.ToolTip.visible: hovered
                            Controls.ToolTip.text: "New playlist"
                            Controls.ToolTip.delay: 400
                            onClicked: {
                                newPlaylistField.text = ""
                                newPlaylistDialog.open()
                            }
                        }
                    }

                    // Real playlists from db
                    Repeater {
                        model: root.playlists
                        delegate: SidebarItem {
                            required property var modelData
                            iconName: "view-media-playlist"
                            label: modelData ? (modelData.name || "Untitled") : ""
                            active: root.view === "playlist_detail" && root.detailPlaylistId === (modelData ? modelData.id : -1)
                            onActivated: {
                                if (!modelData) return
                                root.detailName = modelData.name || ""
                                root.detailPlaylistId = modelData.id
                                library.loadPlaylistTracks(modelData.id)
                                root.view = "playlist_detail"
                            }
                        }
                    }

                    // "No playlists" hint when empty
                    Controls.Label {
                        visible: root.playlists.length === 0
                        Layout.leftMargin: 20
                        text: "No playlists yet"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.72
                        color: root.textFaint
                        font.italic: true
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

                    // Smart Playlists section header + New button
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.leftMargin: 20
                        Layout.rightMargin: 8
                        Layout.topMargin: 4
                        Layout.bottomMargin: 4
                        spacing: 0

                        Controls.Label {
                            Layout.fillWidth: true
                            text: "Smart Playlists"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                            color: root.textFaint
                            font.capitalization: Font.AllUppercase
                            font.letterSpacing: 1.4
                            font.weight: Font.Medium
                        }

                        Controls.ToolButton {
                            text: "+"
                            font.bold: true
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.80
                            implicitHeight: 20
                            implicitWidth: 22
                            opacity: 0.60
                            flat: true
                            padding: 0
                            Controls.ToolTip.visible: hovered
                            Controls.ToolTip.text: "New smart playlist"
                            Controls.ToolTip.delay: 400
                            onClicked: {
                                newSmartPlaylistDialog.open()
                            }
                        }
                    }

                    // Smart playlists from db
                    Repeater {
                        model: root.smartPlaylists
                        delegate: SidebarItem {
                            required property var modelData
                            iconName: "view-filter"
                            label: modelData ? (modelData.name || "Untitled") : ""
                            active: root.view === "smart_detail" && root.detailSmartPlaylistId === (modelData ? modelData.id : -1)
                            onActivated: {
                                if (!modelData) return
                                root.detailName = modelData.name || ""
                                root.detailSmartPlaylistId = modelData.id
                                library.loadSmartPlaylistTracks(modelData.id)
                                root.view = "smart_detail"
                            }
                        }
                    }

                    // "No smart playlists" hint when empty
                    Controls.Label {
                        visible: root.smartPlaylists.length === 0
                        Layout.leftMargin: 20
                        text: "No smart playlists"
                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.72
                        color: root.textFaint
                        font.italic: true
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
                        color: root.textFaint
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 1.4
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
                             || root.view === "playlist_detail"
                             || root.view === "recently"
                             || root.view === "smart_detail"

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 0

                        // Detail header (drill-down views)
                        Item {
                            Layout.fillWidth: true
                            height: (root.view === "album_detail"
                                     || root.view === "artist_detail"
                                     || root.view === "genre_detail"
                                     || root.view === "playlist_detail"
                                     || root.view === "recently"
                                     || root.view === "smart_detail")
                                    ? 48 : 0
                            visible: root.view === "album_detail"
                                     || root.view === "artist_detail"
                                     || root.view === "genre_detail"
                                     || root.view === "playlist_detail"
                                     || root.view === "recently"
                                     || root.view === "smart_detail"
                            clip: true

                            Rectangle {
                                anchors.fill: parent
                                color: Qt.rgba(0, 0, 0, 0.03)
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 16
                                anchors.rightMargin: 16
                                spacing: 10

                                Controls.ToolButton {
                                    text: {
                                        if (root.view === "album_detail") return "‹ Albums"
                                        if (root.view === "genre_detail") return "‹ Genres"
                                        if (root.view === "playlist_detail") return "‹ Playlists"
                                        if (root.view === "recently") return "‹ Library"
                                        if (root.view === "smart_detail") return "‹ Smart Playlists"
                                        return "‹ Artists"
                                    }
                                    onClicked: {
                                        if (root.view === "album_detail")
                                            root.view = "albums"
                                        else if (root.view === "genre_detail")
                                            root.view = "genres"
                                        else if (root.view === "playlist_detail")
                                            root.view = "songs"
                                        else if (root.view === "recently")
                                            root.view = "songs"
                                        else if (root.view === "smart_detail") {
                                            root.view = "songs"
                                            root.detailSmartPlaylistId = -1
                                            root.detailName = ""
                                        } else
                                            root.view = "artists"
                                    }
                                }

                                Controls.Label {
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                    text: root.view === "recently" ? "Recently Added" : root.detailName
                                    font.bold: true
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.08
                                    color: root.textPrimary
                                }

                                // Smart playlist indicator icon
                                Kirigami.Icon {
                                    visible: root.view === "smart_detail"
                                    source: "view-filter"
                                    width: 16
                                    height: 16
                                    color: root.accentColor
                                    opacity: 0.75
                                }

                                // Playlist-specific actions (rename / delete)
                                Controls.ToolButton {
                                    visible: root.view === "playlist_detail"
                                    icon.name: "edit-rename"
                                    Controls.ToolTip.visible: hovered
                                    Controls.ToolTip.text: "Rename playlist"
                                    Controls.ToolTip.delay: 400
                                    onClicked: {
                                        root.renamePlaylistId = root.detailPlaylistId
                                        renamePlaylistField.text = root.detailName
                                        renamePlaylistDialog.open()
                                    }
                                }

                                Controls.ToolButton {
                                    visible: root.view === "playlist_detail"
                                    icon.name: "edit-delete"
                                    Controls.ToolTip.visible: hovered
                                    Controls.ToolTip.text: "Delete playlist"
                                    Controls.ToolTip.delay: 400
                                    onClicked: {
                                        root.deletePlaylistId = root.detailPlaylistId
                                        root.deleteIsSmart = false
                                        root.deletePlaylistName = root.detailName
                                        deletePlaylistDialog.open()
                                    }
                                }

                                Controls.ToolButton {
                                    visible: root.view === "smart_detail"
                                    icon.name: "edit-delete"
                                    Controls.ToolTip.visible: hovered
                                    Controls.ToolTip.text: "Delete smart playlist"
                                    Controls.ToolTip.delay: 400
                                    onClicked: {
                                        root.deletePlaylistId = root.detailSmartPlaylistId
                                        root.deleteIsSmart = true
                                        root.deletePlaylistName = root.detailName
                                        deletePlaylistDialog.open()
                                    }
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
                                        color: Qt.rgba(0, 0, 0, 0.03)
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
                                            font.letterSpacing: 1.0
                                            font.weight: Font.Medium
                                        }

                                        Controls.Label {
                                            Layout.preferredWidth: 50
                                            horizontalAlignment: Text.AlignRight
                                            text: "Time"
                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                            color: root.textFaint
                                            font.capitalization: Font.AllUppercase
                                            font.letterSpacing: 1.0
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
                                    playlistsModel: root.playlists
                                    currentPlaylistId: root.view === "playlist_detail" ? root.detailPlaylistId : -1
                                    isCurrentTrack: {
                                        var title = player.current_title || ""
                                        return title.length > 0
                                               && modelData
                                               && modelData.title === title
                                    }
                                    onTrackClicked: function(idx) {
                                        player.playFromList(library.results_json, idx)
                                    }
                                    onAddToPlaylistRequested: function(trackId, playlistId) {
                                        library.addToPlaylist(playlistId, trackId)
                                    }
                                    onRemoveFromPlaylistRequested: function(trackId, playlistId) {
                                        library.removeFromPlaylist(playlistId, trackId)
                                        // Refresh the playlist track list
                                        library.loadPlaylistTracks(playlistId)
                                    }
                                    onNewPlaylistRequested: function(trackId) {
                                        root._pendingAddTrackId = trackId
                                        newPlaylistField.text = ""
                                        newPlaylistDialog.open()
                                    }
                                    onSaveTagsRequested: function(path, title, artist, album) {
                                        library.saveTrackTags(path, title, artist, album)
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
                                color: Qt.rgba(0, 0, 0, 0.03)
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
                                    font.letterSpacing: 1.0
                                    font.weight: Font.Medium
                                }

                                Controls.Label {
                                    text: "Albums · Tracks"
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                    color: root.textFaint
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 1.0
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

                // ── Settings view ────────────────────────────────────────────
                Item {
                    anchors.fill: parent
                    visible: root.view === "settings"
                    clip: true

                    Flickable {
                        id: settingsFlickable
                        anchors.fill: parent
                        contentWidth: width
                        contentHeight: settingsColumn.implicitHeight + 40
                        clip: true

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        ColumnLayout {
                            id: settingsColumn
                            width: Math.min(settingsFlickable.width, 740)
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.top: parent.top
                            anchors.topMargin: 28
                            spacing: 0

                            // ── Section: Equalizer ───────────────────────────
                            Item {
                                Layout.fillWidth: true
                                Layout.bottomMargin: 24
                                implicitHeight: eqSectionCol.implicitHeight

                                ColumnLayout {
                                    id: eqSectionCol
                                    anchors.fill: parent
                                    spacing: 0

                                    // Section header
                                    RowLayout {
                                        Layout.fillWidth: true
                                        Layout.bottomMargin: 14
                                        spacing: 14

                                        Rectangle {
                                            width: 4
                                            height: 22
                                            radius: 2
                                            color: root.accentColor
                                        }

                                        Controls.Label {
                                            text: "Equalizer"
                                            font.bold: true
                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.10
                                            color: root.textPrimary
                                        }

                                        Item { Layout.fillWidth: true }

                                        Controls.Label {
                                            text: "Enable"
                                            color: root.textDim
                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.88
                                            verticalAlignment: Text.AlignVCenter
                                        }

                                        Controls.Switch {
                                            id: eqSwitch
                                            checked: player.eq_enabled
                                            onToggled: player.setEqEnabled(checked)
                                        }
                                    }

                                    // EQ sliders card
                                    Rectangle {
                                        Layout.fillWidth: true
                                        implicitHeight: eqCardContent.implicitHeight + 24
                                        radius: 12
                                        color: Qt.rgba(0, 0, 0, 0.03)

                                        // Card border
                                        Rectangle {
                                            anchors.fill: parent
                                            radius: parent.radius
                                            color: "transparent"
                                            border.color: "transparent"
                                            border.width: 1
                                        }

                                        ColumnLayout {
                                            id: eqCardContent
                                            anchors.left: parent.left
                                            anchors.right: parent.right
                                            anchors.top: parent.top
                                            anchors.margins: 16
                                            spacing: 8

                                            // 10 vertical EQ band sliders
                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 0

                                                property var eqBands: {
                                                    var s = player.eq_bands_json
                                                    if (!s || s.length === 0) return []
                                                    try { return JSON.parse(s) } catch(e) { return [] }
                                                }

                                                Repeater {
                                                    model: parent.eqBands.length > 0 ? parent.eqBands : 10

                                                    delegate: ColumnLayout {
                                                        required property int index
                                                        Layout.fillWidth: true
                                                        spacing: 4
                                                        opacity: player.eq_enabled ? 1.0 : 0.38

                                                        Behavior on opacity { NumberAnimation { duration: 120 } }

                                                        // Gain label (+12 / 0 / -12)
                                                        Controls.Label {
                                                            Layout.alignment: Qt.AlignHCenter
                                                            text: {
                                                                var bands = parent.parent.parent.eqBands
                                                                if (!bands || !bands[index]) return "0"
                                                                var g = bands[index].gain
                                                                if (Math.abs(g) < 0.05) return "0"
                                                                return (g > 0 ? "+" : "") + g.toFixed(1)
                                                            }
                                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                                            color: {
                                                                var bands = parent.parent.parent.eqBands
                                                                if (!bands || !bands[index]) return root.textFaint
                                                                var g = bands[index].gain
                                                                if (Math.abs(g) < 0.05) return root.textFaint
                                                                return g > 0 ? root.accentColor : Qt.rgba(1, 0.4, 0.4, 0.9)
                                                            }
                                                        }

                                                        // Vertical slider
                                                        Controls.Slider {
                                                            id: eqSlider
                                                            Layout.alignment: Qt.AlignHCenter
                                                            orientation: Qt.Vertical
                                                            implicitHeight: 130
                                                            implicitWidth: 28
                                                            from: -12
                                                            to: 12
                                                            stepSize: 0.5
                                                            enabled: player.eq_enabled
                                                            value: {
                                                                var bands = parent.parent.parent.eqBands
                                                                if (!bands || !bands[index]) return 0
                                                                return bands[index].gain
                                                            }

                                                            onMoved: player.setEqGain(index, value)

                                                            background: Rectangle {
                                                                x: eqSlider.leftPadding + eqSlider.availableWidth / 2 - width / 2
                                                                y: eqSlider.topPadding
                                                                width: 3
                                                                height: eqSlider.availableHeight
                                                                radius: 2
                                                                color: Qt.rgba(0, 0, 0, 0.10)

                                                                // Filled portion (from center = 0 dB)
                                                                Rectangle {
                                                                    property real centerY: parent.height * 0.5
                                                                    property real fillY: eqSlider.visualPosition * parent.height
                                                                    y: Math.min(centerY, fillY)
                                                                    height: Math.abs(centerY - fillY)
                                                                    width: parent.width
                                                                    radius: 2
                                                                    color: {
                                                                        var bands = parent.parent.parent.parent.parent.eqBands
                                                                        if (!bands || !bands[index]) return root.accentColor
                                                                        return bands[index].gain >= 0 ? root.accentColor : Qt.rgba(1, 0.4, 0.4, 0.85)
                                                                    }
                                                                }

                                                                // Center tick mark (0 dB)
                                                                Rectangle {
                                                                    y: parent.height / 2 - height / 2
                                                                    x: -2
                                                                    width: 7
                                                                    height: 1
                                                                    color: Qt.rgba(0, 0, 0, 0.18)
                                                                }
                                                            }

                                                            handle: Rectangle {
                                                                x: eqSlider.leftPadding + eqSlider.availableWidth / 2 - width / 2
                                                                y: eqSlider.topPadding + eqSlider.visualPosition * (eqSlider.availableHeight - height)
                                                                width: eqSlider.pressed || eqSlider.hovered ? 16 : 12
                                                                height: width
                                                                radius: width / 2
                                                                color: root.accentColor

                                                                Rectangle {
                                                                    anchors.centerIn: parent
                                                                    width: parent.width + 8
                                                                    height: parent.height + 8
                                                                    radius: (parent.width + 8) / 2
                                                                    color: root.accentColor
                                                                    opacity: 0.25
                                                                    z: -1
                                                                }

                                                                Behavior on width  { NumberAnimation { duration: 80 } }
                                                                Behavior on height { NumberAnimation { duration: 80 } }
                                                            }
                                                        }

                                                        // Frequency label
                                                        Controls.Label {
                                                            Layout.alignment: Qt.AlignHCenter
                                                            text: {
                                                                var bands = parent.parent.parent.eqBands
                                                                if (!bands || !bands[index]) return ""
                                                                var f = bands[index].freq
                                                                if (f >= 1000) return (f / 1000) + "k"
                                                                return f
                                                            }
                                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                                            color: root.textDim
                                                        }
                                                    }
                                                }
                                            }

                                            // Reset button row
                                            RowLayout {
                                                Layout.fillWidth: true
                                                Item { Layout.fillWidth: true }
                                                Controls.Button {
                                                    text: "Reset"
                                                    enabled: player.eq_enabled
                                                    opacity: player.eq_enabled ? 1.0 : 0.38
                                                    flat: true
                                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                                                    onClicked: player.resetEq()

                                                    Behavior on opacity { NumberAnimation { duration: 120 } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Separator
                            Rectangle {
                                Layout.fillWidth: true
                                height: 1
                                color: root.sepColor
                                Layout.bottomMargin: 24
                            }

                            // ── Section: Audio Quality ───────────────────────
                            Item {
                                Layout.fillWidth: true
                                implicitHeight: qualitySectionCol.implicitHeight

                                ColumnLayout {
                                    id: qualitySectionCol
                                    anchors.fill: parent
                                    spacing: 0

                                    // Section header
                                    RowLayout {
                                        Layout.fillWidth: true
                                        Layout.bottomMargin: 14
                                        spacing: 14

                                        Rectangle {
                                            width: 4
                                            height: 22
                                            radius: 2
                                            color: root.accentColor
                                        }

                                        Controls.Label {
                                            text: "Audio Quality"
                                            font.bold: true
                                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.10
                                            color: root.textPrimary
                                        }

                                        Item { Layout.fillWidth: true }
                                    }

                                    // Bit-perfect card
                                    Rectangle {
                                        Layout.fillWidth: true
                                        implicitHeight: bitPerfectRow.implicitHeight + 24
                                        radius: 12
                                        color: Qt.rgba(0, 0, 0, 0.03)

                                        Rectangle {
                                            anchors.fill: parent
                                            radius: parent.radius
                                            color: "transparent"
                                            border.color: player.bit_perfect
                                                          ? Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.35)
                                                          : "transparent"
                                            border.width: 1

                                            Behavior on border.color { ColorAnimation { duration: 200 } }
                                        }

                                        RowLayout {
                                            id: bitPerfectRow
                                            anchors.left: parent.left
                                            anchors.right: parent.right
                                            anchors.verticalCenter: parent.verticalCenter
                                            anchors.margins: 16
                                            spacing: 16

                                            ColumnLayout {
                                                Layout.fillWidth: true
                                                spacing: 4

                                                Controls.Label {
                                                    text: "Bit-perfect output"
                                                    font.bold: true
                                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.95
                                                    color: root.textPrimary
                                                }

                                                Controls.Label {
                                                    Layout.fillWidth: true
                                                    text: "Bypass the equalizer and resampling for unaltered output."
                                                    color: root.textDim
                                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                                                    wrapMode: Text.WordWrap
                                                }
                                            }

                                            Controls.Switch {
                                                id: bitPerfectSwitch
                                                checked: player.bit_perfect
                                                onToggled: player.setBitPerfect(checked)
                                            }
                                        }
                                    }

                                    // Crossfade card
                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.topMargin: 10
                                        implicitHeight: crossfadeCardContent.implicitHeight + 24
                                        radius: 12
                                        color: Qt.rgba(0, 0, 0, 0.03)

                                        Rectangle {
                                            anchors.fill: parent
                                            radius: parent.radius
                                            color: "transparent"
                                            border.color: player.crossfade_secs > 0
                                                          ? Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.35)
                                                          : "transparent"
                                            border.width: 1

                                            Behavior on border.color { ColorAnimation { duration: 200 } }
                                        }

                                        ColumnLayout {
                                            id: crossfadeCardContent
                                            anchors.left: parent.left
                                            anchors.right: parent.right
                                            anchors.verticalCenter: parent.verticalCenter
                                            anchors.margins: 16
                                            spacing: 10

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 8

                                                ColumnLayout {
                                                    Layout.fillWidth: true
                                                    spacing: 4

                                                    Controls.Label {
                                                        text: "Crossfade"
                                                        font.bold: true
                                                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.95
                                                        color: root.textPrimary
                                                    }

                                                    Controls.Label {
                                                        Layout.fillWidth: true
                                                        text: player.crossfade_secs > 0
                                                              ? ("Blend tracks over " + player.crossfade_secs.toFixed(1) + " s using equal-power curves.")
                                                              : "Off — tracks play back-to-back with no overlap."
                                                        color: root.textDim
                                                        font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.82
                                                        wrapMode: Text.WordWrap
                                                    }
                                                }

                                                Controls.Label {
                                                    text: player.crossfade_secs > 0
                                                          ? (player.crossfade_secs.toFixed(1) + " s")
                                                          : "Off"
                                                    color: player.crossfade_secs > 0 ? root.accentColor : root.textDim
                                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.88
                                                    font.bold: player.crossfade_secs > 0

                                                    Behavior on color { ColorAnimation { duration: 150 } }
                                                }
                                            }

                                            Controls.Slider {
                                                id: crossfadeSlider
                                                Layout.fillWidth: true
                                                from: 0.0
                                                to: 12.0
                                                stepSize: 0.5
                                                value: player.crossfade_secs

                                                onMoved: player.setCrossfade(value)

                                                Controls.ToolTip {
                                                    parent: crossfadeSlider.handle
                                                    visible: crossfadeSlider.pressed
                                                    text: crossfadeSlider.value > 0
                                                          ? (crossfadeSlider.value.toFixed(1) + " s")
                                                          : "Off"
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Bottom spacer
                            Item { implicitHeight: 32 }
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
                                color: Qt.rgba(0, 0, 0, 0.03)
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
                                    font.letterSpacing: 1.0
                                    font.weight: Font.Medium
                                }

                                Controls.Label {
                                    text: "Tracks"
                                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.70
                                    color: root.textFaint
                                    font.capitalization: Font.AllUppercase
                                    font.letterSpacing: 1.0
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

                // Base color — light panel; ambient wash sits on top
                color: "#ffffff"

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

                // ── Ambient album-art wash ───────────────────────────────────
                // Soft light wash: a pale accent tint at the top fading to white,
                // letting the cover's colour gently colour the panel.
                Item {
                    id: ambientBackdrop
                    anchors.fill: parent
                    clip: true
                    opacity: (player.current_cover_thumb || "").length > 0 ? 1.0 : 0.0

                    Behavior on opacity { NumberAnimation { duration: 600; easing.type: Easing.InOutCubic } }

                    Rectangle {
                        anchors.fill: parent
                        gradient: Gradient {
                            GradientStop { position: 0.0; color: Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.16) }
                            GradientStop { position: 0.5; color: Qt.rgba(root.accentColor.r, root.accentColor.g, root.accentColor.b, 0.05) }
                            GradientStop { position: 1.0; color: "#ffffff" }
                        }
                    }
                }

                // ── Idle background (shown when no track) ─────────────────────
                Rectangle {
                    anchors.fill: parent
                    opacity: (player.current_cover_thumb || "").length > 0 ? 0.0 : 1.0
                    color: "#ffffff"

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
                            opacity: 0.14
                        }
                        // Middle shadow
                        Rectangle {
                            anchors.centerIn: parent
                            width: parent.width - 4
                            height: parent.height - 4
                            anchors.verticalCenterOffset: 10
                            radius: 18
                            color: "#000000"
                            opacity: 0.10
                        }

                        Rectangle {
                            id: npCoverFrame
                            anchors.fill: parent
                            radius: 16
                            color: Qt.rgba(0, 0, 0, 0.05)
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
                                color: Qt.rgba(0, 0, 0, 0.05)

                                // Abstract music note icon
                                Kirigami.Icon {
                                    anchors.centerIn: parent
                                    source: "media-optical-audio"
                                    width: 72
                                    height: 72
                                    color: root.textFaint
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
                                    GradientStop { position: 0.0; color: Qt.rgba(1, 1, 1, 0.18) }
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
                                   : Qt.rgba(0, 0, 0, 0.05)
                        }

                        Controls.Label {
                            id: stateBadge
                            anchors.left: stateBadgeBg.left
                            anchors.leftMargin: 8
                            anchors.verticalCenter: stateBadgeBg.verticalCenter
                            text: player.state_text || "Stopped"
                            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.73
                            font.capitalization: Font.AllUppercase
                            font.letterSpacing: 0.9
                            color: (player.state_text === "Playing")
                                   ? root.accentColor
                                   : root.textDim
                            font.weight: Font.Medium
                        }
                    }

                    // ── Spectrum visualizer ──────────────────────────────────
                    // 24 logarithmically-spaced frequency bars driven by the
                    // real-time FFT analyzer.  Heights animate smoothly.
                    // Shown when a track is loaded; rests at zero when stopped.
                    Item {
                        Layout.fillWidth: true
                        height: 44
                        opacity: (player.current_title || "").length > 0 ? 1.0 : 0.0

                        Behavior on opacity { NumberAnimation { duration: 400 } }

                        // Gradient glow layer behind bars
                        Rectangle {
                            anchors.fill: parent
                            gradient: Gradient {
                                GradientStop { position: 0.0; color: "transparent" }
                                GradientStop {
                                    position: 1.0
                                    color: Qt.rgba(
                                        root.accentColor.r,
                                        root.accentColor.g,
                                        root.accentColor.b,
                                        0.06
                                    )
                                }
                            }
                            radius: 4
                        }

                        Row {
                            anchors.fill: parent
                            anchors.leftMargin: 2
                            anchors.rightMargin: 2
                            spacing: 2

                            Repeater {
                                model: 24

                                delegate: Item {
                                    required property int index
                                    width: (parent.width - 46) / 24  // 46 = 23 gaps * 2
                                    height: parent.height

                                    property real barLevel: {
                                        var lvls = root.spectrumLevels
                                        if (!lvls || lvls.length <= index) return 0
                                        return lvls[index] || 0
                                    }

                                    property real barHeight: barLevel * (parent.height - 4)

                                    Behavior on barHeight {
                                        NumberAnimation {
                                            duration: 80
                                            easing.type: Easing.OutCubic
                                        }
                                    }

                                    // The bar itself — anchored to the bottom
                                    Rectangle {
                                        anchors.bottom: parent.bottom
                                        anchors.bottomMargin: 2
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        width: parent.width
                                        height: Math.max(2, parent.barHeight)
                                        radius: Math.min(width, 3)

                                        gradient: Gradient {
                                            GradientStop {
                                                position: 0.0
                                                color: Qt.rgba(
                                                    root.accentColor.r,
                                                    root.accentColor.g,
                                                    root.accentColor.b,
                                                    0.55 + parent.parent.barLevel * 0.45
                                                )
                                            }
                                            GradientStop {
                                                position: 1.0
                                                color: Qt.rgba(
                                                    root.accentColor.r,
                                                    root.accentColor.g,
                                                    root.accentColor.b,
                                                    0.85
                                                )
                                            }
                                        }
                                    }
                                }
                            }
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
                                    color: Qt.rgba(0, 0, 0, 0.05)
                                    clip: true

                                    // Shadow
                                    Rectangle {
                                        anchors.centerIn: parent
                                        width: parent.width + 4
                                        height: parent.height + 4
                                        anchors.verticalCenterOffset: 3
                                        radius: 9
                                        color: "#000000"
                                        opacity: 0.10
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
                                        color: root.textFaint
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
                                color: "transparent"
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
