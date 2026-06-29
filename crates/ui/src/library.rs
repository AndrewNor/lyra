//! Library QObject — real db search / scan exposed to QML.
//!
//! # Threading
//! The `Db` connection lives exclusively on the Qt thread.  `scan()` opens a
//! *separate* `Db` connection on a background OS thread, runs the filesystem
//! scan there, then uses `qt_thread().queue(...)` to update the Qt-thread `Db`
//! indirectly (it calls `load_all()` which re-queries the *same file* now
//! updated by the bg thread) and flip `scanning=false`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, results_json)]
        #[qproperty(i32, track_count)]
        #[qproperty(bool, scanning)]
        #[qproperty(QString, status_text)]
        #[qproperty(QString, albums_json)]
        #[qproperty(QString, artists_json)]
        type Library = super::LibraryRust;

        /// Load all tracks from the db → resultsJson + trackCount.
        #[qinvokable]
        #[cxx_name = "loadAll"]
        fn load_all(self: Pin<&mut Library>);

        /// Search tracks; empty query falls back to loadAll.
        #[qinvokable]
        fn search(self: Pin<&mut Library>, query: QString);

        /// Background scan of `~/Music`, then reload.
        #[qinvokable]
        fn scan(self: Pin<&mut Library>);

        /// Load all albums → albums_json.
        #[qinvokable]
        #[cxx_name = "loadAlbums"]
        fn load_albums(self: Pin<&mut Library>);

        /// Load all artists → artists_json.
        #[qinvokable]
        #[cxx_name = "loadArtists"]
        fn load_artists(self: Pin<&mut Library>);

        /// Load tracks for a specific album → results_json.
        #[qinvokable]
        #[cxx_name = "loadAlbumTracks"]
        fn load_album_tracks(self: Pin<&mut Library>, id: i64);

        /// Load tracks for a specific artist → results_json.
        #[qinvokable]
        #[cxx_name = "loadArtistTracks"]
        fn load_artist_tracks(self: Pin<&mut Library>, id: i64);
    }

    impl cxx_qt::Threading for Library {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use lyra_db::{Album, Artist, Db, Track};

use crate::paths::{art_cache_dir, library_db_path};

// ── Backing struct ───────────────────────────────────────────────────────────

pub struct LibraryRust {
    results_json: QString,
    track_count: i32,
    scanning: bool,
    status_text: QString,
    albums_json: QString,
    artists_json: QString,

    /// Live database connection (Qt thread only).
    db: Db,

    /// Directory to scan when `scan()` is called.
    music_dir: std::path::PathBuf,
}

impl Default for LibraryRust {
    fn default() -> Self {
        let db_path = library_db_path();
        let (db, status) = match Db::open(&db_path) {
            Ok(d) => (d, QString::from("Ready")),
            Err(e) => {
                // Best-effort fallback: in-memory db so the UI still starts.
                let fallback = match Db::open_in_memory() {
                    Ok(d) => d,
                    Err(_) => {
                        // Truly unrecoverable — construct a dummy Db that will
                        // surface errors on every query.
                        Db::open_in_memory().unwrap_or_else(|_| {
                            // Cannot open even in-memory db; this is a fatal
                            // environment issue.  Return a fresh in-memory db
                            // (the second attempt may or may not succeed; if it
                            // fails we have no choice but to panic in Default).
                            panic!("cannot open any database — aborting")
                        })
                    }
                };
                let msg = format!("db error (in-memory fallback): {e}");
                (fallback, QString::from(msg.as_str()))
            }
        };

        let music_dir = std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join("Music"))
            .unwrap_or_else(|_| std::path::PathBuf::from("/Music"));

        Self {
            results_json: QString::from("[]"),
            track_count: 0,
            scanning: false,
            status_text: status,
            albums_json: QString::from("[]"),
            artists_json: QString::from("[]"),
            db,
            music_dir,
        }
    }
}

// ── JSON helpers ─────────────────────────────────────────────────────────────

/// Build a JSON array string from a slice of `Track`s.
/// Uses `serde_json` for correct string escaping.
pub(crate) fn tracks_to_json(tracks: &[Track]) -> String {
    let values: Vec<serde_json::Value> = tracks
        .iter()
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "title": t.title,
                "artist": t.artist.as_deref().unwrap_or(""),
                "album": t.album.as_deref().unwrap_or(""),
                "path": t.path,
                "durationMs": t.duration_ms.unwrap_or(0),
                "cover_thumb": t.cover_thumb.as_deref().unwrap_or(""),
            })
        })
        .collect();
    // serde_json serializes the outer Vec as a JSON array.
    serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_owned())
}

/// Build a JSON array string from a slice of `Album`s.
pub(crate) fn albums_to_json(albums: &[Album]) -> String {
    let values: Vec<serde_json::Value> = albums
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "title": a.title,
                "artist": a.artist.as_deref().unwrap_or(""),
                "year": a.year,
                "track_count": a.track_count,
                "cover_thumb": a.cover_thumb.as_deref().unwrap_or(""),
            })
        })
        .collect();
    serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_owned())
}

/// Build a JSON array string from a slice of `Artist`s.
pub(crate) fn artists_to_json(artists: &[Artist]) -> String {
    let values: Vec<serde_json::Value> = artists
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "name": a.name,
                "album_count": a.album_count,
                "track_count": a.track_count,
            })
        })
        .collect();
    serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_owned())
}

// ── QObject impl ─────────────────────────────────────────────────────────────

impl qobject::Library {
    fn load_all(mut self: Pin<&mut Self>) {
        // Safety: we do not move out of the pin; we only access the db field
        // which does not implement Unpin but is safe to borrow mutably here
        // because we are on the Qt thread and no other reference exists.
        let tracks_result = unsafe { self.as_mut().rust_mut().get_unchecked_mut() }
            .db
            .list_tracks();

        match tracks_result {
            Ok(tracks) => {
                let count = tracks.len() as i32;
                eprintln!("[lyra] load_all: {} tracks loaded", count);
                let json = tracks_to_json(&tracks);
                self.as_mut().set_results_json(QString::from(json.as_str()));
                self.as_mut().set_track_count(count);
                let msg = format!("{count} tracks");
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
            Err(e) => {
                eprintln!("[lyra] load_all error: {e}");
                let msg = format!("loadAll error: {e}");
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
        }
    }

    fn search(mut self: Pin<&mut Self>, query: QString) {
        let q = query.to_string();
        if q.trim().is_empty() {
            self.load_all();
            return;
        }

        let tracks_result = unsafe { self.as_mut().rust_mut().get_unchecked_mut() }
            .db
            .search(&q);

        match tracks_result {
            Ok(tracks) => {
                let count = tracks.len() as i32;
                let json = tracks_to_json(&tracks);
                self.as_mut().set_results_json(QString::from(json.as_str()));
                self.as_mut().set_track_count(count);
                let msg = format!("{count} results for \"{q}\"");
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
            Err(e) => {
                let msg = format!("search error: {e}");
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
        }
    }

    fn scan(mut self: Pin<&mut Self>) {
        self.as_mut().set_scanning(true);
        self.as_mut()
            .set_status_text(QString::from("Scanning\u{2026}"));

        let db_path = library_db_path();
        // Safety: reading music_dir (PathBuf) — no structural pin invariant.
        let music_dir =
            unsafe { self.as_mut().rust_mut().get_unchecked_mut() }
                .music_dir
                .clone();
        let thread = self.qt_thread();

        std::thread::spawn(move || {
            // Open a SEPARATE db connection on the background thread.
            let art = lyra_library::ArtCache::new(art_cache_dir());
            let summary_msg = match Db::open(&db_path) {
                Err(e) => format!("Scan failed (db): {e}"),
                Ok(mut bg_db) => match lyra_library::scan(&music_dir, &mut bg_db, &art) {
                    Ok(s) => format!(
                        "Scan done — {} added, {} updated, {} unchanged, {} failed",
                        s.added, s.updated, s.unchanged, s.failed
                    ),
                    Err(e) => format!("Scan error: {e}"),
                },
            };

            // Post results back to Qt thread.
            let _ = thread.queue(move |mut lib: Pin<&mut qobject::Library>| {
                // Re-load from the Qt-thread Db (same file, now updated).
                lib.as_mut().load_all();
                lib.as_mut().set_scanning(false);
                lib.as_mut()
                    .set_status_text(QString::from(summary_msg.as_str()));
            });
        });
    }

    fn load_albums(mut self: Pin<&mut Self>) {
        let result = unsafe { self.as_mut().rust_mut().get_unchecked_mut() }
            .db
            .list_albums();
        match result {
            Ok(albums) => {
                let json = albums_to_json(&albums);
                self.as_mut().set_albums_json(QString::from(json.as_str()));
                let msg = format!("{} albums", albums.len());
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
            Err(e) => {
                let msg = format!("loadAlbums error: {e}");
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
        }
    }

    fn load_artists(mut self: Pin<&mut Self>) {
        let result = unsafe { self.as_mut().rust_mut().get_unchecked_mut() }
            .db
            .list_artists();
        match result {
            Ok(artists) => {
                let json = artists_to_json(&artists);
                self.as_mut().set_artists_json(QString::from(json.as_str()));
                let msg = format!("{} artists", artists.len());
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
            Err(e) => {
                let msg = format!("loadArtists error: {e}");
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
        }
    }

    fn load_album_tracks(mut self: Pin<&mut Self>, id: i64) {
        let result = unsafe { self.as_mut().rust_mut().get_unchecked_mut() }
            .db
            .tracks_by_album(id);
        match result {
            Ok(tracks) => {
                let count = tracks.len() as i32;
                let json = tracks_to_json(&tracks);
                self.as_mut().set_results_json(QString::from(json.as_str()));
                self.as_mut().set_track_count(count);
                let msg = format!("{count} tracks");
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
            Err(e) => {
                let msg = format!("loadAlbumTracks error: {e}");
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
        }
    }

    fn load_artist_tracks(mut self: Pin<&mut Self>, id: i64) {
        let result = unsafe { self.as_mut().rust_mut().get_unchecked_mut() }
            .db
            .tracks_by_artist(id);
        match result {
            Ok(tracks) => {
                let count = tracks.len() as i32;
                let json = tracks_to_json(&tracks);
                self.as_mut().set_results_json(QString::from(json.as_str()));
                self.as_mut().set_track_count(count);
                let msg = format!("{count} tracks");
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
            Err(e) => {
                let msg = format!("loadArtistTracks error: {e}");
                self.as_mut().set_status_text(QString::from(msg.as_str()));
            }
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::tracks_to_json;
    use lyra_db::Track;

    fn make_track(id: i64, title: &str, artist: &str) -> Track {
        Track {
            id,
            path: format!("/music/{title}.flac"),
            title: title.to_owned(),
            artist: Some(artist.to_owned()),
            album: Some("Album".to_owned()),
            track_no: Some(1),
            duration_ms: Some(180_000),
            cover_thumb: None,
        }
    }

    #[test]
    fn json_escapes_double_quote_and_backslash() {
        let t = make_track(1, r#"She Said "Hello""#, r"AC\DC");
        let json = tracks_to_json(&[t]);
        // Must be valid JSON parseable by serde_json itself.
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("tracks_to_json must produce valid JSON");
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["title"].as_str().unwrap(), r#"She Said "Hello""#);
        assert_eq!(arr[0]["artist"].as_str().unwrap(), r"AC\DC");
    }

    #[test]
    fn json_empty_slice_produces_empty_array() {
        assert_eq!(tracks_to_json(&[]), "[]");
    }

    #[test]
    fn json_null_optional_fields_become_empty_strings() {
        let t = Track {
            id: 2,
            path: "/p.flac".to_owned(),
            title: "Title".to_owned(),
            artist: None,
            album: None,
            track_no: None,
            duration_ms: None,
            cover_thumb: None,
        };
        let json = tracks_to_json(&[t]);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["artist"].as_str().unwrap(), "");
        assert_eq!(parsed[0]["album"].as_str().unwrap(), "");
        assert_eq!(parsed[0]["durationMs"].as_u64().unwrap(), 0);
    }
}
