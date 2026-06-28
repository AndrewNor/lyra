//! CXX-Qt 0.8.1 bridge. The QObject is declared in `extern "RustQt"`; the
//! backing struct lives OUTSIDE the bridge and is referenced via `super::`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, greeting)]
        #[qproperty(QStringList, tracks)]
        #[qproperty(i32, current)]
        #[qproperty(i32, len)]
        type LibraryController = super::LibraryControllerRust;

        // Delegates to the pure lyra-core logic. -1 means "stop" (None).
        #[qinvokable]
        #[cxx_name = "nextIndex"]
        fn next_index(self: &LibraryController) -> i32;

        // Background work marshalled back to the Qt thread (future scan demo).
        #[qinvokable]
        #[cxx_name = "simulateScan"]
        fn simulate_scan(self: Pin<&mut LibraryController>);
    }

    impl cxx_qt::Threading for LibraryController {}
}

use core::pin::Pin;
use cxx_qt::Threading;
use cxx_qt_lib::{QString, QStringList};
use lyra_core::{next_index as core_next_index, RepeatMode};

pub struct LibraryControllerRust {
    greeting: QString,
    tracks: QStringList,
    current: i32,
    len: i32,
}

impl Default for LibraryControllerRust {
    fn default() -> Self {
        let mut tracks = QStringList::default();
        tracks.append(QString::from("Boards of Canada — Roygbiv"));
        tracks.append(QString::from("Aphex Twin — Avril 14th"));
        tracks.append(QString::from("Tycho — Awake"));
        Self {
            greeting: QString::from("Welcome to Lyra"),
            tracks,
            current: 0,
            len: 3,
        }
    }
}

// Methods are implemented on the GENERATED type, not on the Rust struct.
impl qobject::LibraryController {
    fn next_index(&self) -> i32 {
        // qproperty getters return &T; deref to read. If the compiler says a
        // getter already returns i32 by value, drop the `*`.
        let cur = (*self.current()).max(0) as usize;
        let len = (*self.len()).max(0) as usize;
        match core_next_index(cur, len, RepeatMode::All) {
            Some(i) => i as i32,
            None => -1,
        }
    }

    fn simulate_scan(self: Pin<&mut Self>) {
        let thread = self.qt_thread(); // Send+Sync handle, from Threading impl
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            thread
                .queue(|mut qobject: Pin<&mut qobject::LibraryController>| {
                    let mut list = qobject.tracks().clone();
                    list.append(QString::from("Scanned — New Track"));
                    qobject.as_mut().set_tracks(list);
                })
                .expect("failed to queue onto Qt thread");
        });
    }
}
