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
        type Player = super::PlayerRust;

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
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use lyra_core::PlayQueue;
use lyra_engine::Engine;

// ── Row type for the cached playlist ────────────────────────────────────────

#[derive(Clone)]
struct TrackRow {
    id: i64,
    title: String,
    artist: String,
    path: String,
    cover_thumb: String,
}

// ── Backing struct ───────────────────────────────────────────────────────────

pub struct PlayerRust {
    state_text: QString,
    current_title: QString,
    current_artist: QString,
    current_cover_thumb: QString,
    queue_json: QString,

    /// Lazily initialised on first `play()`.  `!Send` — Qt thread only.
    engine: Option<Engine>,

    /// Play-queue holding track ids.
    play_queue: PlayQueue,

    /// Cached playlist parsed from the last `play_from_list` call.
    playlist: Vec<TrackRow>,
}

impl Default for PlayerRust {
    fn default() -> Self {
        Self {
            state_text: QString::from("Stopped"),
            current_title: QString::from(""),
            current_artist: QString::from(""),
            current_cover_thumb: QString::from(""),
            queue_json: QString::from("[]"),
            engine: None,
            play_queue: PlayQueue::new(),
            playlist: Vec::new(),
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
            if path.is_empty() {
                return None;
            }
            Some(TrackRow { id, title, artist, path, cover_thumb })
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

    /// Internal: play a path string, update title/artist/cover/state properties.
    fn play_row(mut self: Pin<&mut Self>, path: &str, title: &str, artist: &str, cover_thumb: &str) {
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
        self.as_mut().play_row(&path_s, &title_s, &artist_s, "");
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

        let (path, title, artist, cover) = (
            row.path.clone(),
            row.title.clone(),
            row.artist.clone(),
            row.cover_thumb.clone(),
        );
        self.as_mut().play_row(&path, &title, &artist, &cover);
    }

    fn next(mut self: Pin<&mut Self>) {
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
        let (path, title, artist, cover, queue_json) = {
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
                qj,
            )
        };

        self.as_mut()
            .set_queue_json(QString::from(queue_json.as_str()));
        self.as_mut().play_row(&path, &title, &artist, &cover);
    }

    fn prev(mut self: Pin<&mut Self>) {
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
        let (path, title, artist, cover, queue_json) = {
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
                qj,
            )
        };

        self.as_mut()
            .set_queue_json(QString::from(queue_json.as_str()));
        self.as_mut().play_row(&path, &title, &artist, &cover);
    }

    fn pause(mut self: Pin<&mut Self>) {
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
    }
}
