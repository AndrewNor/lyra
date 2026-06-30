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
        /// Accent colour (hex `#rrggbb`) sampled from the current cover art.
        /// Drives the album-art-tinted UI accent.  Falls back to a default when
        /// there is no cover.
        #[qproperty(QString, current_accent)]
        #[qproperty(QString, queue_json)]
        /// Lyrics for the current track as JSON: `{"synced":bool,"lines":[{"t":number|null,"text":string}]}`.
        #[qproperty(QString, lyrics_json)]
        /// Current playback position in seconds (updated by `refreshPosition`).
        #[qproperty(f64, position_secs)]
        /// Total duration of the current track in seconds (set when a track starts).
        #[qproperty(f64, duration_secs)]
        /// Master volume gain, 0.0..=1.0.  Default 1.0.
        #[qproperty(f64, volume)]
        /// Whether shuffle is currently enabled.
        #[qproperty(bool, shuffle)]
        /// Current repeat mode: "off", "all", or "one".
        #[qproperty(QString, repeat_mode)]
        /// Whether the graphic equalizer is enabled.
        #[qproperty(bool, eq_enabled)]
        /// JSON array of EQ bands: `[{"freq":31,"gain":0.0},…]` (10 entries).
        #[qproperty(QString, eq_bands_json)]
        /// Whether bit-perfect mode is active.
        #[qproperty(bool, bit_perfect)]
        /// Crossfade duration in seconds (0.0 = off, max 12 s).
        #[qproperty(f64, crossfade_secs)]
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

        /// Seek to an absolute position in seconds.
        /// Used by the MPRIS `Seek`/`SetPosition` D-Bus methods.
        #[qinvokable]
        #[cxx_name = "seekToSecs"]
        fn seek_to_secs(self: Pin<&mut Player>, secs: f64);

        /// Set the master volume (0.0..=1.0).  Updates the engine immediately.
        #[qinvokable]
        #[cxx_name = "changeVolume"]
        fn set_volume_invokable(self: Pin<&mut Player>, v: f64);

        /// Toggle shuffle on/off.
        #[qinvokable]
        #[cxx_name = "toggleShuffle"]
        fn toggle_shuffle(self: Pin<&mut Player>);

        /// Cycle repeat mode: off → all → one → off.
        #[qinvokable]
        #[cxx_name = "cycleRepeat"]
        fn cycle_repeat(self: Pin<&mut Player>);

        /// Enable or disable the graphic equalizer.
        #[qinvokable]
        #[cxx_name = "setEqEnabled"]
        fn set_eq_enabled_invokable(self: Pin<&mut Player>, enabled: bool);

        /// Set the gain (in dB, −12..+12) for one EQ band (0-based index).
        #[qinvokable]
        #[cxx_name = "setEqGain"]
        fn set_eq_gain_invokable(self: Pin<&mut Player>, band: i32, db: f64);

        /// Reset all EQ bands to 0 dB.
        #[qinvokable]
        #[cxx_name = "resetEq"]
        fn reset_eq_invokable(self: Pin<&mut Player>);

        /// Enable or disable bit-perfect mode.
        #[qinvokable]
        #[cxx_name = "setBitPerfect"]
        fn set_bit_perfect_invokable(self: Pin<&mut Player>, enabled: bool);

        /// Set the crossfade duration in seconds (0.0 = off, clamped to 0..12 s).
        #[qinvokable]
        #[cxx_name = "setCrossfade"]
        fn set_crossfade_invokable(self: Pin<&mut Player>, secs: f64);

        /// Return the 24 spectrum band levels (0.0..=1.0) as a JSON array.
        /// Poll this from QML (e.g. every 33 ms) to drive the spectrum visualizer.
        /// Returns "[0,0,...,0]" (24 zeros) when stopped.
        #[qinvokable]
        #[cxx_name = "spectrumLevels"]
        fn spectrum_levels(self: Pin<&mut Player>) -> QString;

        /// Restore the last playback session from state.json on startup.
        /// Call from QML `Component.onCompleted` after the library loads.
        /// Loads the queue and current track (paused/stopped), does NOT play.
        /// Falls back to the first 50 library tracks if no session file exists.
        #[qinvokable]
        #[cxx_name = "restoreSession"]
        fn restore_session(self: Pin<&mut Player>);

        /// Play the current queue track from its saved position (if any).
        /// Use this from QML play button when state_text is "Stopped" but a
        /// current track is already loaded.
        #[qinvokable]
        #[cxx_name = "playCurrent"]
        fn play_current(self: Pin<&mut Player>);
    }

    impl cxx_qt::Threading for Player {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use lyra_core::{PlayQueue, RepeatMode};
use lyra_engine::{Engine, EQ_FREQS_HZ};
use mpris_server::PlaybackStatus;
use serde::{Deserialize, Serialize};

use crate::mpris::{MprisHandle, MprisState};
use lyra_db::Db;

// ── Session persistence ──────────────────────────────────────────────────────

/// Persisted session state written to `$XDG_DATA_HOME/lyra/state.json`.
/// `tracks_json` is the same format as the Library `results_json` array.
#[derive(Serialize, Deserialize, Default)]
struct SessionState {
    /// The full playlist the queue was built from (Library results_json format).
    tracks_json: String,
    /// Index of the current track within the playlist.
    index: i32,
    /// Playback position in seconds at the time of save.
    position_secs: f64,
    /// Master volume (0.0..=1.0).
    volume: f64,
    /// Whether shuffle was enabled.
    shuffle: bool,
    /// Repeat mode string: "off", "all", or "one".
    repeat_mode: String,
}

/// Write `SessionState` to `paths::state_file()`.  Never panics.
fn save_session(state: &SessionState) {
    let path = crate::paths::state_file();
    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string(state) {
        Ok(json) => {
            let _ = std::fs::write(&path, json.as_bytes());
        }
        Err(e) => {
            eprintln!("[lyra] session save error: {e}");
        }
    }
}

/// Read `SessionState` from `paths::state_file()`.  Returns `None` on any
/// error (missing file, parse failure, etc.) — never panics.
fn load_session() -> Option<SessionState> {
    let path = crate::paths::state_file();
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice::<SessionState>(&bytes).ok()
}

/// Build the `eq_bands_json` string from a gains array.
fn build_eq_bands_json(gains: &[f32; 10]) -> String {
    let mut out = String::from("[");
    for (i, (&freq, &gain)) in EQ_FREQS_HZ.iter().zip(gains.iter()).enumerate() {
        if i > 0 {
            out.push(',');
        }
        // Use integer frequency labels; gains to 2 decimal places.
        out.push_str(&format!(
            r#"{{"freq":{},"gain":{:.2}}}"#,
            freq as u32, gain
        ));
    }
    out.push(']');
    out
}

// ── Album-art accent colour extraction ───────────────────────────────────────

/// Fallback accent when there is no cover (or extraction fails) — a calm indigo
/// that reads well on the light theme.
const DEFAULT_ACCENT: &str = "#5b62d6";

/// Sample a vibrant, light-theme-friendly accent colour from a cover thumbnail.
///
/// Loads the (already small) thumbnail, downsamples further, and computes a
/// saturation-weighted average that favours vivid, mid-lightness pixels and
/// ignores near-black / near-white / grey.  The result is pushed into a
/// controlled saturation/lightness band so it always works as an accent on a
/// near-white background.  Returns `DEFAULT_ACCENT` on any failure or for
/// (near-)monochrome art.  Never panics.
fn accent_from_cover(cover_thumb: &str) -> String {
    if cover_thumb.is_empty() {
        return DEFAULT_ACCENT.to_owned();
    }
    let img = match image::open(cover_thumb) {
        Ok(i) => i,
        Err(_) => return DEFAULT_ACCENT.to_owned(),
    };
    let small = img.thumbnail(48, 48).to_rgb8();

    let (mut wr, mut wg, mut wb, mut wsum) = (0f64, 0f64, 0f64, 0f64);
    for px in small.pixels() {
        let r = px[0] as f64 / 255.0;
        let g = px[1] as f64 / 255.0;
        let b = px[2] as f64 / 255.0;
        let (_, s, l) = rgb_to_hsl(r, g, b);
        // Peak weight at mid-lightness; zero at pure black/white.
        let light_window = 1.0 - (2.0 * l - 1.0).powi(2);
        let w = s * s * light_window;
        wr += r * w;
        wg += g * w;
        wb += b * w;
        wsum += w;
    }

    if wsum < 1e-6 {
        return DEFAULT_ACCENT.to_owned(); // monochrome / no colour to pull
    }

    let (h, s, l) = rgb_to_hsl(wr / wsum, wg / wsum, wb / wsum);
    // Force into a band that reads as a confident accent on a light ground.
    let (r, g, b) = hsl_to_rgb(h, s.max(0.55), l.clamp(0.42, 0.55));
    format!(
        "#{:02x}{:02x}{:02x}",
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8
    )
}

fn rgb_to_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-9 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    (h, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (r1 + m, g1 + m, b1 + m)
}

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
    /// Accent colour (hex) sampled from the current cover.
    current_accent: QString,
    queue_json: QString,
    /// Lyrics for the current track, serialised as JSON.
    lyrics_json: QString,

    /// Current playback position in seconds (polled from the engine).
    position_secs: f64,

    /// Duration of the current track in seconds (set when a track starts).
    duration_secs: f64,

    /// Master volume gain, 0.0..=1.0.  Persisted across tracks.
    volume: f64,

    /// Whether shuffle is currently active.
    shuffle: bool,

    /// Current repeat mode as a string: "off", "all", or "one".
    repeat_mode: QString,

    /// Lazily initialised on first `play()`.  `!Send` — Qt thread only.
    engine: Option<Engine>,

    /// Play-queue holding track ids.
    play_queue: PlayQueue,

    /// Cached playlist parsed from the last `play_from_list` call.
    playlist: Vec<TrackRow>,

    /// MPRIS2 server handle — initialised once from QML via `initMpris`.
    mpris_handle: Option<MprisHandle>,

    /// Whether the graphic EQ is enabled.
    eq_enabled: bool,

    /// JSON representation of the 10 EQ bands (freq + gain per band).
    eq_bands_json: QString,

    /// Whether bit-perfect mode is active.
    bit_perfect: bool,

    /// Crossfade duration in seconds (0.0 = off).  Persisted across tracks.
    crossfade_secs: f64,

    /// Persisted EQ gains (applied to the engine when it's lazily created).
    eq_gains: [f32; 10],

    /// Position (in seconds) restored from session state, to seek to on the
    /// next `play_current` call.  Cleared after the seek is applied.
    pending_resume_secs: f64,
}

const EMPTY_LYRICS_JSON: &str = r#"{"synced":false,"lines":[]}"#;

impl Default for PlayerRust {
    fn default() -> Self {
        let default_gains = [0.0f32; 10];
        Self {
            state_text: QString::from("Stopped"),
            current_title: QString::from(""),
            current_artist: QString::from(""),
            current_cover_thumb: QString::from(""),
            current_accent: QString::from(DEFAULT_ACCENT),
            queue_json: QString::from("[]"),
            lyrics_json: QString::from(EMPTY_LYRICS_JSON),
            position_secs: 0.0,
            duration_secs: 0.0,
            volume: 1.0,
            shuffle: false,
            repeat_mode: QString::from("off"),
            engine: None,
            play_queue: PlayQueue::new(),
            playlist: Vec::new(),
            mpris_handle: None,
            eq_enabled: false,
            eq_bands_json: QString::from(build_eq_bands_json(&default_gains).as_str()),
            bit_perfect: false,
            crossfade_secs: 0.0,
            eq_gains: default_gains,
            pending_resume_secs: 0.0,
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
            seeked_to_us: None,
        };
        handle.update(state);
    }

    // ── Engine management ─────────────────────────────────────────────────────

    /// Ensure the engine exists, returning false and setting state_text on error.
    /// When the engine is lazily created, the stored volume is applied immediately.
    fn ensure_engine(mut self: Pin<&mut Self>) -> bool {
        let needs_init = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.engine.is_none()
        };

        if needs_init {
            match Engine::new() {
                Ok(e) => {
                    let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
                    // Apply stored settings so they persist across tracks.
                    e.set_volume(r.volume as f32);
                    e.set_eq_enabled(r.eq_enabled);
                    for (i, &gain) in r.eq_gains.iter().enumerate() {
                        e.set_eq_gain(i, gain);
                    }
                    e.set_bit_perfect(r.bit_perfect);
                    e.set_crossfade_secs(r.crossfade_secs as f32);
                    r.engine = Some(e);
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
    ///
    /// `next_path` — if `Some`, informs the engine of the next track so the
    /// crossfade mixer can pre-open it when needed.  Pass `None` when the next
    /// track is unknown (e.g. direct `play()` invokable).
    fn play_row(
        mut self: Pin<&mut Self>,
        path: &str,
        title: &str,
        artist: &str,
        cover_thumb: &str,
        duration_ms: u64,
        next_path: Option<&str>,
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
                // Inform the engine of the next track for crossfade preparation.
                {
                    let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
                    if let Some(e) = r.engine.as_ref() {
                        e.set_next_track_path(
                            next_path.map(std::path::Path::new)
                        );
                    }
                }
                self.as_mut().set_current_title(QString::from(title));
                self.as_mut().set_current_artist(QString::from(artist));
                self.as_mut().set_current_cover_thumb(QString::from(cover_thumb));
                let accent = accent_from_cover(cover_thumb);
                self.as_mut().set_current_accent(QString::from(accent.as_str()));
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
        // Duration and next track unknown when using the direct `play()` invokable.
        self.as_mut().play_row(&path_s, &title_s, &artist_s, "", 0, None);
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

        // Resolve the next track path for crossfade pre-loading.
        let next_row_path: Option<String> = if idx + 1 < playlist.len() {
            Some(playlist[idx + 1].path.clone())
        } else {
            None
        };

        let (path, title, artist, cover, dur_ms) = (
            row.path.clone(),
            row.title.clone(),
            row.artist.clone(),
            row.cover_thumb.clone(),
            row.duration_ms,
        );
        self.as_mut().play_row(
            &path,
            &title,
            &artist,
            &cover,
            dur_ms,
            next_row_path.as_deref(),
        );
        // Persist session after track change.
        self.as_ref().save_current_session();
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
        let (path, title, artist, cover, dur_ms, queue_json, next_path) = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            let playlist = &r.playlist;
            let Some(row) = find_row(playlist, id) else {
                return;
            };
            let pos = playlist.iter().position(|r| r.id == id);
            let next_p = pos
                .and_then(|i| playlist.get(i + 1))
                .map(|r| r.path.clone());
            let qj = build_queue_json(playlist, Some(id));
            (
                row.path.clone(),
                row.title.clone(),
                row.artist.clone(),
                row.cover_thumb.clone(),
                row.duration_ms,
                qj,
                next_p,
            )
        };

        self.as_mut()
            .set_queue_json(QString::from(queue_json.as_str()));
        self.as_mut().play_row(
            &path,
            &title,
            &artist,
            &cover,
            dur_ms,
            next_path.as_deref(),
        );
        // Persist session after track change.
        self.as_ref().save_current_session();
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
        let (path, title, artist, cover, dur_ms, queue_json, next_path) = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            let playlist = &r.playlist;
            let Some(row) = find_row(playlist, id) else {
                return;
            };
            let pos = playlist.iter().position(|r| r.id == id);
            let next_p = pos
                .and_then(|i| playlist.get(i + 1))
                .map(|r| r.path.clone());
            let qj = build_queue_json(playlist, Some(id));
            (
                row.path.clone(),
                row.title.clone(),
                row.artist.clone(),
                row.cover_thumb.clone(),
                row.duration_ms,
                qj,
                next_p,
            )
        };

        self.as_mut()
            .set_queue_json(QString::from(queue_json.as_str()));
        self.as_mut().play_row(
            &path,
            &title,
            &artist,
            &cover,
            dur_ms,
            next_path.as_deref(),
        );
        // Persist session after track change.
        self.as_ref().save_current_session();
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
            // Capture position at pause time and persist.
            self.as_ref().save_current_session();
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
        // Persist the cleared state.
        self.as_ref().save_current_session();
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
        // Notify MPRIS of the new position so the Seeked signal is emitted.
        let pos_us = (target_secs * 1_000_000.0) as i64;
        let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
        if let Some(ref handle) = r.mpris_handle {
            handle.notify_seeked(pos_us);
        }
    }

    pub fn seek_to_secs(mut self: Pin<&mut Self>, secs: f64) {
        let dur = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.duration_secs
        };

        if dur <= 0.0 {
            return;
        }

        let target_secs = secs.clamp(0.0, dur);
        {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            if let Some(e) = r.engine.as_mut() {
                let _ = e.seek(target_secs);
            }
        }
        self.as_mut().set_position_secs(target_secs);
        // Notify MPRIS so the Seeked signal is emitted (when called from D-Bus,
        // the MPRIS loop already set seeked_to_us, but this is harmless to repeat).
        let pos_us = (target_secs * 1_000_000.0) as i64;
        let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
        if let Some(ref handle) = r.mpris_handle {
            handle.notify_seeked(pos_us);
        }
    }

    fn set_volume_invokable(mut self: Pin<&mut Self>, v: f64) {
        let clamped = v.clamp(0.0, 1.0);
        // Apply to the engine immediately (reads `clamped`, not the field).
        {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            if let Some(e) = r.engine.as_ref() {
                e.set_volume(clamped as f32);
            }
        }
        // `set_volume` (generated, change-guarded) writes r.volume AND emits
        // volumeChanged.  Don't pre-mutate r.volume above or the signal is lost
        // (so the slider wouldn't reflect restored/MPRIS volume changes).
        self.as_mut().set_volume(clamped);
        self.as_ref().save_current_session();
    }

    fn toggle_shuffle(mut self: Pin<&mut Self>) {
        // Update ONLY the play-queue directly here.  The `shuffle` qproperty
        // field must be written via the generated `set_shuffle` setter below,
        // NOT pre-mutated: cxx-qt setters are change-guarded (`if self.shuffle
        // == value { return }`), so pre-mutating the field makes the setter a
        // no-op and `shuffleChanged` never fires — leaving QML bindings stale.
        let new_shuffle = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            let s = !r.shuffle;
            r.play_queue.set_shuffle(s);
            s
        };
        self.as_mut().set_shuffle(new_shuffle);
        self.as_ref().save_current_session();
    }

    fn cycle_repeat(mut self: Pin<&mut Self>) {
        let new_mode_str = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            let current = r.play_queue.repeat();
            let next = match current {
                RepeatMode::Off => RepeatMode::All,
                RepeatMode::All => RepeatMode::One,
                RepeatMode::One => RepeatMode::Off,
            };
            r.play_queue.set_repeat(next);
            match next {
                RepeatMode::Off => "off",
                RepeatMode::All => "all",
                RepeatMode::One => "one",
            }
        };
        self.as_mut().set_repeat_mode(QString::from(new_mode_str));
        self.as_ref().save_current_session();
    }

    fn set_eq_enabled_invokable(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            if let Some(e) = r.engine.as_ref() {
                e.set_eq_enabled(enabled);
            }
        }
        // Generated setter writes the field + emits the change signal; don't
        // pre-mutate r.eq_enabled or the change-guarded setter becomes a no-op.
        self.as_mut().set_eq_enabled(enabled);
    }

    fn set_eq_gain_invokable(mut self: Pin<&mut Self>, band: i32, db: f64) {
        if band < 0 || band >= 10 {
            return;
        }
        let band_usize = band as usize;
        let db_f32 = db as f32;
        let new_json = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.eq_gains[band_usize] = db_f32.clamp(-12.0, 12.0);
            if let Some(e) = r.engine.as_ref() {
                e.set_eq_gain(band_usize, db_f32);
            }
            build_eq_bands_json(&r.eq_gains)
        };
        self.as_mut().set_eq_bands_json(QString::from(new_json.as_str()));
    }

    fn reset_eq_invokable(mut self: Pin<&mut Self>) {
        let new_json = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.eq_gains = [0.0; 10];
            if let Some(e) = r.engine.as_ref() {
                e.reset_eq();
            }
            build_eq_bands_json(&r.eq_gains)
        };
        self.as_mut().set_eq_bands_json(QString::from(new_json.as_str()));
    }

    fn set_bit_perfect_invokable(mut self: Pin<&mut Self>, enabled: bool) {
        {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            if let Some(e) = r.engine.as_ref() {
                e.set_bit_perfect(enabled);
            }
        }
        // Generated setter writes the field + emits; don't pre-mutate the field.
        self.as_mut().set_bit_perfect(enabled);
    }

    fn set_crossfade_invokable(mut self: Pin<&mut Self>, secs: f64) {
        let clamped = secs.clamp(0.0, 12.0);
        {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            if let Some(e) = r.engine.as_ref() {
                e.set_crossfade_secs(clamped as f32);
            }
        }
        // Generated setter writes the field + emits; don't pre-mutate the field.
        self.as_mut().set_crossfade_secs(clamped);
    }

    fn spectrum_levels(self: Pin<&mut Self>) -> QString {
        let arr = {
            let r = self.rust();
            r.engine.as_ref().map(|e| e.spectrum_levels()).unwrap_or([0.0f32; 24])
        };

        // Build a compact JSON array: [0.123,0.456,...] — 24 values, 3 d.p.
        let mut s = String::with_capacity(24 * 7 + 2);
        s.push('[');
        for (i, &v) in arr.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            // Clamp and format to 3 decimal places.
            let clamped = v.clamp(0.0, 1.0);
            // Use a compact integer representation to avoid locale issues.
            let millis = (clamped * 1000.0).round() as u32;
            let whole = millis / 1000;
            let frac = millis % 1000;
            s.push_str(&format!("{whole}.{frac:03}"));
        }
        s.push(']');

        QString::from(s.as_str())
    }

    // ── Session persistence helpers ───────────────────────────────────────────

    /// Collect current player state and write it to `state.json`.
    /// Never panics; errors are logged to stderr.
    fn save_current_session(self: Pin<&Self>) {
        let r = self.rust();

        // If there is no playlist, nothing meaningful to persist.
        if r.playlist.is_empty() {
            return;
        }

        // Build tracks_json from the cached playlist.
        let tracks_json = {
            let values: Vec<serde_json::Value> = r.playlist.iter().map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "title": t.title,
                    "artist": t.artist,
                    "album": "",
                    "path": t.path,
                    "durationMs": t.duration_ms,
                    "cover_thumb": t.cover_thumb,
                })
            }).collect();
            serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_owned())
        };

        // Current queue index: find the position of the current id in playlist.
        let current_id = r.play_queue.current();
        let index = current_id
            .and_then(|id| r.playlist.iter().position(|row| row.id == id))
            .unwrap_or(0) as i32;

        // Live position from engine (more accurate than the polled property).
        let position_secs = r.engine
            .as_ref()
            .map(|e| e.position_secs())
            .unwrap_or(r.position_secs);

        let repeat_mode = r.repeat_mode.to_string();

        let state = SessionState {
            tracks_json,
            index,
            position_secs,
            volume: r.volume,
            shuffle: r.shuffle,
            repeat_mode,
        };

        save_session(&state);
    }

    /// Restore the last session from `state.json`.  Called from QML
    /// `Component.onCompleted`.  Loads queue and current track display
    /// (paused/stopped) — does NOT call the audio engine.
    /// Falls back to the first 50 library tracks when no session exists.
    fn restore_session(mut self: Pin<&mut Self>) {
        let session_opt = load_session();

        match session_opt {
            Some(session) if !session.tracks_json.is_empty() => {
                eprintln!("[lyra] restore_session: restoring saved session (index={})", session.index);

                let playlist = parse_playlist(&session.tracks_json);
                if playlist.is_empty() {
                    eprintln!("[lyra] restore_session: empty playlist in state file, falling back");
                    self.as_mut().restore_fallback();
                    return;
                }

                let idx = (session.index as usize).min(playlist.len().saturating_sub(1));
                let row = playlist[idx].clone();
                let current_id = Some(row.id);

                // Rebuild the PlayQueue.
                let ids: Vec<i64> = playlist.iter().map(|r| r.id).collect();
                {
                    let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
                    r.playlist = playlist.clone();
                    r.play_queue.set_items(ids);
                    r.play_queue.jump_to(idx);
                }

                // Restore volume, shuffle, repeat.
                let volume = session.volume.clamp(0.0, 1.0);
                let shuffle = session.shuffle;
                let repeat_mode_str = session.repeat_mode.clone();
                let repeat_mode = match repeat_mode_str.as_str() {
                    "all" => RepeatMode::All,
                    "one" => RepeatMode::One,
                    _ => RepeatMode::Off,
                };

                {
                    let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
                    r.volume = volume;
                    r.shuffle = shuffle;
                    r.play_queue.set_shuffle(shuffle);
                    r.play_queue.set_repeat(repeat_mode);
                    // Store the resume position to apply when play_current is called.
                    r.pending_resume_secs = session.position_secs;
                }

                // Update QObject properties.
                self.as_mut().set_volume(volume);
                self.as_mut().set_shuffle(shuffle);
                self.as_mut().set_repeat_mode(QString::from(repeat_mode_str.as_str()));

                // Set the current track display.
                self.as_mut().set_current_title(QString::from(row.title.as_str()));
                self.as_mut().set_current_artist(QString::from(row.artist.as_str()));
                self.as_mut().set_current_cover_thumb(QString::from(row.cover_thumb.as_str()));
                self.as_mut().set_current_accent(QString::from(accent_from_cover(&row.cover_thumb).as_str()));
                self.as_mut().set_duration_secs(row.duration_ms as f64 / 1000.0);
                self.as_mut().set_position_secs(session.position_secs);
                self.as_mut().set_state_text(QString::from("Stopped"));

                // Build queue_json (tracks after current).
                let queue_json = build_queue_json(&playlist, current_id);
                self.as_mut().set_queue_json(QString::from(queue_json.as_str()));

                eprintln!("[lyra] restore_session: loaded \"{}\" at {:.1}s", row.title, session.position_secs);
            }
            _ => {
                eprintln!("[lyra] restore_session: no session file, loading fallback tracks");
                self.as_mut().restore_fallback();
            }
        }
    }

    /// Fallback: load the first 50 tracks from the library into the queue.
    /// The now-playing panel will show the first track; nothing plays.
    fn restore_fallback(mut self: Pin<&mut Self>) {
        // Open a read-only db connection to fetch tracks.
        let db_path = crate::paths::library_db_path();
        let tracks_opt = Db::open(&db_path)
            .ok()
            .and_then(|db| db.recently_added(50).ok());

        let Some(tracks) = tracks_opt else {
            eprintln!("[lyra] restore_fallback: no tracks available");
            return;
        };

        if tracks.is_empty() {
            eprintln!("[lyra] restore_fallback: library is empty");
            return;
        }

        // Convert db Track rows to our cached TrackRow format.
        let playlist: Vec<TrackRow> = tracks.iter().map(|t| TrackRow {
            id: t.id,
            title: t.title.clone(),
            artist: t.artist.clone().unwrap_or_default(),
            path: t.path.clone(),
            cover_thumb: t.cover_thumb.clone().unwrap_or_default(),
            duration_ms: t.duration_ms.unwrap_or(0),
        }).collect();

        let ids: Vec<i64> = playlist.iter().map(|r| r.id).collect();
        {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            r.playlist = playlist.clone();
            r.play_queue.set_items(ids);
            r.play_queue.jump_to(0);
        }

        let row = &playlist[0];
        let current_id = Some(row.id);
        let queue_json = build_queue_json(&playlist, current_id);

        self.as_mut().set_current_title(QString::from(row.title.as_str()));
        self.as_mut().set_current_artist(QString::from(row.artist.as_str()));
        self.as_mut().set_current_cover_thumb(QString::from(row.cover_thumb.as_str()));
        self.as_mut().set_current_accent(QString::from(accent_from_cover(&row.cover_thumb).as_str()));
        self.as_mut().set_duration_secs(row.duration_ms as f64 / 1000.0);
        self.as_mut().set_position_secs(0.0);
        self.as_mut().set_state_text(QString::from("Stopped"));
        self.as_mut().set_queue_json(QString::from(queue_json.as_str()));

        eprintln!("[lyra] restore_fallback: loaded {} tracks, showing \"{}\"", playlist.len(), row.title);
    }

    /// Play the current queue track from its pending resume position (if any).
    /// Used by the QML play button when state is "Stopped" but a track is loaded.
    fn play_current(mut self: Pin<&mut Self>) {
        // Collect what we need before calling play_row.
        let (path, title, artist, cover, dur_ms, next_path) = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            let current_id = r.play_queue.current();
            let Some(id) = current_id else {
                return;
            };
            let Some(row) = find_row(&r.playlist, id) else {
                return;
            };
            let pos = r.playlist.iter().position(|x| x.id == id);
            let next_p = pos
                .and_then(|i| r.playlist.get(i + 1))
                .map(|x| x.path.clone());
            (
                row.path.clone(),
                row.title.clone(),
                row.artist.clone(),
                row.cover_thumb.clone(),
                row.duration_ms,
                next_p,
            )
        };

        // Grab the pending resume position and clear it.
        let resume_secs = {
            let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
            let s = r.pending_resume_secs;
            r.pending_resume_secs = 0.0;
            s
        };

        self.as_mut().play_row(
            &path,
            &title,
            &artist,
            &cover,
            dur_ms,
            next_path.as_deref(),
        );

        // Seek to the saved position after play starts, if applicable.
        if resume_secs > 0.5 {
            let dur = {
                let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
                r.duration_secs
            };
            if dur > 0.0 {
                let target = resume_secs.min(dur - 0.1).max(0.0);
                {
                    let r = unsafe { self.as_mut().rust_mut().get_unchecked_mut() };
                    if let Some(e) = r.engine.as_mut() {
                        let _ = e.seek(target);
                    }
                }
                self.as_mut().set_position_secs(target);
            }
        }

        // Persist session.
        self.as_ref().save_current_session();
    }
}
