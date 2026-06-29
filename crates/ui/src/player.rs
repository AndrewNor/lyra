//! Player QObject — engine transport (play / pause / resume / stop) +
//! play-queue with now-playing cover and Up Next.
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
        #[qproperty(QString, current_cover_thumb)]
        #[qproperty(QString, queue_json)]
        /// Lyrics for the current track as JSON: `{"synced":bool,"lines":[{"t":number|null,"text":string}]}`.
        #[qproperty(QString, lyrics_json)]
        /// Current playback position in seconds (updated by `refreshPosition`).
        #[qproperty(f64, position_secs)]
        /// Total duration of the current track in seconds (set when a track starts).
        #[qproperty(f64, duration_secs)]
        type Player = super::PlayerRust;

        /// Initialise the MPRIS2 D-Bus server.  Call once from QML
        /// `Component.onCompleted`.
        #[qinvokable]
        #[cxx_name = "initMpris"]
        fn init_mpris(self: Pin<&mut Player>);

        /// Start playback of `path`.  Lazily opens the audio device on first call.
        #[qinvokable]
        fn play(self: Pin<&mut Player>, path: QString, title: QString, artist: QString);

        /// Play from a Library results JSON array at the given index.
        /// Parses the JSON, loads all track ids into the queue, jumps to
        /// `index`, and starts playback of that track.
        #[qinvokable]
        #[cxx_name = "playFromList"]
        fn play_from_list(self: Pin<&mut Player>, results_json: QString, index: i32);

        /// Advance the queue and play the next track.
        #[qinvokable]
        #[cxx_name = "next"]
        fn next(self: Pin<&mut Player>);

        /// Step back in the queue and play the previous track.
        #[qinvokable]
        #[cxx_name = "prev"]
        fn prev(self: Pin<&mut Player>);

        /// Pause current playback.
        #[qinvokable]
        fn pause(self: Pin<&mut Player>);

        /// Resume a paused track.
        #[qinvokable]
        fn resume(self: Pin<&mut Player>);

        /// Stop playback and release resources.
        #[qinvokable]
        fn stop(self: Pin<&mut Player>);

        /// Poll the engine for the current position and update `position_secs`.
        /// Call this from a QML `Timer` (e.g. every 250 ms while playing).
        #[qinvokable]
        #[cxx_name = "refreshPosition"]
        fn refresh_position(self: Pin<&mut Player>);

        /// Seek to a fractional position (0.0 = start, 1.0 = end).
        /// Best-effort: if the engine seek is a no-op the call is silently ignored.
        #[qinvokable]
        #[cxx_name = "seek"]
        fn seek(self: Pin<&mut Player>, fraction: f64);
    }

    impl cxx_qt::Threading for Player {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use lyra_core::PlayQueue;
use lyra_engine::Engine;
use mpris_server::PlaybackStatus;

use crate::mpris::{MprisHandle, MprisState};

// ── Row type for the cached playlist ────────────────────────────────────────

#[derive(Clone)]
struct TrackRow {
    id: i64,
    title: String,
    artist: String,
    path: String,
    cover_thumb: String,
    duration_ms: u64,
}

// ── Backing struct ───────────────────────────────────────────────────────────

pub struct PlayerRust {
    state_text: QString,
    current_title: QString,
    current_artist: QString,
    current_cover_thumb: QString,
    queue_json: QString,
    /// Lyrics for the current track, serialised as JSON.
    lyrics_json: QString,

    /// Current playback position in seconds (polled from the engine).
    position_secs: f64,

    /// Duration of the current track in seconds (set when a track starts).
    duration_secs: f64,

    /// Lazily initialised on first `play()`.  `!Send` — Qt thread only.
    engine: Option<Engine>,

    /// Play-queue holding track ids.
    play_queue: PlayQueue,

    /// Cached playlist parsed from the last `play_from_list` call.
    playlist: Vec<TrackRow>,

    /// MPRIS2 server handle — initialised once from QML via `initMpris`.
    mpris_handle: Option<MprisHandle>,
}

const EMPTY_LYRICS_JSON: &str = r#"{"synced":false,"lines":[]}"#;

impl Default for PlayerRust {
    fn default() -> Self {
        Self {
            state_text: QString::from("Stopped"),
            current_title: QString::from(""),
            current_artist: QString::from(""),
            current_cover_thumb: QString::from(""),
            queue_json: QString::from("[]"),
            lyrics_json: QString::from(EMPTY_LYRICS_JSON),
            position_secs: 0.0,
            duration_secs: 0.0,
            engine: None,
            play_queue: PlayQueue::new(),
            playlist: Vec::new(),
            mpris_handle: None,
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Parse a Library results JSON array into a Vec of TrackRow.
fn parse_playlist(json: &str) -> Vec<TrackRow> {
    let Ok(arr) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(arr) = arr.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| {
            let id = v["id"].as_i64()?;
            let title = v["title"].as_str().unwrap_or("").to_owned();
            let artist = v["artist"].as_str().unwrap_or("").to_owned();
            let path = v["path"].as_str().unwrap_or("").to_owned();
            let cover_thumb = v["cover_thumb"].as_str().unwrap_or("").to_owned();
            let duration_ms = v["durationMs"].as_u64().unwrap_or(0);
            if path.is_empty() {
                return None;
            }
            Some(TrackRow { id, title, artist, path, cover_thumb, duration_ms })
        })
        .collect()
}

/// Resolve a track id to its row in the cached playlist.
fn find_row<'a>(playlist: &'a [TrackRow], id: i64) -> Option<&'a TrackRow> {
    playlist.iter().find(|r| r.id == id)
}

/// Build the queue_json: the rows AFTER the current position in the playlist.
/// Uses the play-queue's current id to determine the current position.
fn build_queue_json(playlist: &[TrackRow], current_id: Option<i64>) -> String {
    let start = match current_id {
        None => return "[]".to_owned(),
        Some(id) => {
            match playlist.iter().position(|r| r.id == id) {
                None => return "[]".to_owned(),
                Some(pos) => pos + 1,
            }
        }
    };
    let upcoming: Vec<serde_json::Value> = playlist[start..]
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "artist": r.artist,
                "path": r.path,
                "cover_thumb": r.cover_thumb,
            })
        })
        .collect();
    serde_json::to_string(&upcoming).unwrap_or_else(|_| "[]".to_owned())
}

// ── QObject impl ─────────────────────────────────────────────────────────────

impl qobject::Player {
    // ── MPRIS helpers ─────────────────────────────────────────────────────────

    /// Initialise the MPRIS2 D-Bus server once from QML `Component.onCompleted`.
    fn init_mpris(mut self: Pin<&mut Self>) {
        let qt = self.as_mut().qt_thread();
        let handle = crate::mpris::start(qt);
        unsafe { self.as_mut().rust_mut().get_unchecked_mut() }.mpris_handle = handle;
    }

    /// Collect the current playback state and push it to the MPRIS thread.
    /// Silently does nothing if MPRIS was not initialised.
    fn push_mpris_state(self: Pin<&Self>) {
        let r = self.rust();
        let Some(ref handle) = r.mpris_handle else {
            return;
        };
        let status = {
            let st = r.state_text.to_string();
            match st.as_str() {
                "Playing" => PlaybackStatus::Playing,
                "Paused" => PlaybackStatus::Paused,
                _ => PlaybackStatus::Stopped,
            }
        };
        let state = MprisState {
            status,
            title: r.current_title.to_string(),
            artist: r.current_artist.to_string(),
            cover_path: r.current_cover_thumb.to_string(),
            duration_us: (r.duration_secs * 1_000_000.0) as i64,
            position_us: (r.position_secs * 1_000_000.0) as i64,
        };
        handle.update(state);
    }

    // ── Engine management ─────────────────────────────────────────────────────

    /// Ensure the engine exists, returning false and setting state_text on error.
    fn ensure_engine(mut self: Pin<&mut Self>) -> bool {
        let needs_init = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.engine.is_none()
        };

        if needs_init {
            match Engine::new() {
                Ok(e) => {
                    unsafe { self.as_mut().rust_mut().get_unchecked_mut() }.engine = Some(e);
                }
                Err(e) => {
                    let msg = format!("Engine init error: {e}");
                    self.as_mut().set_state_text(QString::from(msg.as_str()));
                    return false;
                }
            }
        }
        true
    }

    /// Load lyrics for `path` and set the `lyrics_json` property.
    /// On any error, sets the property to the empty-lyrics sentinel.
    fn load_lyrics(mut self: Pin<&mut Self>, path: &str) {
        let json = match lyra_metadata::read_lyrics(std::path::Path::new(path)) {
            Ok(lyrics) => serde_json::to_string(&lyrics).unwrap_or_else(|_| EMPTY_LYRICS_JSON.to_owned()),
            Err(_) => EMPTY_LYRICS_JSON.to_owned(),
        };
        self.as_mut().set_lyrics_json(QString::from(json.as_str()));
    }

    /// Internal: play a path string, update title/artist/cover/state/duration properties.
    fn play_row(
        mut self: Pin<&mut Self>,
        path: &str,
        title: &str,
        artist: &str,
        cover_thumb: &str,
        duration_ms: u64,
    ) {
        if !self.as_mut().ensure_engine() {
            return;
        }

        let file_path = std::path::Path::new(path);
        let play_result: Option<lyra_engine::Result<()>> = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.engine.as_mut().map(|e| e.play(file_path))
        };

        match play_result {
            Some(Ok(())) => {
                self.as_mut().set_current_title(QString::from(title));
                self.as_mut().set_current_artist(QString::from(artist));
                self.as_mut().set_current_cover_thumb(QString::from(cover_thumb));
                self.as_mut().set_state_text(QString::from("Playing"));
                // Reset position and set duration for the new track.
                self.as_mut().set_position_secs(0.0);
                self.as_mut().set_duration_secs(duration_ms as f64 / 1000.0);
                // Load lyrics for the new track (sidecar .lrc or embedded tag).
                self.as_mut().load_lyrics(path);
                // Notify MPRIS of the new track and Playing status.
                self.as_ref().push_mpris_state();
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

    fn play(mut self: Pin<&mut Self>, path: QString, title: QString, artist: QString) {
        let path_s = path.to_string();
        let title_s = title.to_string();
        let artist_s = artist.to_string();
        // Duration unknown when playing via the direct `play()` invokable.
        self.as_mut().play_row(&path_s, &title_s, &artist_s, "", 0);
    }

    fn play_from_list(mut self: Pin<&mut Self>, results_json: QString, index: i32) {
        let json_str = results_json.to_string();
        let playlist = parse_playlist(&json_str);

        if playlist.is_empty() {
            self.as_mut()
                .set_state_text(QString::from("Play error: empty playlist"));
            return;
        }

        let idx = index as usize;
        if idx >= playlist.len() {
            self.as_mut()
                .set_state_text(QString::from("Play error: index out of range"));
            return;
        }

        // Load the queue with all ids and jump to the requested index.
        let ids: Vec<i64> = playlist.iter().map(|r| r.id).collect();
        {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.playlist = playlist.clone();
            r.play_queue.set_items(ids);
            r.play_queue.jump_to(idx);
        }

        let row = playlist[idx].clone();
        let current_id = Some(row.id);
        let queue_json = build_queue_json(&playlist, current_id);
        self.as_mut()
            .set_queue_json(QString::from(queue_json.as_str()));

        let (path, title, artist, cover, dur_ms) = (
            row.path.clone(),
            row.title.clone(),
            row.artist.clone(),
            row.cover_thumb.clone(),
            row.duration_ms,
        );
        self.as_mut().play_row(&path, &title, &artist, &cover, dur_ms);
    }

    pub fn next(mut self: Pin<&mut Self>) {
        // Advance the queue and get the new id.
        let next_id = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.play_queue.next()
        };

        let Some(id) = next_id else {
            self.as_mut()
                .set_state_text(QString::from("End of queue"));
            return;
        };

        // Resolve id → row from cached playlist.
        let (path, title, artist, cover, dur_ms, queue_json) = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            let playlist = &r.playlist;
            let Some(row) = find_row(playlist, id) else {
                return;
            };
            let qj = build_queue_json(playlist, Some(id));
            (
                row.path.clone(),
                row.title.clone(),
                row.artist.clone(),
                row.cover_thumb.clone(),
                row.duration_ms,
                qj,
            )
        };

        self.as_mut()
            .set_queue_json(QString::from(queue_json.as_str()));
        self.as_mut().play_row(&path, &title, &artist, &cover, dur_ms);
    }

    pub fn prev(mut self: Pin<&mut Self>) {
        // Step back the queue and get the new id.
        let prev_id = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.play_queue.prev()
        };

        let Some(id) = prev_id else {
            self.as_mut()
                .set_state_text(QString::from("Beginning of queue"));
            return;
        };

        // Resolve id → row from cached playlist.
        let (path, title, artist, cover, dur_ms, queue_json) = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            let playlist = &r.playlist;
            let Some(row) = find_row(playlist, id) else {
                return;
            };
            let qj = build_queue_json(playlist, Some(id));
            (
                row.path.clone(),
                row.title.clone(),
                row.artist.clone(),
                row.cover_thumb.clone(),
                row.duration_ms,
                qj,
            )
        };

        self.as_mut()
            .set_queue_json(QString::from(queue_json.as_str()));
        self.as_mut().play_row(&path, &title, &artist, &cover, dur_ms);
    }

    pub fn pause(mut self: Pin<&mut Self>) {
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
            self.as_ref().push_mpris_state();
        }
    }

    pub fn resume(mut self: Pin<&mut Self>) {
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
            self.as_ref().push_mpris_state();
        }
    }

    pub fn stop(mut self: Pin<&mut Self>) {
        {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            if let Some(e) = r.engine.as_mut() {
                e.stop();
            }
        }
        self.as_mut().set_state_text(QString::from("Stopped"));
        self.as_mut().set_current_title(QString::from(""));
        self.as_mut().set_current_artist(QString::from(""));
        self.as_mut().set_current_cover_thumb(QString::from(""));
        self.as_mut().set_lyrics_json(QString::from(EMPTY_LYRICS_JSON));
        self.as_mut().set_position_secs(0.0);
        self.as_mut().set_duration_secs(0.0);
        // Notify MPRIS of Stopped status + cleared metadata.
        self.as_ref().push_mpris_state();
    }

    fn refresh_position(mut self: Pin<&mut Self>) {
        let pos = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.engine.as_ref().map(|e| e.position_secs()).unwrap_or(0.0)
        };
        self.as_mut().set_position_secs(pos);
    }

    fn seek(mut self: Pin<&mut Self>, fraction: f64) {
        let clamped = fraction.clamp(0.0, 1.0);
        let target_secs = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            let dur = r.duration_secs;
            let secs = clamped * dur;
            // Call seek on the engine; ignore errors (seek is best-effort).
            if let Some(e) = r.engine.as_mut() {
                let _ = e.seek(secs);
            }
            secs
        };
        // Update position display immediately so the UI feels responsive,
        // even though the engine seek may be a no-op.
        self.as_mut().set_position_secs(target_secs);
    }
}
