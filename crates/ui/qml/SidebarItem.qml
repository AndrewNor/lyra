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
    height: 36

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 4
        anchors.rightMargin: 4
        radius: 4
        color: {
            var hc = Kirigami.Theme.highlightColor
            var tc = Kirigami.Theme.textColor
            if (root.active && hc) return hc
            if (hoverArea.containsMouse && root.enabled && tc)
                return Qt.rgba(tc.r, tc.g, tc.b, 0.07)
            return "transparent"
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Kirigami.Units.largeSpacing - 4
            anchors.rightMargin: Kirigami.Units.largeSpacing - 4
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Icon {
                source: root.iconName
                width: 18
                height: 18
                color: root.active
                       ? (Kirigami.Theme.highlightedTextColor || "#ffffff")
                       : (Kirigami.Theme.textColor || "#000000")
                opacity: root.enabled ? 1.0 : 0.45
            }

            Controls.Label {
                Layout.fillWidth: true
                text: root.label
                elide: Text.ElideRight
                color: root.active
                       ? (Kirigami.Theme.highlightedTextColor || "#ffffff")
                       : (Kirigami.Theme.textColor || "#000000")
                opacity: root.enabled ? 1.0 : 0.45
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 0.92
            }
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
