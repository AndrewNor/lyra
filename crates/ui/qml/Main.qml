// Proves the QML <-> Rust seam: a Q_PROPERTY in a header, a Rust list driving
// a ListView, an invokable delegating to lyra-core, and a background thread.
import QtQuick
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import ai.drivee.lyra

Kirigami.ApplicationWindow {
    id: root
    title: "Lyra (Phase 0 spike)"
    width: 520
    height: 680

    LibraryController { id: controller }

    pageStack.initialPage: Kirigami.ScrollablePage {
        title: "Library"

        // Toolbar button -> background thread -> appends a track ~0.5s later.
        actions: [
            Kirigami.Action {
                text: "Simulate scan"
                icon.name: "media-playback-start"
                onTriggered: controller.simulateScan()
            }
        ]

        // ScrollablePage takes ONE flickable child: the ListView.
        ListView {
            model: controller.tracks   // QStringList -> use `modelData`
            header: Controls.Label {
                width: ListView.view ? ListView.view.width : implicitWidth
                padding: Kirigami.Units.largeSpacing
                font.bold: true
                text: controller.greeting + "   ·   nextIndex() = " + controller.nextIndex()
            }
            delegate: Controls.ItemDelegate {
                width: ListView.view.width
                text: modelData
            }
        }
    }
}
