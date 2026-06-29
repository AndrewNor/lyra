//! Player QObject — engine transport (play / pause / resume / stop).
//!
//! # Threading
//! `lyra_engine::Engine` is `!Send` (it holds a `cpal::Stream`).  It is
//! created and used exclusively on the Qt thread, never moved elsewhere.
//! The backing struct holds `engine: Option<Engine>` and initialises it
//! lazily on the first `play()` call.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, state_text)]
        #[qproperty(QString, current_title)]
        #[qproperty(QString, current_artist)]
        type Player = super::PlayerRust;

        /// Start playback of `path`.  Lazily opens the audio device on first call.
        #[qinvokable]
        fn play(self: Pin<&mut Player>, path: QString, title: QString, artist: QString);

        /// Pause current playback.
        #[qinvokable]
        fn pause(self: Pin<&mut Player>);

        /// Resume a paused track.
        #[qinvokable]
        fn resume(self: Pin<&mut Player>);

        /// Stop playback and release resources.
        #[qinvokable]
        fn stop(self: Pin<&mut Player>);
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use lyra_engine::Engine;

// ── Backing struct ───────────────────────────────────────────────────────────

pub struct PlayerRust {
    state_text: QString,
    current_title: QString,
    current_artist: QString,

    /// Lazily initialised on first `play()`.  `!Send` — Qt thread only.
    engine: Option<Engine>,
}

impl Default for PlayerRust {
    fn default() -> Self {
        Self {
            state_text: QString::from("Stopped"),
            current_title: QString::from(""),
            current_artist: QString::from(""),
            engine: None,
        }
    }
}

// ── QObject impl ─────────────────────────────────────────────────────────────

impl qobject::Player {
    fn play(mut self: Pin<&mut Self>, path: QString, title: QString, artist: QString) {
        // Lazily create the engine on first play.
        // Each unsafe block is a narrow scope so NLL ends the borrow before
        // we call any set_* method (which re-borrows self via Pin).

        // ── step 1: lazy init ────────────────────────────────────────────────
        let needs_init = {
            // SAFETY: we only read a bool field; no move out of Pin.
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.engine.is_none()
        };

        if needs_init {
            match Engine::new() {
                Ok(e) => {
                    // SAFETY: same — assign into engine field; not moving Self.
                    unsafe { self.as_mut().rust_mut().get_unchecked_mut() }.engine = Some(e);
                }
                Err(e) => {
                    let msg = format!("Engine init error: {e}");
                    self.as_mut().set_state_text(QString::from(msg.as_str()));
                    return;
                }
            }
        }

        // ── step 2: play ─────────────────────────────────────────────────────
        let path_str = path.to_string();
        let file_path = std::path::Path::new(&path_str);

        let play_result: Option<lyra_engine::Result<()>> = {
            // SAFETY: accessing engine field; borrow ends before step 3.
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.engine.as_mut().map(|e| e.play(file_path))
        };

        // ── step 3: update properties ─────────────────────────────────────────
        match play_result {
            Some(Ok(())) => {
                self.as_mut().set_current_title(title);
                self.as_mut().set_current_artist(artist);
                self.as_mut().set_state_text(QString::from("Playing"));
            }
            Some(Err(e)) => {
                let msg = format!("Play error: {e}");
                self.as_mut().set_state_text(QString::from(msg.as_str()));
            }
            None => {
                self.as_mut()
                    .set_state_text(QString::from("Internal error: no engine"));
            }
        }
    }

    fn pause(mut self: Pin<&mut Self>) {
        // SAFETY: accessing engine field; borrow ends before set_*.
        let did_pause = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            if let Some(e) = r.engine.as_mut() {
                e.pause();
                true
            } else {
                false
            }
        };
        if did_pause {
            self.as_mut().set_state_text(QString::from("Paused"));
        }
    }

    fn resume(mut self: Pin<&mut Self>) {
        // SAFETY: accessing engine field; borrow ends before set_*.
        let did_resume = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            if let Some(e) = r.engine.as_mut() {
                e.resume();
                true
            } else {
                false
            }
        };
        if did_resume {
            self.as_mut().set_state_text(QString::from("Playing"));
        }
    }

    fn stop(mut self: Pin<&mut Self>) {
        // SAFETY: accessing engine field; borrow ends before set_*.
        {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            if let Some(e) = r.engine.as_mut() {
                e.stop();
            }
        }
        self.as_mut().set_state_text(QString::from("Stopped"));
        self.as_mut().set_current_title(QString::from(""));
        self.as_mut().set_current_artist(QString::from(""));
    }
}
