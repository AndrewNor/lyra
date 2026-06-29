// SidebarItem.qml — a single navigation row in the left sidebar.
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

    width: parent ? parent.width : 200
    height: 38

    // ── Active pill background ──────────────────────────────────────────────
    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.smallSpacing
        anchors.rightMargin: Kirigami.Units.smallSpacing
        anchors.topMargin: 2
        anchors.bottomMargin: 2
        radius: Kirigami.Units.smallSpacing
        color: {
            var hc = Kirigami.Theme.highlightColor
            var tc = Kirigami.Theme.textColor
            if (root.active && hc) return Qt.rgba(hc.r, hc.g, hc.b, 0.15)
            if (hoverArea.containsMouse && root.enabled && tc)
                return Qt.rgba(tc.r, tc.g, tc.b, 0.06)
            return "transparent"
        }

        Behavior on color { ColorAnimation { duration: 120 } }
    }

    // ── Left accent bar for active state ───────────────────────────────────
    Rectangle {
        anchors.left: parent.left
        anchors.leftMargin: 0
        anchors.top: parent.top
        anchors.topMargin: 5
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 5
        width: 3
        radius: 1.5
        color: Kirigami.Theme.highlightColor || "#3daee9"
        opacity: root.active ? 1.0 : 0.0

        Behavior on opacity { NumberAnimation { duration: 150 } }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing
        anchors.rightMargin: Kirigami.Units.smallSpacing
        spacing: Kirigami.Units.smallSpacing + 2

        Kirigami.Icon {
            source: root.iconName
            width: 16
            height: 16
            color: root.active
                   ? (Kirigami.Theme.highlightColor || "#3daee9")
                   : (Kirigami.Theme.textColor || "#000000")
            opacity: root.enabled ? 1.0 : 0.38

            Behavior on color { ColorAnimation { duration: 150 } }
        }

        Controls.Label {
            Layout.fillWidth: true
            text: root.label
            elide: Text.ElideRight
            color: root.active
                   ? (Kirigami.Theme.highlightColor || "#3daee9")
                   : (Kirigami.Theme.textColor || "#000000")
            opacity: root.enabled ? 1.0 : 0.38
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.93
            font.weight: root.active ? Font.Medium : Font.Normal

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
