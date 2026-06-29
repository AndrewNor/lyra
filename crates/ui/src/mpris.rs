//! MPRIS2 server for Lyra — exposes `org.mpris.MediaPlayer2.Lyra` on the
//! session D-Bus so Plasma's media controls, the lock screen, and keyboard
//! media keys can control playback and display the current track.
//!
//! # Threading
//! ```text
//! Qt thread                MPRIS thread (async_io executor)
//! ---------                ----------------------------------------
//! PlayerRust               LyraPlayer { Arc<Mutex<MprisState>>, CxxQtThread<Player> }
//!   mpris_handle ──────►   Server::new("Lyra", player).await
//!   (Arc<Mutex<State>>)         ↑ reads MprisState for property queries
//!   (Sender<()>)       ─────►  recv trigger → server.properties_changed(...)
//!
//! Incoming D-Bus control methods (Play/Pause/Next…) call
//!   qt_thread.queue(|p| p.as_mut().resume() / .pause() / …)
//! which marshals the call safely back to the Qt thread.
//! ```

use std::sync::{Arc, Mutex};

use async_channel::{Receiver, Sender};
use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, Property, Server, Time, TrackId,
    zbus::fdo,
};

use crate::player::qobject::Player;

// ── Shared state ─────────────────────────────────────────────────────────────

/// Playback state mirrored from the Qt thread so the MPRIS thread can answer
/// property queries without crossing thread boundaries.
#[derive(Clone, Debug)]
pub struct MprisState {
    pub status: PlaybackStatus,
    pub title: String,
    pub artist: String,
    /// Local filesystem path to the cover thumbnail (no `file://` prefix).
    pub cover_path: String,
    /// Track duration in microseconds.
    pub duration_us: i64,
    /// Playback position in microseconds.
    pub position_us: i64,
}

impl Default for MprisState {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            title: String::new(),
            artist: String::new(),
            cover_path: String::new(),
            duration_us: 0,
            position_us: 0,
        }
    }
}

// ── Handle returned to the Qt-thread Player ──────────────────────────────────

/// Owned by `PlayerRust`; lets the Qt thread push state updates and trigger
/// D-Bus `PropertiesChanged` emissions from the MPRIS thread.
pub struct MprisHandle {
    state: Arc<Mutex<MprisState>>,
    /// Sending `()` wakes the MPRIS loop to emit PropertiesChanged.
    trigger: Sender<()>,
}

impl MprisHandle {
    /// Update the shared MPRIS state and trigger a PropertiesChanged emission.
    ///
    /// Safe to call from the Qt thread.  Errors are silently ignored so a
    /// D-Bus failure never brings down the UI.
    pub fn update(&self, new_state: MprisState) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = new_state;
        }
        // Fire-and-forget: if the channel is full or closed the ping is dropped;
        // the next update will carry the latest state anyway.
        let _ = self.trigger.try_send(());
    }
}

// ── MPRIS implementation ──────────────────────────────────────────────────────

/// Implements both `RootInterface` and `PlayerInterface` for mpris-server 0.10.
///
/// Property queries read from the shared `Arc<Mutex<MprisState>>`.
/// Control methods queue closures onto the Qt event loop via `CxxQtThread`.
struct LyraPlayer {
    state: Arc<Mutex<MprisState>>,
    qt_thread: cxx_qt::CxxQtThread<Player>,
}

// mpris-server 0.10 generates both `RootInterface` and `PlayerInterface`
// as `Send + Sync` async traits.  `LyraPlayer` satisfies that because:
//   • `Arc<Mutex<MprisState>>` — Send + Sync
//   • `CxxQtThread<Player>`   — Send + Sync (as per cxx-qt guarantee)

impl mpris_server::RootInterface for LyraPlayer {
    async fn raise(&self) -> fdo::Result<()> {
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> mpris_server::zbus::Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("Lyra".to_owned())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok(String::new())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }
}

impl mpris_server::PlayerInterface for LyraPlayer {
    // ── Controls ──────────────────────────────────────────────────────────────

    async fn next(&self) -> fdo::Result<()> {
        let _ = self.qt_thread.queue(|mut p| {
            p.as_mut().next();
        });
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        let _ = self.qt_thread.queue(|mut p| {
            p.as_mut().prev();
        });
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        let _ = self.qt_thread.queue(|mut p| {
            p.as_mut().pause();
        });
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        // Read status without holding the lock across the queue call.
        let status = self
            .state
            .lock()
            .map(|g| g.status)
            .unwrap_or(PlaybackStatus::Stopped);
        match status {
            PlaybackStatus::Playing => {
                let _ = self.qt_thread.queue(|mut p| {
                    p.as_mut().pause();
                });
            }
            PlaybackStatus::Paused => {
                let _ = self.qt_thread.queue(|mut p| {
                    p.as_mut().resume();
                });
            }
            PlaybackStatus::Stopped => {
                // Nothing loaded — no-op (media key while stopped).
            }
        }
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        let _ = self.qt_thread.queue(|mut p| {
            p.as_mut().stop();
        });
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        let _ = self.qt_thread.queue(|mut p| {
            p.as_mut().resume();
        });
        Ok(())
    }

    async fn seek(&self, _offset: Time) -> fdo::Result<()> {
        // Seek deferred to a later phase.
        Ok(())
    }

    async fn set_position(&self, _track_id: TrackId, _position: Time) -> fdo::Result<()> {
        Ok(())
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Ok(())
    }

    // ── Properties ────────────────────────────────────────────────────────────

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(self
            .state
            .lock()
            .map(|g| g.status)
            .unwrap_or(PlaybackStatus::Stopped))
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> mpris_server::zbus::Result<()> {
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> mpris_server::zbus::Result<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> mpris_server::zbus::Result<()> {
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        let st = self.state.lock().map(|g| g.clone()).unwrap_or_default();
        Ok(build_metadata(&st))
    }

    async fn volume(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }

    async fn set_volume(&self, _volume: f64) -> mpris_server::zbus::Result<()> {
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(self
            .state
            .lock()
            .map(|g| Time::from_micros(g.position_us))
            .unwrap_or(Time::ZERO))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

// ── Server startup ────────────────────────────────────────────────────────────

/// Spawn the MPRIS server on a dedicated thread and return the handle used by
/// the Qt-thread `PlayerRust` to push state updates.
///
/// Returns `None` if the D-Bus connection cannot be established (e.g. running
/// headless without a session bus), ensuring the caller always stays
/// operational.
pub fn start(qt: cxx_qt::CxxQtThread<Player>) -> Option<MprisHandle> {
    let state = Arc::new(Mutex::new(MprisState::default()));
    // Bounded channel of depth 1: we only need to know "something changed".
    let (tx, rx): (Sender<()>, Receiver<()>) = async_channel::bounded(1);

    let handle = MprisHandle {
        state: Arc::clone(&state),
        trigger: tx,
    };

    std::thread::Builder::new()
        .name("lyra-mpris".to_owned())
        .spawn(move || {
            run_mpris_server(state, rx, qt);
        })
        .ok()?;

    Some(handle)
}

/// Blocking MPRIS event loop running on the dedicated MPRIS thread.
fn run_mpris_server(
    state: Arc<Mutex<MprisState>>,
    rx: Receiver<()>,
    qt: cxx_qt::CxxQtThread<Player>,
) {
    let player_impl = LyraPlayer {
        state: Arc::clone(&state),
        qt_thread: qt,
    };

    // `async_io::block_on` drives the zbus async executor on this thread.
    async_io::block_on(async move {
        let server = match Server::new("Lyra", player_impl).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[lyra-mpris] Failed to connect to D-Bus: {e}");
                return;
            }
        };

        // Wait for state-change pings from the Qt thread, then emit
        // PropertiesChanged for the properties that clients care about.
        loop {
            match rx.recv().await {
                Ok(()) => {
                    let st = state.lock().map(|g| g.clone()).unwrap_or_default();
                    let props = vec![
                        Property::PlaybackStatus(st.status),
                        Property::Metadata(build_metadata(&st)),
                    ];
                    if let Err(e) = server.properties_changed(props).await {
                        eprintln!("[lyra-mpris] properties_changed error: {e}");
                    }
                }
                Err(_) => {
                    // Channel closed — Qt object was destroyed; exit cleanly.
                    break;
                }
            }
        }
    });
}

/// Build `mpris_server::Metadata` from a state snapshot.
fn build_metadata(state: &MprisState) -> Metadata {
    let mut builder = Metadata::builder().trackid(
        TrackId::try_from("/ai/drivee/lyra/NoTrack").unwrap_or(TrackId::NO_TRACK),
    );

    if !state.title.is_empty() {
        builder = builder.title(state.title.clone());
    }
    if !state.artist.is_empty() {
        builder = builder.artist(vec![state.artist.clone()]);
    }
    if state.duration_us > 0 {
        builder = builder.length(Time::from_micros(state.duration_us));
    }
    if !state.cover_path.is_empty() {
        builder = builder.art_url(format!("file://{}", state.cover_path));
    }

    builder.build()
}
