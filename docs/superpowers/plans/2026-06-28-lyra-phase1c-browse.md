# Lyra Phase 1C-browse — Album & Artist views

> superpowers:subagent-driven-development. DB queries TDD'd; QML via offscreen + screenshot.

**Goal:** Make the sidebar's **Albums** and **Artists** nav real — browse an album cover-grid and an artist list, drill into a selected album/artist's tracks. (Genres deferred: needs a `genre` column the scan doesn't store yet — note it.)

## Global Constraints
- cxx-qt qproperties are snake_case in QML; invokables `#[cxx_name]`. UI crate `lyra_ui`. No `unwrap`/`expect` outside tests; guard all JSON in QML. Breeze, desktop-grade. Root `/home/andrew/Documents/Personal Projects/lyra` (branch `phase-1c-browse`, quote it). Build via `cmake --build build`.

---

### Task A: DB album/artist queries + Library QObject (TDD on DB)

**Files:** `crates/db/src/{model.rs,queries.rs}`; `crates/ui/src/library.rs`.

- [ ] **A1 — DB types + queries (TDD):** add `pub struct Album { id:i64, title:String, artist:Option<String>, year:Option<i32>, track_count:i64, cover_thumb:Option<String> }` and `pub struct Artist { id:i64, name:String, album_count:i64, track_count:i64 }` (re-export at crate root). Add to `impl Db`:
  - `list_albums() -> Result<Vec<Album>>` — group tracks by album, representative `cover_thumb` = any non-null (e.g. `MAX(t.cover_thumb)`), `track_count = COUNT(t.id)`, join artist name; order by title.
  - `list_artists() -> Result<Vec<Artist>>` — `track_count` + `album_count = COUNT(DISTINCT album_id)`; order by name.
  - `tracks_by_album(album_id:i64) -> Result<Vec<Track>>` (order by disc_no, track_no).
  - `tracks_by_artist(artist_id:i64) -> Result<Vec<Track>>` (order by album title, track_no).
  Add unit tests (in-memory db, upsert a few tracks across 2 albums/2 artists): assert `list_albums()` returns the right count + track_counts + a cover_thumb when present; `list_artists()` album_count/track_count; `tracks_by_album`/`tracks_by_artist` return the right rows in order. Keep existing db tests green.
- [ ] **A2 — Library QObject** (`library.rs`): add qproperties `#[qproperty(QString, albums_json)]`, `#[qproperty(QString, artists_json)]`. Invokables (cxx_name): `loadAlbums()` (→ albums_json: `[{id,title,artist,year,track_count,cover_thumb}]`), `loadArtists()` (→ artists_json: `[{id,name,album_count,track_count}]`), `loadAlbumTracks(id:i64)` and `loadArtistTracks(id:i64)` (→ set the existing `results_json` to those tracks, so the track-list view reuses unchanged). Reuse `tracks_to_json`; add `albums_to_json`/`artists_to_json` (serde_json). No panics.
- [ ] **A3 — build + test + commit.** `cargo test -p lyra-db`; `cargo build -p lyra_ui`. Commit `feat(db): album/artist queries` + `feat(ui): Library album/artist browsing`.

### Task B: QML — Albums grid + Artists list + nav

**Files:** `crates/ui/qml/Main.qml`, maybe new `crates/ui/qml/{AlbumCard.qml,ArtistRow.qml}` (register in `build.rs` `qml_files`).

- [ ] **B1 — view state + nav:** add a `property string view: "songs"` to the window. Sidebar rows set it + load: Songs → `view="songs"; library.loadAll()`; Albums → `view="albums"; library.loadAlbums()`; Artists → `view="artists"; library.loadArtists()`. Highlight the active row. (Genres/Recently Added/Playlists/Sources stay placeholders.)
- [ ] **B2 — Albums grid:** when `view==="albums"`, the main area is a `GridView` of album cards (cover thumbnail via `file://` + fallback rect; title bold; artist dimmed; track_count) from `JSON.parse(library.albums_json)`. Click a card → `library.loadAlbumTracks(id); view="album_detail"` showing that album's tracks (the existing TrackDelegate list) with a back affordance (a header with the album title + a "‹ Albums" button → `view="albums"`).
- [ ] **B3 — Artists list:** when `view==="artists"`, a list from `JSON.parse(library.artists_json)` (name + "N albums · M tracks"). Click → `library.loadArtistTracks(id); view="artist_detail"` showing the tracks + a "‹ Artists" back button.
- [ ] **B4 — keep Songs + search + transport working** in all views (search returns to a songs-style track list; playing from any track list uses `player.playFromList(library.results_json, index)`).
- [ ] **B5 — build + verify:** `cmake --build build`; `QT_QPA_PLATFORM=offscreen timeout 15 ./build/lyra 2>&1 | tee /tmp/lyra-browse.log` clean (no QML errors, 679 tracks). Commit `feat(ui): Albums grid + Artists list with drill-down`.

**Visual gate (owner):** Albums shows a cover grid; clicking an album shows its tracks; Artists lists artists; clicking shows their tracks; Songs + search + play still work. Screenshot for the record.

## Exit Criteria
DB album/artist queries tested green; Library exposes albums/artists + drill-down; QML has working Albums grid + Artists list + nav switching, offscreen-clean. Review → merge. Genres deferred (needs a `genre` column in scan/schema).
