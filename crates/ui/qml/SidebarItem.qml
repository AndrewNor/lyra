// SidebarItem.qml — premium sidebar navigation row (Apple Music / Spotify tier)
// Signals: activated() when clicked and enabled.

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Item {
    id: root

    property string iconName: ""
    property string label: ""
    property bool active: false

    signal activated()

    // Used inside ColumnLayouts: a layout ignores an explicit `width`, so fill
    // via the layout instead (otherwise the row collapses to width 0 — full
    // height but no hit area, and clicks silently miss).
    Layout.fillWidth: true
    implicitWidth: 200
    height: 40

    // ── Active pill — accent gradient fill ─────────────────────────────────
    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 6
        anchors.topMargin: 2
        anchors.bottomMargin: 2
        radius: 8
        opacity: root.active ? 1.0 : (hoverArea.containsMouse && root.enabled ? 1.0 : 0.0)

        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop {
                position: 0.0
                color: root.active
                       ? Qt.rgba(
                             (Kirigami.Theme.highlightColor || "#3daee9").r,
                             (Kirigami.Theme.highlightColor || "#3daee9").g,
                             (Kirigami.Theme.highlightColor || "#3daee9").b,
                             0.22
                         )
                       : Qt.rgba(1, 1, 1, 0.05)
            }
            GradientStop {
                position: 1.0
                color: "transparent"
            }
        }

        Behavior on opacity { NumberAnimation { duration: 140 } }
    }

    // ── Left accent bar ────────────────────────────────────────────────────
    Rectangle {
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.topMargin: 7
        anchors.bottomMargin: 7
        width: 3
        radius: 2

        // Glow using layered rectangles (no MultiEffect needed here)
        color: Kirigami.Theme.highlightColor || "#3daee9"
        opacity: root.active ? 1.0 : 0.0

        // Soft outer glow
        Rectangle {
            anchors.centerIn: parent
            width: parent.width + 6
            height: parent.height + 4
            radius: parent.radius + 3
            color: Kirigami.Theme.highlightColor || "#3daee9"
            opacity: 0.30
            z: -1
        }

        Behavior on opacity { NumberAnimation { duration: 160 } }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing + 2
        anchors.rightMargin: Kirigami.Units.smallSpacing
        spacing: Kirigami.Units.smallSpacing + 3

        Kirigami.Icon {
            source: root.iconName
            width: 16
            height: 16
            color: root.active
                   ? (Kirigami.Theme.highlightColor || "#3daee9")
                   : Qt.rgba(1, 1, 1, 0.60)
            opacity: root.enabled ? 1.0 : 0.30

            Behavior on color { ColorAnimation { duration: 150 } }
        }

        Controls.Label {
            Layout.fillWidth: true
            text: root.label
            elide: Text.ElideRight
            color: root.active
                   ? (Kirigami.Theme.highlightColor || "#3daee9")
                   : Qt.rgba(1, 1, 1, 0.75)
            opacity: root.enabled ? 1.0 : 0.30
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.92
            font.weight: root.active ? Font.SemiBold : Font.Normal
            font.letterSpacing: root.active ? 0.2 : 0.0

            Behavior on color { ColorAnimation { duration: 150 } }
        }
    }

    MouseArea {
        id: hoverArea
        anchors.fill: parent
        hoverEnabled: root.enabled
        cursorShape: root.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: {
            if (root.enabled)
                root.activated()
        }
    }
}
