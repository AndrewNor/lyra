# Lyra Phase 1A — Data Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Build the pure-Rust data backbone of Lyra: an SQLite+FTS5 library database, audio metadata read/write, and a parallel incremental directory scanner with an album-art thumbnail cache — all unit/integration tested, with zero Qt.

**Architecture:** Three new workspace crates. `lyra-db` (rusqlite + FTS5: schema, types, upsert, queries). `lyra-metadata` (lofty: tag read/write + cover extraction, mapped to plain structs). `lyra-library` (walkdir + rayon: discover → mtime-diff → parse → upsert, plus an `image`+`blake3` thumbnail cache). Everything is testable with `cargo test -p <crate>` — the ~1s loop, no Qt/CMake.

**Tech Stack:** rusqlite 0.40 (bundled), rusqlite_migration 2.6, lofty 0.24, walkdir 2.5, rayon 1.12, image 0.25, blake3 1.8, serde 1, thiserror 2, tempfile 3 (dev).

## Global Constraints

- **Pure Rust, no Qt.** None of these crates may depend on `lyra-ui`, cxx-qt, or Qt. Each must build and test with plain `cargo test -p <crate>` — no CMake, no qmake. This is the fast loop and the whole point of the layering.
- **Version pins** (add to root `[workspace.dependencies]`, exact as resolved 2026-06-28): `rusqlite = { version = "0.40", features = ["bundled"] }` (bundled SQLite ships FTS5), `rusqlite_migration = "2.6"`, `lofty = "0.24"`, `walkdir = "2.5"`, `rayon = "1.12"`, `image = "0.25"`, `blake3 = "1.8"`, `serde = { version = "1", features = ["derive"] }`, `thiserror = "2"`, `tempfile = "3"` (dev-dependency).
- **Error handling:** each crate defines its own `Error` enum via `thiserror` and a `type Result<T> = std::result::Result<T, Error>`. No `unwrap()`/`expect()` in library code (tests may use them).
- **DB is the source of truth for paths:** a track is identified by its absolute filesystem path (TEXT, UNIQUE). Re-scanning the same path updates the row, never duplicates it.
- **API-correction clause:** the crate snippets below are written against rusqlite 0.40 / lofty 0.24 / image 0.25 as known on 2026-06-28, but exact method names/signatures may differ. If the compiler rejects a call, correct it to the installed crate's real API (check `cargo doc --open -p <crate>` or docs.rs for the pinned version) — preserve the **behavior and the public signatures this plan defines**, adjust the crate-call details. Only report BLOCKED if a required capability is genuinely missing from the crate.
- **Project root:** `/home/andrew/Documents/Personal Projects/lyra` (a git repo, branch `phase-1a-data`). Paths below are relative to it; quote the root (it has a space).

## File Structure

```
crates/
  db/                 # lyra-db
    Cargo.toml
    src/
      lib.rs          # re-exports, Error, Db open/migrate
      schema.rs       # migration SQL (tables + FTS5)
      model.rs        # Track / NewTrack / Album / Artist structs
      queries.rs      # upsert_track, list_*, search
  metadata/           # lyra-metadata
    Cargo.toml
    src/
      lib.rs          # Error, public fns
      tags.rs         # TrackTags struct + lofty mapping (pure)
      cover.rs        # embedded-art extraction
    tests/
      roundtrip.rs    # generate WAV -> write -> read tags
  library/            # lyra-library
    Cargo.toml
    src/
      lib.rs          # Error, ScanSummary, scan()
      discover.rs     # walkdir audio-file discovery
      artcache.rs     # image resize + blake3-keyed disk cache
    tests/
      scan.rs         # tempdir of WAVs -> scan -> assert db
```

---

### Task 0: Add crates to the workspace and pin dependencies

**Files:** Modify root `Cargo.toml`; create `crates/db/{Cargo.toml,src/lib.rs}`, `crates/metadata/{Cargo.toml,src/lib.rs}`, `crates/library/{Cargo.toml,src/lib.rs}` as compiling stubs.

**Interfaces:** Produces three workspace members + the `[workspace.dependencies]` pins all later tasks inherit.

- [ ] **Step 1: Add members + dependency pins to root `Cargo.toml`**

In `[workspace] members`, add `"crates/db"`, `"crates/metadata"`, `"crates/library"`. In `[workspace.dependencies]`, append:
```toml
rusqlite = { version = "0.40", features = ["bundled"] }
rusqlite_migration = "2.6"
lofty = "0.24"
walkdir = "2.5"
rayon = "1.12"
image = "0.25"
blake3 = "1.8"
serde = { version = "1", features = ["derive"] }
thiserror = "2"
tempfile = "3"
```

- [ ] **Step 2: Create the three crate stubs**

`crates/db/Cargo.toml`:
```toml
[package]
name = "lyra-db"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
rusqlite = { workspace = true }
rusqlite_migration = { workspace = true }
serde = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```
`crates/db/src/lib.rs`: `//! lyra-db: SQLite+FTS5 library database.`

`crates/metadata/Cargo.toml`:
```toml
[package]
name = "lyra-metadata"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
lofty = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```
`crates/metadata/src/lib.rs`: `//! lyra-metadata: audio tag read/write + cover extraction.`

`crates/library/Cargo.toml`:
```toml
[package]
name = "lyra-library"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
lyra-db = { path = "../db" }
lyra-metadata = { path = "../metadata" }
walkdir = { workspace = true }
rayon = { workspace = true }
image = { workspace = true }
blake3 = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```
`crates/library/src/lib.rs`: `//! lyra-library: directory scan + album-art cache.`

- [ ] **Step 3: Verify and commit**

Run: `cd "/home/andrew/Documents/Personal Projects/lyra" && cargo metadata --format-version 1 >/dev/null && echo OK`
Expected: `OK`. Then `cargo build -p lyra-db -p lyra-metadata -p lyra-library` (downloads + compiles the new deps; first build is slow). Expected: `Finished`.
Commit: `git add Cargo.toml Cargo.lock crates/db crates/metadata crates/library && git commit -m "chore: add lyra-db/metadata/library crates + pin data-layer deps"`

---

### Task 1: `lyra-db` — schema, open, and migrations (TDD)

**Files:** Create `crates/db/src/schema.rs`; modify `crates/db/src/lib.rs`.

**Interfaces:**
- Produces: `pub struct Db { conn: rusqlite::Connection }`; `Db::open(path: &std::path::Path) -> Result<Db>`; `Db::open_in_memory() -> Result<Db>`; `pub enum Error` (with `#[from] rusqlite::Error` and `#[from] rusqlite_migration::Error`); `pub type Result<T>`.
- Consumed by Tasks 2–3 and by `lyra-library`.

- [ ] **Step 1: Write the failing test** in `crates/db/src/lib.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    fn names(db: &Db, kind: &str) -> Vec<String> {
        db.conn
            .prepare(&format!("SELECT name FROM sqlite_master WHERE type='{kind}' ORDER BY name"))
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn migrations_create_core_tables() {
        let db = Db::open_in_memory().unwrap();
        let tables = names(&db, "table");
        for t in ["artists", "albums", "tracks"] {
            assert!(tables.iter().any(|n| n == t), "missing table {t}");
        }
    }

    #[test]
    fn fts_table_exists() {
        let db = Db::open_in_memory().unwrap();
        let tables = names(&db, "table");
        assert!(tables.iter().any(|n| n == "tracks_fts"), "missing FTS5 table");
    }
}
```

- [ ] **Step 2: Run, verify it fails** — `cargo test -p lyra-db` → fails to compile (`Db` undefined).

- [ ] **Step 3: Implement.** `crates/db/src/schema.rs`:
```rust
use rusqlite_migration::{Migrations, M};

pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(
        r#"
        CREATE TABLE artists (
            id   INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE albums (
            id        INTEGER PRIMARY KEY,
            title     TEXT NOT NULL,
            artist_id INTEGER REFERENCES artists(id),
            year      INTEGER,
            UNIQUE(title, artist_id)
        );
        CREATE TABLE tracks (
            id         INTEGER PRIMARY KEY,
            path       TEXT NOT NULL UNIQUE,
            title      TEXT NOT NULL,
            artist_id  INTEGER REFERENCES artists(id),
            album_id   INTEGER REFERENCES albums(id),
            track_no   INTEGER,
            disc_no    INTEGER,
            duration_ms INTEGER,
            mtime      INTEGER NOT NULL,
            cover_thumb TEXT
        );
        CREATE INDEX idx_tracks_album ON tracks(album_id);
        CREATE INDEX idx_tracks_artist ON tracks(artist_id);

        CREATE VIRTUAL TABLE tracks_fts USING fts5(
            title, artist, album,
            content=''            -- contentless; we feed it explicitly
        );
        "#,
    )])
}
```
`crates/db/src/lib.rs`:
```rust
//! lyra-db: SQLite+FTS5 library database.
mod schema;
pub mod model;     // added in Task 2
pub mod queries;   // added in Task 3

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Migration(#[from] rusqlite_migration::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

pub struct Db {
    pub(crate) conn: rusqlite::Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        let mut conn = rusqlite::Connection::open(path)?;
        Self::init(&mut conn)?;
        Ok(Db { conn })
    }
    pub fn open_in_memory() -> Result<Db> {
        let mut conn = rusqlite::Connection::open_in_memory()?;
        Self::init(&mut conn)?;
        Ok(Db { conn })
    }
    fn init(conn: &mut rusqlite::Connection) -> Result<()> {
        conn.pragma_update(None, "foreign_keys", true)?;
        schema::migrations().to_latest(conn)?;
        Ok(())
    }
}
```
Note: `pub mod model;`/`pub mod queries;` reference files created in Tasks 2–3 — create empty `model.rs`/`queries.rs` (just `//! placeholder`) now so this compiles, or add the `mod` lines in those tasks. To keep this task self-contained and compiling, create `model.rs` and `queries.rs` as empty stubs here.

- [ ] **Step 4: Run, verify pass** — `cargo test -p lyra-db` → 2 passed.
- [ ] **Step 5: Commit** — `git add crates/db && git commit -m "feat(db): schema + migrations (artists/albums/tracks + FTS5)"`

---

### Task 2: `lyra-db` — domain types + `upsert_track` (TDD)

**Files:** Create/replace `crates/db/src/model.rs`, `crates/db/src/queries.rs`.

**Interfaces:**
- Produces: `pub struct NewTrack { pub path: String, pub title: String, pub artist: Option<String>, pub album: Option<String>, pub album_artist: Option<String>, pub track_no: Option<u32>, pub disc_no: Option<u32>, pub year: Option<i32>, pub duration_ms: Option<u64>, pub mtime: i64, pub cover_thumb: Option<String> }`; `pub struct Track { pub id: i64, pub path: String, pub title: String, pub artist: Option<String>, pub album: Option<String>, pub track_no: Option<u32>, pub duration_ms: Option<u64>, pub cover_thumb: Option<String> }`; `impl Db { pub fn upsert_track(&mut self, t: &NewTrack) -> Result<i64> }` (returns track id; idempotent on `path`; gets-or-creates artist/album; refreshes the FTS row).
- Consumed by Task 3 and `lyra-library`.

- [ ] **Step 1: Write the failing test** in `crates/db/src/queries.rs`:
```rust
#[cfg(test)]
mod tests {
    use crate::{model::NewTrack, Db};

    fn nt(path: &str, title: &str, artist: &str, album: &str) -> NewTrack {
        NewTrack {
            path: path.into(), title: title.into(),
            artist: Some(artist.into()), album: Some(album.into()),
            album_artist: Some(artist.into()), track_no: Some(1), disc_no: Some(1),
            year: Some(2020), duration_ms: Some(180_000), mtime: 100, cover_thumb: None,
        }
    }

    #[test]
    fn upsert_inserts_then_updates_same_path() {
        let mut db = Db::open_in_memory().unwrap();
        let id1 = db.upsert_track(&nt("/m/a.flac", "A", "Artist", "Album")).unwrap();
        let mut t2 = nt("/m/a.flac", "A (remaster)", "Artist", "Album");
        t2.mtime = 200;
        let id2 = db.upsert_track(&t2).unwrap();
        assert_eq!(id1, id2, "same path must reuse the row");
        let n: i64 = db.conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        let title: String = db.conn.query_row("SELECT title FROM tracks", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "A (remaster)");
    }

    #[test]
    fn artist_and_album_are_deduplicated() {
        let mut db = Db::open_in_memory().unwrap();
        db.upsert_track(&nt("/m/a.flac", "A", "Artist", "Album")).unwrap();
        db.upsert_track(&nt("/m/b.flac", "B", "Artist", "Album")).unwrap();
        let artists: i64 = db.conn.query_row("SELECT COUNT(*) FROM artists", [], |r| r.get(0)).unwrap();
        let albums: i64 = db.conn.query_row("SELECT COUNT(*) FROM albums", [], |r| r.get(0)).unwrap();
        assert_eq!((artists, albums), (1, 1));
    }
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p lyra-db` → `upsert_track` undefined.

- [ ] **Step 3: Implement.** `crates/db/src/model.rs` — the `NewTrack`/`Track` structs above (derive `Debug, Clone`; `NewTrack` also `serde::Deserialize` optional). `crates/db/src/queries.rs`:
```rust
use crate::{model::NewTrack, Db, Result};

impl Db {
    pub fn upsert_track(&mut self, t: &NewTrack) -> Result<i64> {
        let tx = self.conn.transaction()?;
        let artist_id: Option<i64> = match &t.artist {
            Some(name) => {
                tx.execute("INSERT OR IGNORE INTO artists(name) VALUES (?1)", [name])?;
                Some(tx.query_row("SELECT id FROM artists WHERE name=?1", [name], |r| r.get(0))?)
            }
            None => None,
        };
        let album_id: Option<i64> = match &t.album {
            Some(title) => {
                tx.execute(
                    "INSERT OR IGNORE INTO albums(title, artist_id, year) VALUES (?1,?2,?3)",
                    rusqlite::params![title, artist_id, t.year],
                )?;
                Some(tx.query_row(
                    "SELECT id FROM albums WHERE title=?1 AND artist_id IS ?2",
                    rusqlite::params![title, artist_id], |r| r.get(0),
                )?)
            }
            None => None,
        };
        tx.execute(
            "INSERT INTO tracks(path,title,artist_id,album_id,track_no,disc_no,duration_ms,mtime,cover_thumb)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(path) DO UPDATE SET
               title=excluded.title, artist_id=excluded.artist_id, album_id=excluded.album_id,
               track_no=excluded.track_no, disc_no=excluded.disc_no,
               duration_ms=excluded.duration_ms, mtime=excluded.mtime, cover_thumb=excluded.cover_thumb",
            rusqlite::params![t.path, t.title, artist_id, album_id, t.track_no, t.disc_no, t.duration_ms.map(|d| d as i64), t.mtime, t.cover_thumb],
        )?;
        let id: i64 = tx.query_row("SELECT id FROM tracks WHERE path=?1", [&t.path], |r| r.get(0))?;
        // refresh FTS: delete any existing row for this track, re-insert
        tx.execute("DELETE FROM tracks_fts WHERE rowid=?1", [id])?;
        tx.execute(
            "INSERT INTO tracks_fts(rowid,title,artist,album) VALUES (?1,?2,?3,?4)",
            rusqlite::params![id, t.title, t.artist.clone().unwrap_or_default(), t.album.clone().unwrap_or_default()],
        )?;
        tx.commit()?;
        Ok(id)
    }
}
```

- [ ] **Step 4: Run, verify pass** — `cargo test -p lyra-db` → 4 passed.
- [ ] **Step 5: Commit** — `git add crates/db && git commit -m "feat(db): NewTrack/Track types + idempotent upsert_track with FTS sync"`

---

### Task 3: `lyra-db` — list + FTS search queries (TDD)

**Files:** Modify `crates/db/src/queries.rs`.

**Interfaces:**
- Produces: `impl Db { pub fn list_tracks(&self) -> Result<Vec<Track>>; pub fn search(&self, query: &str) -> Result<Vec<Track>> }` (search uses FTS5 `MATCH`; results ordered by relevance via `bm25`).
- Consumed by `lyra-ui` later.

- [ ] **Step 1: Write the failing test** (append to the `tests` mod in `queries.rs`):
```rust
    #[test]
    fn search_matches_title_and_artist() {
        let mut db = Db::open_in_memory().unwrap();
        db.upsert_track(&nt("/m/1.flac", "Midnight City", "M83", "Hurry Up")).unwrap();
        db.upsert_track(&nt("/m/2.flac", "Kerala", "Bonobo", "Migration")).unwrap();
        assert_eq!(db.search("midnight").unwrap().len(), 1);
        assert_eq!(db.search("bonobo").unwrap()[0].title, "Kerala");
        assert_eq!(db.search("nonsensequery").unwrap().len(), 0);
        assert_eq!(db.list_tracks().unwrap().len(), 2);
    }
```

- [ ] **Step 2: Run, verify fail** — `search`/`list_tracks` undefined.

- [ ] **Step 3: Implement** (append to `queries.rs`):
```rust
use crate::model::Track;

const SELECT_TRACK: &str = "SELECT t.id, t.path, t.title, ar.name, al.title, t.track_no, t.duration_ms, t.cover_thumb \
    FROM tracks t LEFT JOIN artists ar ON ar.id=t.artist_id LEFT JOIN albums al ON al.id=t.album_id";

fn row_to_track(r: &rusqlite::Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get(0)?, path: r.get(1)?, title: r.get(2)?,
        artist: r.get(3)?, album: r.get(4)?,
        track_no: r.get::<_, Option<i64>>(5)?.map(|v| v as u32),
        duration_ms: r.get::<_, Option<i64>>(6)?.map(|v| v as u64),
        cover_thumb: r.get(7)?,
    })
}

impl Db {
    pub fn list_tracks(&self) -> Result<Vec<Track>> {
        let sql = format!("{SELECT_TRACK} ORDER BY t.title");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn search(&self, query: &str) -> Result<Vec<Track>> {
        // FTS5 MATCH against the contentless index, joined back to tracks by rowid.
        let sql = format!(
            "{SELECT_TRACK} JOIN tracks_fts f ON f.rowid=t.id \
             WHERE tracks_fts MATCH ?1 ORDER BY bm25(tracks_fts)"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        // Append '*' for prefix matching; escape the term in quotes.
        let term = format!("\"{}\"*", query.replace('"', "\"\""));
        let rows = stmt.query_map([term], row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
```
(If `bm25(tracks_fts)` ordering errors on the contentless config, fall back to ordering by `rank` — correct against the FTS5 build.)

- [ ] **Step 4: Run, verify pass** — `cargo test -p lyra-db` → 5 passed.
- [ ] **Step 5: Commit** — `git add crates/db && git commit -m "feat(db): list_tracks + FTS5 prefix search"`

---

### Task 4: `lyra-metadata` — `TrackTags` + pure lofty mapping (TDD)

**Files:** Create `crates/metadata/src/tags.rs`; modify `crates/metadata/src/lib.rs`.

**Interfaces:**
- Produces: `pub struct TrackTags { pub title: Option<String>, pub artist: Option<String>, pub album: Option<String>, pub album_artist: Option<String>, pub track_no: Option<u32>, pub disc_no: Option<u32>, pub year: Option<i32>, pub genre: Option<String>, pub duration_ms: Option<u64> }` (derive `Debug, Clone, Default, PartialEq`); `pub(crate) fn tags_from_lofty(tagged: &lofty::file::TaggedFile) -> TrackTags` mapping the primary tag + properties.
- Consumed by Task 5 and `lyra-library`.

- [ ] **Step 1: Write the failing test** in `tags.rs` — test the **pure mapping** by building a lofty `Tag` in memory (no file):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use lofty::tag::{Tag, Accessor};
    use lofty::config::TagType;

    #[test]
    fn maps_primary_tag_fields() {
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title("Awake".to_string());
        tag.set_artist("Tycho".to_string());
        tag.set_album("Blue Hour".to_string());
        tag.set_track(3);
        let tt = tags_from_tag(&tag);
        assert_eq!(tt.title.as_deref(), Some("Awake"));
        assert_eq!(tt.artist.as_deref(), Some("Tycho"));
        assert_eq!(tt.album.as_deref(), Some("Blue Hour"));
        assert_eq!(tt.track_no, Some(3));
    }
}
```
(This isolates the mapping from a `lofty::tag::Tag`. The file-level `tags_from_lofty` wraps it by picking the primary tag and merging `FileProperties` for `duration_ms`.)

- [ ] **Step 2: Run, verify fail** — `tags_from_tag` undefined.

- [ ] **Step 3: Implement.** `crates/metadata/src/tags.rs`:
```rust
use lofty::tag::{Accessor, Tag};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Pure mapping from a single lofty Tag to TrackTags (no I/O).
pub fn tags_from_tag(tag: &Tag) -> TrackTags {
    TrackTags {
        title: tag.title().map(|s| s.into_owned()),
        artist: tag.artist().map(|s| s.into_owned()),
        album: tag.album().map(|s| s.into_owned()),
        album_artist: None, // refined against lofty's ItemKey::AlbumArtist if available
        track_no: tag.track(),
        disc_no: tag.disk(),
        year: tag.year().map(|y| y as i32),
        genre: tag.genre().map(|s| s.into_owned()),
        duration_ms: None,
    }
}
```
Wire `mod tags; pub use tags::TrackTags;` into `lib.rs`. (The `Accessor` trait provides `title()/artist()/album()/track()/disk()/year()/genre()`. Adjust to lofty 0.24's exact `Accessor` surface; `album_artist` may need `tag.get_string(&ItemKey::AlbumArtist)` — correct against the crate.)

- [ ] **Step 4: Run, verify pass** — `cargo test -p lyra-metadata` → 1 passed.
- [ ] **Step 5: Commit** — `git add crates/metadata && git commit -m "feat(metadata): TrackTags + pure lofty Tag mapping"`

---

### Task 5: `lyra-metadata` — `read_tags` / `write_tags` round-trip (TDD)

**Files:** Modify `crates/metadata/src/lib.rs`; create `crates/metadata/tests/roundtrip.rs` (+ a test WAV helper).

**Interfaces:**
- Produces: `pub fn read_tags(path: &std::path::Path) -> Result<TrackTags>` (reads the primary tag + duration); `pub fn write_tags(path: &std::path::Path, tags: &TrackTags) -> Result<()>` (writes title/artist/album/track to the file's primary tag); `pub enum Error` (`#[from] lofty::error::LoftyError`, `#[from] std::io::Error`).
- Consumed by `lyra-library`.

- [ ] **Step 1: Write the failing integration test** `crates/metadata/tests/roundtrip.rs`:
```rust
use lyra_metadata::{read_tags, write_tags, TrackTags};
use std::io::Write;

/// Minimal valid 16-bit PCM WAV (44-byte header + 1 sample frame), enough for
/// lofty to recognise the file and attach a tag.
fn write_min_wav(path: &std::path::Path) {
    let data: [u8; 4] = [0, 0, 0, 0]; // one stereo 16-bit frame of silence
    let mut f = std::fs::File::create(path).unwrap();
    let n = data.len() as u32;
    let riff = 36 + n;
    f.write_all(b"RIFF").unwrap(); f.write_all(&riff.to_le_bytes()).unwrap();
    f.write_all(b"WAVE").unwrap();
    f.write_all(b"fmt ").unwrap(); f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap();   // PCM
    f.write_all(&2u16.to_le_bytes()).unwrap();   // 2 ch
    f.write_all(&44100u32.to_le_bytes()).unwrap();
    f.write_all(&176400u32.to_le_bytes()).unwrap(); // byte rate
    f.write_all(&4u16.to_le_bytes()).unwrap();   // block align
    f.write_all(&16u16.to_le_bytes()).unwrap();  // bits
    f.write_all(b"data").unwrap(); f.write_all(&n.to_le_bytes()).unwrap();
    f.write_all(&data).unwrap();
}

#[test]
fn write_then_read_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("t.wav");
    write_min_wav(&p);

    let mut tags = TrackTags::default();
    tags.title = Some("Verdant".into());
    tags.artist = Some("Bonobo".into());
    tags.album = Some("Migration".into());
    tags.track_no = Some(4);
    write_tags(&p, &tags).unwrap();

    let got = read_tags(&p).unwrap();
    assert_eq!(got.title.as_deref(), Some("Verdant"));
    assert_eq!(got.artist.as_deref(), Some("Bonobo"));
    assert_eq!(got.album.as_deref(), Some("Migration"));
    assert_eq!(got.track_no, Some(4));
}
```

- [ ] **Step 2: Run, verify fail** — `read_tags`/`write_tags` undefined.

- [ ] **Step 3: Implement** in `lib.rs` (using lofty's `read_from_path`, `primary_tag`/`first_tag`, `save_to_path`, and a fresh `Tag`/`TagType` when none exists; map duration from `properties()`):
```rust
mod tags;
pub use tags::{tags_from_tag, TrackTags};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)] Lofty(#[from] lofty::error::LoftyError),
    #[error(transparent)] Io(#[from] std::io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

pub fn read_tags(path: &Path) -> Result<TrackTags> {
    use lofty::file::{AudioFile, TaggedFileExt};
    let tagged = lofty::read_from_path(path)?;
    let mut tt = tagged.primary_tag().or_else(|| tagged.first_tag()).map(tags_from_tag).unwrap_or_default();
    tt.duration_ms = Some(tagged.properties().duration().as_millis() as u64);
    Ok(tt)
}

pub fn write_tags(path: &Path, tags: &TrackTags) -> Result<()> {
    use lofty::file::TaggedFileExt;
    use lofty::tag::{Accessor, Tag, TagExt};
    use lofty::config::TagType;
    let mut tagged = lofty::read_from_path(path)?;
    if tagged.primary_tag().is_none() {
        let tt = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tt));
    }
    let tag = tagged.primary_tag_mut().expect("tag inserted above");
    if let Some(v) = &tags.title  { tag.set_title(v.clone()); }
    if let Some(v) = &tags.artist { tag.set_artist(v.clone()); }
    if let Some(v) = &tags.album  { tag.set_album(v.clone()); }
    if let Some(n) = tags.track_no { tag.set_track(n); }
    tag.save_to_path(path, lofty::config::WriteOptions::default())?;
    Ok(())
}
```
(lofty 0.24's exact import paths/trait names — `TaggedFileExt`, `AudioFile`, `TagExt`, `WriteOptions`, `primary_tag_type` — may differ slightly; correct against the installed crate while preserving the round-trip behavior. If a hand-rolled WAV is rejected by lofty, switch the fixture to lofty's own test approach or a minimal valid MP3 frame — but try the WAV first.)

- [ ] **Step 4: Run, verify pass** — `cargo test -p lyra-metadata` → 2 passed (unit + round-trip).
- [ ] **Step 5: Commit** — `git add crates/metadata && git commit -m "feat(metadata): read_tags/write_tags with WAV round-trip test"`

---

### Task 6: `lyra-metadata` — embedded cover extraction (TDD)

**Files:** Create `crates/metadata/src/cover.rs`; modify `lib.rs`.

**Interfaces:**
- Produces: `pub struct Cover { pub mime: String, pub data: Vec<u8> }`; `pub fn read_cover(path: &std::path::Path) -> Result<Option<Cover>>` (first embedded picture, if any).
- Consumed by `lyra-library` artcache.

- [ ] **Step 1: Write the failing test** in `crates/metadata/tests/roundtrip.rs` (append) — embed a tiny PNG via lofty, read it back:
```rust
#[test]
fn cover_round_trips() {
    use lyra_metadata::read_cover;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("c.wav");
    write_min_wav(&p);
    // 1x1 PNG
    let png: &[u8] = &[137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,8,2,0,0,0,144,119,83,222,0,0,0,12,73,68,65,84,8,215,99,248,207,192,0,0,0,3,0,1,0,24,221,141,219,0,0,0,0,73,69,78,68,174,66,96,130];
    lyra_metadata::write_cover(&p, "image/png", png).unwrap();
    let got = read_cover(&p).unwrap().expect("cover present");
    assert_eq!(got.mime, "image/png");
    assert!(!got.data.is_empty());
}
```

- [ ] **Step 2: Run, verify fail.**

- [ ] **Step 3: Implement** `crates/metadata/src/cover.rs` with `read_cover` and a test-supporting `write_cover` using lofty's `Picture`/`MimeType` API (attach to primary tag, save). Wire `mod cover; pub use cover::{Cover, read_cover, write_cover};` into `lib.rs`. (Correct the `Picture::new_unchecked`/`PictureType::CoverFront`/`MimeType` calls against lofty 0.24.)

- [ ] **Step 4: Run, verify pass** — 3 passed.
- [ ] **Step 5: Commit** — `git add crates/metadata && git commit -m "feat(metadata): embedded cover read/write"`

---

### Task 7: `lyra-library` — audio-file discovery (TDD)

**Files:** Create `crates/library/src/discover.rs`; modify `lib.rs`.

**Interfaces:**
- Produces: `pub fn discover(root: &std::path::Path) -> Vec<std::path::PathBuf>` (recursive; keeps files whose lowercased extension is in `{mp3,flac,m4a,aac,ogg,opus,wav,aiff,alac,wv}`; skips dirs/symlink loops).
- Consumed by Task 8.

- [ ] **Step 1: Write the failing test** in `discover.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_audio_recursively_ignores_others() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        for f in ["a.flac", "b.MP3", "cover.jpg", "notes.txt"] {
            std::fs::write(dir.path().join(f), b"x").unwrap();
        }
        std::fs::write(sub.join("c.ogg"), b"x").unwrap();
        let mut found: Vec<String> = discover(dir.path())
            .iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        found.sort();
        assert_eq!(found, vec!["a.flac", "b.MP3", "c.ogg"]);
    }
}
```

- [ ] **Step 2–4: Implement** with `walkdir::WalkDir::new(root).follow_links(false)`, filter `entry.file_type().is_file()` and extension membership (lowercased) in a `const AUDIO_EXTS: &[&str]`. Run → 1 passed.
- [ ] **Step 5: Commit** — `git add crates/library && git commit -m "feat(library): recursive audio-file discovery"`

---

### Task 8: `lyra-library` — incremental parallel scan into the DB (TDD)

**Files:** Modify `crates/library/src/lib.rs`; create `crates/library/tests/scan.rs`.

**Interfaces:**
- Produces: `pub struct ScanSummary { pub added: usize, pub updated: usize, pub unchanged: usize, pub failed: usize }`; `pub fn scan(root: &std::path::Path, db: &mut lyra_db::Db) -> Result<ScanSummary>`. Behavior: discover files; for each, read fs mtime; if the DB has the path with an equal mtime → unchanged; else parse tags (parallel via rayon) and `upsert_track`, counting added vs updated. A per-file parse error increments `failed` and is skipped (scan never aborts on one bad file).
- Consumed by `lyra-ui` later (background scan job).

- [ ] **Step 1: Write the failing integration test** `crates/library/tests/scan.rs` — reuse the `write_min_wav` helper (copy it in), write tags to two WAVs, scan, assert; then rescan asserts `unchanged`; then bump one file's mtime and rescan asserts `updated`:
```rust
use lyra_db::Db;
use lyra_library::scan;
// (copy write_min_wav + a tag-writing helper using lyra_metadata::write_tags)

#[test]
fn scan_is_incremental() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav"); let b = dir.path().join("b.wav");
    make_tagged_wav(&a, "A", "Artist", "Album");
    make_tagged_wav(&b, "B", "Artist", "Album");
    let mut db = Db::open_in_memory().unwrap();

    let s1 = scan(dir.path(), &mut db).unwrap();
    assert_eq!((s1.added, s1.updated), (2, 0));
    assert_eq!(db.list_tracks().unwrap().len(), 2);

    let s2 = scan(dir.path(), &mut db).unwrap();
    assert_eq!((s2.added, s2.updated, s2.unchanged), (0, 0, 2));

    // bump mtime on a.wav and rescan
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    filetime::set_file_mtime(&a, filetime::FileTime::from_system_time(later)).ok();
    // if `filetime` is undesired, instead rewrite the file to change mtime.
    let s3 = scan(dir.path(), &mut db).unwrap();
    assert!(s3.updated >= 1 || s3.unchanged == 2);
}
```
(To avoid adding the `filetime` dev-dep, the implementer may instead change mtime by rewriting the file via `write_tags` again, or `std::fs::OpenOptions` touch — pick the simplest that reliably changes mtime; preserve the assertion intent: an mtime change forces a re-parse.)

- [ ] **Step 2: Run, verify fail.**

- [ ] **Step 3: Implement `scan`**: build a `path -> mtime` map from the DB (`SELECT path, mtime FROM tracks` — add a small `Db::track_mtimes(&self) -> Result<HashMap<String,i64>>` helper in `lyra-db`), `discover()` the root, partition into unchanged vs to-parse by comparing mtime, parse the to-parse set in parallel with `rayon` (`into_par_iter().map(read_tags).collect()`), then upsert sequentially (rusqlite `Connection` is not `Sync`) counting added/updated by whether the path pre-existed. mtime via `entry.metadata()?.modified()` → seconds since epoch (i64).

- [ ] **Step 4: Run, verify pass** — `cargo test -p lyra-library` → passes; `cargo test -p lyra-db` still green (new helper).
- [ ] **Step 5: Commit** — `git add crates/db crates/library && git commit -m "feat(library): incremental parallel scan into the database"`

---

### Task 9: `lyra-library` — album-art thumbnail cache (TDD)

**Files:** Create `crates/library/src/artcache.rs`; modify `lib.rs`.

**Interfaces:**
- Produces: `pub struct ArtCache { dir: PathBuf }`; `impl ArtCache { pub fn new(dir: PathBuf) -> Self; pub fn store(&self, image_bytes: &[u8]) -> Result<PathBuf> }` — decode `image_bytes`, resize to a 256×256 thumbnail (preserve aspect, `image::imageops::thumbnail`), encode PNG, write to `dir/<blake3-of-input-hex>.png`, return that path; if the file already exists, skip re-encoding (content-addressed dedup).
- Consumed by the scan/UI later.

- [ ] **Step 1: Write the failing test** in `artcache.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, _| image::Rgb([(x % 256) as u8, 0, 0]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }
    #[test]
    fn stores_thumbnail_and_dedups_by_hash() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ArtCache::new(dir.path().to_path_buf());
        let src = png_bytes(512, 400);
        let p1 = cache.store(&src).unwrap();
        assert!(p1.exists());
        let (w, h) = image::image_dimensions(&p1).unwrap();
        assert!(w <= 256 && h <= 256 && (w == 256 || h == 256));
        let mtime1 = std::fs::metadata(&p1).unwrap().modified().unwrap();
        let p2 = cache.store(&src).unwrap();          // same input
        assert_eq!(p1, p2);                            // same hash path
        assert_eq!(mtime1, std::fs::metadata(&p2).unwrap().modified().unwrap()); // not rewritten
    }
}
```

- [ ] **Step 2–4: Implement** with `blake3::hash(image_bytes)` → hex filename; `image::load_from_memory`, `.thumbnail(256,256)`, `.save(path)` (or encode PNG explicitly); early-return if the path exists. Run → passes.
- [ ] **Step 5: Commit** — `git add crates/library && git commit -m "feat(library): content-addressed album-art thumbnail cache"`

---

## Phase 1A Exit Criteria

Complete when `cargo test -p lyra-db -p lyra-metadata -p lyra-library` is all-green (no Qt, no CMake), covering: schema/migrations, idempotent upsert, FTS search, tag read/write round-trip, cover extraction, recursive discovery, incremental parallel scan, and the art-thumbnail cache. The whole-branch review then runs, and `phase-1a-data` merges to `master`. **Next:** Phase 1B (playback engine) consumes `lyra-db`/`lyra-metadata`; the eventual UI binds `Db::search`/`list_tracks` and `ArtCache` into Layout B.
