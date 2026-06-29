use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    // 0.8.1 API: CxxQtBuilder::new_qml_module with builder-pattern QmlModule.
    // URI must match CMakeLists.txt and Main.qml byte-for-byte.
    CxxQtBuilder::new_qml_module(
        QmlModule::new("ai.drivee.lyra")
            .qml_files([
                "qml/Main.qml",
                "qml/SidebarItem.qml",
                "qml/TrackDelegate.qml",
                "qml/AlbumCard.qml",
                "qml/ArtistRow.qml",
            ]),
    )
    .file("src/bridge.rs")
    .file("src/library.rs")
    .file("src/player.rs")
    .qt_module("Gui")
    .qt_module("Quick")
    .build();
}
