//! Database query implementations.

use crate::{model::{Album, Artist, Genre, NewTrack, Playlist, SmartPlaylist, Track}, Db, Result};

const SELECT_TRACK: &str = "SELECT t.id, t.path, t.title, ar.name, al.title, t.track_no, t.duration_ms, t.cover_thumb \
    FROM tracks t LEFT JOIN artists ar ON ar.id=t.artist_id LEFT JOIN albums al ON al.id=t.album_id";

fn row_to_track(r: &rusqlite::Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get(0)?,
        path: r.get(1)?,
        title: r.get(2)?,
        artist: r.get(3)?,
        album: r.get(4)?,
        track_no: r.get::<_, Option<i64>>(5)?.map(|v| v as u32),
        duration_ms: r.get::<_, Option<i64>>(6)?.map(|v| v as u64),
        cover_thumb: r.get(7)?,
    })
}

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
            "INSERT INTO tracks(path,title,artist_id,album_id,track_no,disc_no,duration_ms,mtime,cover_thumb,genre)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(path) DO UPDATE SET
               title=excluded.title, artist_id=excluded.artist_id, album_id=excluded.album_id,
               track_no=excluded.track_no, disc_no=excluded.disc_no,
               duration_ms=excluded.duration_ms, mtime=excluded.mtime, cover_thumb=excluded.cover_thumb,
               genre=excluded.genre",
            rusqlite::params![
                t.path, t.title, artist_id, album_id,
                t.track_no, t.disc_no,
                t.duration_ms.map(|d| d as i64),
                t.mtime, t.cover_thumb, t.genre
            ],
        )?;
        let id: i64 = tx.query_row("SELECT id FROM tracks WHERE path=?1", [&t.path], |r| r.get(0))?;
        // Refresh FTS: delete any existing row for this track, then re-insert.
        tx.execute("DELETE FROM tracks_fts WHERE rowid=?1", [id])?;
        tx.execute(
            "INSERT INTO tracks_fts(rowid,title,artist,album) VALUES (?1,?2,?3,?4)",
            rusqlite::params![
                id, t.title,
                t.artist.clone().unwrap_or_default(),
                t.album.clone().unwrap_or_default()
            ],
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn list_tracks(&self) -> Result<Vec<Track>> {
        let sql = format!("{SELECT_TRACK} ORDER BY t.title");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Returns a map of `path -> mtime` for all tracks in the database.
    pub fn track_mtimes(&self) -> Result<std::collections::HashMap<String, i64>> {
        let mut stmt = self.conn.prepare("SELECT path, mtime FROM tracks")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let (path, mtime) = row?;
            map.insert(path, mtime);
        }
        Ok(map)
    }

    pub fn search(&self, query: &str) -> Result<Vec<Track>> {
        // FTS5 MATCH against the contentless index, joined back to tracks by rowid.
        let sql = format!(
            "{SELECT_TRACK} JOIN tracks_fts f ON f.rowid=t.id \
             WHERE tracks_fts MATCH ?1 ORDER BY rank"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        // Append '*' for prefix matching; escape the term in quotes.
        let term = format!("\"{}\"*", query.replace('"', "\"\""));
        let rows = stmt.query_map([term], row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Returns all albums with joined artist name and per-album track count / cover.
    /// `cover_thumb` is the MAX (any non-null) cover stored on the album's tracks.
    pub fn list_albums(&self) -> Result<Vec<Album>> {
        let mut stmt = self.conn.prepare(
            "SELECT al.id, al.title, ar.name, al.year, \
                    COUNT(t.id) AS track_count, MAX(t.cover_thumb) AS cover_thumb \
             FROM albums al \
             LEFT JOIN artists ar ON ar.id = al.artist_id \
             LEFT JOIN tracks  t  ON t.album_id = al.id \
             GROUP BY al.id \
             ORDER BY al.title",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Album {
                id:          r.get(0)?,
                title:       r.get(1)?,
                artist:      r.get(2)?,
                year:        r.get(3)?,
                track_count: r.get(4)?,
                cover_thumb: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Returns all artists with aggregate album_count and track_count.
    pub fn list_artists(&self) -> Result<Vec<Artist>> {
        let mut stmt = self.conn.prepare(
            "SELECT ar.id, ar.name, \
                    COUNT(DISTINCT t.album_id) AS album_count, \
                    COUNT(t.id)               AS track_count \
             FROM artists ar \
             LEFT JOIN tracks t ON t.artist_id = ar.id \
             GROUP BY ar.id \
             ORDER BY ar.name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Artist {
                id:          r.get(0)?,
                name:        r.get(1)?,
                album_count: r.get(2)?,
                track_count: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Returns all tracks in the given album, ordered by disc_no then track_no.
    pub fn tracks_by_album(&self, album_id: i64) -> Result<Vec<Track>> {
        let sql = format!(
            "{SELECT_TRACK} \
             WHERE t.album_id = ?1 \
             ORDER BY t.disc_no, t.track_no"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([album_id], row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Returns all tracks by the given artist, ordered by album title then track_no.
    pub fn tracks_by_artist(&self, artist_id: i64) -> Result<Vec<Track>> {
        let sql = format!(
            "{SELECT_TRACK} \
             WHERE t.artist_id = ?1 \
             ORDER BY al.title, t.track_no"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([artist_id], row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Returns all genres with track counts, ordered alphabetically.
    /// Rows with NULL or empty genre are excluded.
    pub fn list_genres(&self) -> Result<Vec<Genre>> {
        let mut stmt = self.conn.prepare(
            "SELECT genre, COUNT(*) AS track_count \
             FROM tracks \
             WHERE genre IS NOT NULL AND genre <> '' \
             GROUP BY genre \
             ORDER BY genre",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Genre {
                name:        r.get(0)?,
                track_count: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Returns all tracks for the given genre, ordered by title.
    pub fn tracks_by_genre(&self, genre: &str) -> Result<Vec<Track>> {
        let sql = format!(
            "{SELECT_TRACK} \
             WHERE t.genre = ?1 \
             ORDER BY t.title"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([genre], row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // ── Playlist queries ─────────────────────────────────────────────────────

    /// Create a new playlist and return its id.
    pub fn create_playlist(&mut self, name: &str) -> Result<i64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO playlists(name, created) VALUES (?1, ?2)",
            rusqlite::params![name, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Rename an existing playlist.
    pub fn rename_playlist(&mut self, id: i64, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE playlists SET name = ?1 WHERE id = ?2",
            rusqlite::params![name, id],
        )?;
        Ok(())
    }

    /// Delete a playlist (cascade removes playlist_tracks rows).
    pub fn delete_playlist(&mut self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Add a track to a playlist. Position is max+1 (appended). Duplicates are ignored.
    pub fn add_to_playlist(&mut self, playlist_id: i64, track_id: i64) -> Result<()> {
        let next_pos: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            [playlist_id],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO playlist_tracks(playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            rusqlite::params![playlist_id, track_id, next_pos],
        )?;
        Ok(())
    }

    /// Remove a track from a playlist.
    pub fn remove_from_playlist(&mut self, playlist_id: i64, track_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            rusqlite::params![playlist_id, track_id],
        )?;
        Ok(())
    }

    /// List all playlists with track counts, ordered by name.
    pub fn list_playlists(&self) -> Result<Vec<Playlist>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.name, COUNT(pt.track_id) AS track_count \
             FROM playlists p \
             LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id \
             GROUP BY p.id \
             ORDER BY p.name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Playlist {
                id:          r.get(0)?,
                name:        r.get(1)?,
                track_count: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Returns all tracks in the given playlist, ordered by position.
    pub fn playlist_tracks(&self, id: i64) -> Result<Vec<Track>> {
        let sql = format!(
            "{SELECT_TRACK} \
             JOIN playlist_tracks pt ON pt.track_id = t.id \
             WHERE pt.playlist_id = ?1 \
             ORDER BY pt.position"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([id], row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Returns the cover_thumb path for the track at `path`, if any.
    pub fn cover_thumb_for_path(&self, path: &str) -> Option<String> {
        self.conn.query_row(
            "SELECT cover_thumb FROM tracks WHERE path = ?1",
            [path],
            |row| row.get::<_, Option<String>>(0),
        ).unwrap_or(None)
    }

    /// Returns the most recently added tracks (newest first by row id).
    pub fn recently_added(&self, limit: i64) -> Result<Vec<Track>> {
        let sql = format!(
            "{SELECT_TRACK} \
             ORDER BY t.id DESC \
             LIMIT ?1"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([limit], row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // ── Smart Playlist queries ────────────────────────────────────────────────

    /// Create a new smart playlist and return its id.
    pub fn create_smart_playlist(
        &mut self,
        name: &str,
        rules_json: &str,
        match_all: bool,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO smart_playlists(name, rules_json, match_all) VALUES (?1, ?2, ?3)",
            rusqlite::params![name, rules_json, match_all as i64],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Delete a smart playlist by id.
    pub fn delete_smart_playlist(&mut self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM smart_playlists WHERE id = ?1", [id])?;
        Ok(())
    }

    /// List all smart playlists, ordered by name.
    pub fn list_smart_playlists(&self) -> Result<Vec<SmartPlaylist>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, rules_json, match_all FROM smart_playlists ORDER BY name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SmartPlaylist {
                id:         r.get(0)?,
                name:       r.get(1)?,
                rules_json: r.get(2)?,
                match_all:  r.get::<_, i64>(3)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Evaluate a smart playlist's rules and return matching tracks (ORDER BY title).
    ///
    /// Rules are parsed from the stored `rules_json` column.  Each rule is
    /// `{ "field": "...", "op": "...", "value": "..." }`.  Unknown fields or
    /// operators are silently skipped (so the rule contributes nothing to the
    /// WHERE clause).  All parameters are bound — never interpolated — ensuring
    /// injection safety even when the value contains quotes or wildcards.
    pub fn smart_playlist_tracks(&self, id: i64) -> Result<Vec<Track>> {
        // Load the smart playlist row.
        let row = self.conn.query_row(
            "SELECT rules_json, match_all FROM smart_playlists WHERE id = ?1",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )?;
        let (rules_json, match_all_int) = row;
        let match_all = match_all_int != 0;

        // Parse rules.
        let rules: Vec<serde_json::Value> =
            serde_json::from_str(&rules_json).unwrap_or_default();

        // Build WHERE fragments and collect bound parameter values.
        let mut fragments: Vec<String> = Vec::new();
        let mut params: Vec<String> = Vec::new();

        for rule in &rules {
            let field = rule.get("field").and_then(|v| v.as_str()).unwrap_or("");
            let op    = rule.get("op").and_then(|v| v.as_str()).unwrap_or("");
            let value = rule.get("value").and_then(|v| v.as_str()).unwrap_or("");

            // Map logical field name → SQL column expression.
            let col = match field {
                "title"  => "t.title",
                "artist" => "ar.name",
                "album"  => "al.title",
                "genre"  => "t.genre",
                "year"   => "CAST(al.year AS INTEGER)",
                _        => continue, // unknown field — skip
            };

            let (fragment, bound_value) = match op {
                "contains" => {
                    // Case-insensitive LIKE: LOWER(col) LIKE '%' || lower(?) || '%'
                    (
                        format!("LOWER({col}) LIKE '%' || LOWER(?{}) || '%'", params.len() + 1),
                        value.to_owned(),
                    )
                }
                "is" => {
                    (
                        format!("{col} = ?{}", params.len() + 1),
                        value.to_owned(),
                    )
                }
                "gt" => {
                    (
                        format!("{col} > ?{}", params.len() + 1),
                        value.to_owned(),
                    )
                }
                "lt" => {
                    (
                        format!("{col} < ?{}", params.len() + 1),
                        value.to_owned(),
                    )
                }
                _ => continue, // unknown op — skip
            };

            fragments.push(fragment);
            params.push(bound_value);
        }

        // If no valid rules remain, return all tracks.
        let where_clause = if fragments.is_empty() {
            String::new()
        } else {
            let joiner = if match_all { " AND " } else { " OR " };
            format!(" WHERE {}", fragments.join(joiner))
        };

        let sql = format!("{SELECT_TRACK}{where_clause} ORDER BY t.title");

        let mut stmt = self.conn.prepare(&sql)?;
        let rusqlite_params: Vec<rusqlite::types::Value> = params
            .into_iter()
            .map(rusqlite::types::Value::Text)
            .collect();
        let rows = stmt.query_map(
            rusqlite::params_from_iter(rusqlite_params.iter()),
            row_to_track,
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

#[cfg(test)]
mod tests {
    use crate::{model::NewTrack, Db};

    fn nt(path: &str, title: &str, artist: &str, album: &str) -> NewTrack {
        NewTrack {
            path: path.into(), title: title.into(),
            artist: Some(artist.into()), album: Some(album.into()),
            album_artist: Some(artist.into()), track_no: Some(1), disc_no: Some(1),
            year: Some(2020), duration_ms: Some(180_000), mtime: 100, cover_thumb: None,
            genre: None,
        }
    }

    fn nt_with_genre(path: &str, title: &str, artist: &str, album: &str, genre: &str) -> NewTrack {
        NewTrack {
            path: path.into(), title: title.into(),
            artist: Some(artist.into()), album: Some(album.into()),
            album_artist: Some(artist.into()), track_no: Some(1), disc_no: Some(1),
            year: Some(2020), duration_ms: Some(180_000), mtime: 100,
            cover_thumb: None, genre: Some(genre.into()),
        }
    }

    /// Helper: insert 4 tracks across 2 albums and 2 artists.
    /// Album A ("Alpha Album") has 3 tracks by "Artist One", track 1 has a cover_thumb.
    /// Album B ("Beta Album") has 1 track by "Artist Two", no cover_thumb.
    fn seed_two_artists_two_albums(db: &mut Db) {
        let mut t1 = nt("/m/a1.flac", "Alpha Track 1", "Artist One", "Alpha Album");
        t1.track_no = Some(1); t1.disc_no = Some(1); t1.cover_thumb = Some("/art/alpha.jpg".into());

        let mut t2 = nt("/m/a2.flac", "Alpha Track 2", "Artist One", "Alpha Album");
        t2.track_no = Some(2); t2.disc_no = Some(1);

        let mut t3 = nt("/m/a3.flac", "Alpha Track 3", "Artist One", "Alpha Album");
        t3.track_no = Some(3); t3.disc_no = Some(1);

        let mut t4 = nt("/m/b1.flac", "Beta Track 1", "Artist Two", "Beta Album");
        t4.track_no = Some(1); t4.disc_no = Some(1);

        db.upsert_track(&t1).unwrap();
        db.upsert_track(&t2).unwrap();
        db.upsert_track(&t3).unwrap();
        db.upsert_track(&t4).unwrap();
    }

    #[test]
    fn list_albums_count_and_track_counts() {
        let mut db = Db::open_in_memory().unwrap();
        seed_two_artists_two_albums(&mut db);

        let albums = db.list_albums().unwrap();
        assert_eq!(albums.len(), 2, "expected 2 albums");

        // Albums are sorted by title: Alpha Album, Beta Album.
        let alpha = &albums[0];
        assert_eq!(alpha.title, "Alpha Album");
        assert_eq!(alpha.track_count, 3, "Alpha Album should have 3 tracks");
        assert!(
            alpha.cover_thumb.as_deref().is_some(),
            "Alpha Album should carry a cover_thumb"
        );
        assert_eq!(alpha.cover_thumb.as_deref(), Some("/art/alpha.jpg"));

        let beta = &albums[1];
        assert_eq!(beta.title, "Beta Album");
        assert_eq!(beta.track_count, 1, "Beta Album should have 1 track");
        // No cover was set for Beta Album tracks.
        assert!(beta.cover_thumb.is_none(), "Beta Album should have no cover_thumb");
    }

    #[test]
    fn list_artists_album_and_track_counts() {
        let mut db = Db::open_in_memory().unwrap();
        seed_two_artists_two_albums(&mut db);

        let artists = db.list_artists().unwrap();
        assert_eq!(artists.len(), 2, "expected 2 artists");

        // Sorted by name: Artist One, Artist Two.
        let one = &artists[0];
        assert_eq!(one.name, "Artist One");
        assert_eq!(one.track_count, 3);
        assert_eq!(one.album_count, 1, "Artist One has tracks in 1 album");

        let two = &artists[1];
        assert_eq!(two.name, "Artist Two");
        assert_eq!(two.track_count, 1);
        assert_eq!(two.album_count, 1, "Artist Two has tracks in 1 album");
    }

    #[test]
    fn tracks_by_album_returns_correct_rows_in_order() {
        let mut db = Db::open_in_memory().unwrap();
        seed_two_artists_two_albums(&mut db);

        let albums = db.list_albums().unwrap();
        let alpha_id = albums.iter().find(|a| a.title == "Alpha Album").unwrap().id;

        let tracks = db.tracks_by_album(alpha_id).unwrap();
        assert_eq!(tracks.len(), 3, "should return 3 tracks for Alpha Album");
        // disc_no and track_no ordering: 1/1, 1/2, 1/3.
        assert_eq!(tracks[0].title, "Alpha Track 1");
        assert_eq!(tracks[1].title, "Alpha Track 2");
        assert_eq!(tracks[2].title, "Alpha Track 3");
    }

    #[test]
    fn tracks_by_artist_returns_correct_rows() {
        let mut db = Db::open_in_memory().unwrap();
        seed_two_artists_two_albums(&mut db);

        let artists = db.list_artists().unwrap();
        let artist_one_id = artists.iter().find(|a| a.name == "Artist One").unwrap().id;

        let tracks = db.tracks_by_artist(artist_one_id).unwrap();
        assert_eq!(tracks.len(), 3, "Artist One has 3 tracks");
        // All belong to Alpha Album; within album ordered by track_no.
        assert!(tracks.iter().all(|t| t.album.as_deref() == Some("Alpha Album")));
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

    #[test]
    fn fts_stays_in_sync_after_update() {
        let mut db = Db::open_in_memory().unwrap();
        // Insert initial version with title "Original"
        db.upsert_track(&nt("/m/song.flac", "Original", "Artist", "Album")).unwrap();
        // Upsert same path with new title "Remastered"
        let mut t2 = nt("/m/song.flac", "Remastered", "Artist", "Album");
        t2.mtime = 200;
        db.upsert_track(&t2).unwrap();
        // FTS should reflect the new title only
        assert_eq!(db.search("remastered").unwrap().len(), 1, "updated title should be searchable");
        assert_eq!(db.search("original").unwrap().len(), 0, "old title must be removed from FTS index");
    }

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

    #[test]
    fn list_genres_groups_and_counts() {
        let mut db = Db::open_in_memory().unwrap();
        db.upsert_track(&nt_with_genre("/m/r1.flac", "Rock 1", "A", "Al1", "Rock")).unwrap();
        db.upsert_track(&nt_with_genre("/m/r2.flac", "Rock 2", "B", "Al2", "Rock")).unwrap();
        db.upsert_track(&nt_with_genre("/m/j1.flac", "Jazz 1", "C", "Al3", "Jazz")).unwrap();

        let genres = db.list_genres().unwrap();
        assert_eq!(genres.len(), 2, "expected 2 genres");
        // Sorted alphabetically: Jazz, Rock
        assert_eq!(genres[0].name, "Jazz");
        assert_eq!(genres[0].track_count, 1);
        assert_eq!(genres[1].name, "Rock");
        assert_eq!(genres[1].track_count, 2);
    }

    #[test]
    fn tracks_by_genre_returns_correct_rows() {
        let mut db = Db::open_in_memory().unwrap();
        db.upsert_track(&nt_with_genre("/m/r1.flac", "Rock 1", "A", "Al1", "Rock")).unwrap();
        db.upsert_track(&nt_with_genre("/m/r2.flac", "Rock 2", "B", "Al2", "Rock")).unwrap();
        db.upsert_track(&nt_with_genre("/m/j1.flac", "Jazz 1", "C", "Al3", "Jazz")).unwrap();

        let rock = db.tracks_by_genre("Rock").unwrap();
        assert_eq!(rock.len(), 2, "expected 2 Rock tracks");
        // Ordered by title
        assert_eq!(rock[0].title, "Rock 1");
        assert_eq!(rock[1].title, "Rock 2");

        let jazz = db.tracks_by_genre("Jazz").unwrap();
        assert_eq!(jazz.len(), 1, "expected 1 Jazz track");
        assert_eq!(jazz[0].title, "Jazz 1");
    }

    #[test]
    fn list_genres_ignores_null_and_empty() {
        let mut db = Db::open_in_memory().unwrap();
        // Track with no genre
        let no_genre = NewTrack {
            path: "/m/ng.flac".into(), title: "No Genre".into(),
            artist: Some("A".into()), album: Some("Al".into()),
            album_artist: Some("A".into()), track_no: Some(1), disc_no: Some(1),
            year: Some(2020), duration_ms: Some(180_000), mtime: 100,
            cover_thumb: None, genre: None,
        };
        db.upsert_track(&no_genre).unwrap();
        db.upsert_track(&nt_with_genre("/m/r1.flac", "Rock 1", "B", "Al2", "Rock")).unwrap();

        let genres = db.list_genres().unwrap();
        assert_eq!(genres.len(), 1, "NULL genre must not appear in list");
        assert_eq!(genres[0].name, "Rock");
    }

    // ── Playlist tests ────────────────────────────────────────────────────────

    #[test]
    fn playlist_create_add_list_and_tracks() {
        let mut db = Db::open_in_memory().unwrap();
        seed_two_artists_two_albums(&mut db);

        // Get track ids
        let tracks = db.list_tracks().unwrap();
        assert!(tracks.len() >= 2, "need at least 2 tracks");
        let t1_id = tracks[0].id;
        let t2_id = tracks[1].id;

        // Create playlist
        let pid = db.create_playlist("My Mix").unwrap();
        assert!(pid > 0, "playlist id must be positive");

        // Add 2 tracks
        db.add_to_playlist(pid, t1_id).unwrap();
        db.add_to_playlist(pid, t2_id).unwrap();

        // list_playlists: count and track_count
        let playlists = db.list_playlists().unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "My Mix");
        assert_eq!(playlists[0].track_count, 2);

        // playlist_tracks: order by position (insertion order)
        let pt = db.playlist_tracks(pid).unwrap();
        assert_eq!(pt.len(), 2);
        assert_eq!(pt[0].id, t1_id);
        assert_eq!(pt[1].id, t2_id);
    }

    #[test]
    fn playlist_duplicate_add_is_ignored() {
        let mut db = Db::open_in_memory().unwrap();
        seed_two_artists_two_albums(&mut db);
        let tracks = db.list_tracks().unwrap();
        let t1_id = tracks[0].id;

        let pid = db.create_playlist("Dup Test").unwrap();
        db.add_to_playlist(pid, t1_id).unwrap();
        db.add_to_playlist(pid, t1_id).unwrap(); // duplicate — should be ignored

        let pt = db.playlist_tracks(pid).unwrap();
        assert_eq!(pt.len(), 1, "duplicate track must not be added twice");
    }

    #[test]
    fn playlist_remove_track() {
        let mut db = Db::open_in_memory().unwrap();
        seed_two_artists_two_albums(&mut db);
        let tracks = db.list_tracks().unwrap();
        let t1_id = tracks[0].id;
        let t2_id = tracks[1].id;

        let pid = db.create_playlist("Remove Test").unwrap();
        db.add_to_playlist(pid, t1_id).unwrap();
        db.add_to_playlist(pid, t2_id).unwrap();

        db.remove_from_playlist(pid, t1_id).unwrap();

        let pt = db.playlist_tracks(pid).unwrap();
        assert_eq!(pt.len(), 1);
        assert_eq!(pt[0].id, t2_id);
    }

    #[test]
    fn playlist_delete_cascades() {
        let mut db = Db::open_in_memory().unwrap();
        seed_two_artists_two_albums(&mut db);
        let tracks = db.list_tracks().unwrap();
        let t1_id = tracks[0].id;

        let pid = db.create_playlist("To Delete").unwrap();
        db.add_to_playlist(pid, t1_id).unwrap();

        db.delete_playlist(pid).unwrap();

        // Playlist row gone
        let playlists = db.list_playlists().unwrap();
        assert!(playlists.is_empty(), "playlist must be deleted");

        // playlist_tracks row cascaded
        let n: i64 = db.conn
            .query_row("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?1", [pid], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "playlist_tracks must cascade-delete");
    }

    #[test]
    fn recently_added_returns_newest_first() {
        let mut db = Db::open_in_memory().unwrap();
        // Insert in order: track A then track B — B has higher id
        db.upsert_track(&nt("/m/a.flac", "Track A", "Artist", "Album")).unwrap();
        db.upsert_track(&nt("/m/b.flac", "Track B", "Artist", "Album")).unwrap();

        let recent = db.recently_added(10).unwrap();
        assert_eq!(recent.len(), 2);
        // Newest first: Track B was inserted last so has higher id
        assert_eq!(recent[0].title, "Track B");
        assert_eq!(recent[1].title, "Track A");
    }

    #[test]
    fn recently_added_respects_limit() {
        let mut db = Db::open_in_memory().unwrap();
        seed_two_artists_two_albums(&mut db); // inserts 4 tracks

        let recent = db.recently_added(2).unwrap();
        assert_eq!(recent.len(), 2, "limit must be respected");
    }

    // ── Smart Playlist tests ───────────────────────────────────────────────────

    /// Seed tracks suitable for smart playlist testing: various genres and years.
    fn seed_smart_tracks(db: &mut Db) {
        // Rock tracks
        let mut r1 = NewTrack {
            path: "/sp/rock1.flac".into(), title: "Rock Anthem".into(),
            artist: Some("Band A".into()), album: Some("Rock Album".into()),
            album_artist: Some("Band A".into()), track_no: Some(1), disc_no: Some(1),
            year: Some(2015), duration_ms: Some(200_000), mtime: 1,
            cover_thumb: None, genre: Some("Rock".into()),
        };
        let r2 = NewTrack {
            path: "/sp/rock2.flac".into(), title: "Stone Cold".into(),
            artist: Some("Band A".into()), album: Some("Rock Album".into()),
            album_artist: Some("Band A".into()), track_no: Some(2), disc_no: Some(1),
            year: Some(2015), duration_ms: Some(210_000), mtime: 2,
            cover_thumb: None, genre: Some("Rock".into()),
        };
        // Jazz track
        let j1 = NewTrack {
            path: "/sp/jazz1.flac".into(), title: "Blue Note".into(),
            artist: Some("Jazz Cat".into()), album: Some("Cool Jazz".into()),
            album_artist: Some("Jazz Cat".into()), track_no: Some(1), disc_no: Some(1),
            year: Some(1998), duration_ms: Some(320_000), mtime: 3,
            cover_thumb: None, genre: Some("Jazz".into()),
        };
        // Electronic track — newer year
        let e1 = NewTrack {
            path: "/sp/elec1.flac".into(), title: "Pulse Wave".into(),
            artist: Some("Synth Mind".into()), album: Some("Circuits".into()),
            album_artist: Some("Synth Mind".into()), track_no: Some(1), disc_no: Some(1),
            year: Some(2022), duration_ms: Some(240_000), mtime: 4,
            cover_thumb: None, genre: Some("Electronic".into()),
        };
        r1.year = Some(2015);
        db.upsert_track(&r1).unwrap();
        db.upsert_track(&r2).unwrap();
        db.upsert_track(&j1).unwrap();
        db.upsert_track(&e1).unwrap();
    }

    #[test]
    fn smart_playlist_genre_is_rule() {
        let mut db = Db::open_in_memory().unwrap();
        seed_smart_tracks(&mut db);

        let rules = r#"[{"field":"genre","op":"is","value":"Rock"}]"#;
        let id = db.create_smart_playlist("Rock Only", rules, true).unwrap();

        let tracks = db.smart_playlist_tracks(id).unwrap();
        assert_eq!(tracks.len(), 2, "should return 2 Rock tracks");
        assert!(tracks.iter().all(|t| t.title == "Rock Anthem" || t.title == "Stone Cold"),
            "unexpected track in Rock-only playlist");
    }

    #[test]
    fn smart_playlist_title_contains_rule() {
        let mut db = Db::open_in_memory().unwrap();
        seed_smart_tracks(&mut db);

        let rules = r#"[{"field":"title","op":"contains","value":"stone"}]"#;
        let id = db.create_smart_playlist("Stone Tracks", rules, true).unwrap();

        let tracks = db.smart_playlist_tracks(id).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Stone Cold");
    }

    #[test]
    fn smart_playlist_year_gt_rule() {
        let mut db = Db::open_in_memory().unwrap();
        seed_smart_tracks(&mut db);

        // year > 2015 should return only "Pulse Wave" (2022)
        let rules = r#"[{"field":"year","op":"gt","value":"2015"}]"#;
        let id = db.create_smart_playlist("Recent", rules, true).unwrap();

        let tracks = db.smart_playlist_tracks(id).unwrap();
        assert_eq!(tracks.len(), 1, "only 2022 track exceeds 2015");
        assert_eq!(tracks[0].title, "Pulse Wave");
    }

    #[test]
    fn smart_playlist_multi_rule_all() {
        let mut db = Db::open_in_memory().unwrap();
        seed_smart_tracks(&mut db);

        // genre is Rock AND artist contains "Band" → both Rock tracks
        let rules = r#"[{"field":"genre","op":"is","value":"Rock"},{"field":"artist","op":"contains","value":"Band"}]"#;
        let id = db.create_smart_playlist("Rock by Band", rules, true).unwrap();

        let tracks = db.smart_playlist_tracks(id).unwrap();
        assert_eq!(tracks.len(), 2);
    }

    #[test]
    fn smart_playlist_multi_rule_any() {
        let mut db = Db::open_in_memory().unwrap();
        seed_smart_tracks(&mut db);

        // genre is Rock OR genre is Jazz → 3 tracks
        let rules = r#"[{"field":"genre","op":"is","value":"Rock"},{"field":"genre","op":"is","value":"Jazz"}]"#;
        let id = db.create_smart_playlist("Rock or Jazz", rules, false).unwrap();

        let tracks = db.smart_playlist_tracks(id).unwrap();
        assert_eq!(tracks.len(), 3, "2 Rock + 1 Jazz = 3");
    }

    #[test]
    fn smart_playlist_artist_contains_case_insensitive() {
        let mut db = Db::open_in_memory().unwrap();
        seed_smart_tracks(&mut db);

        // "synth" (lowercase) should match "Synth Mind"
        let rules = r#"[{"field":"artist","op":"contains","value":"synth"}]"#;
        let id = db.create_smart_playlist("Synth Artists", rules, true).unwrap();

        let tracks = db.smart_playlist_tracks(id).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Pulse Wave");
    }

    #[test]
    fn smart_playlist_injection_safety() {
        let mut db = Db::open_in_memory().unwrap();
        seed_smart_tracks(&mut db);

        // A value with SQL metacharacters must not break the query or return extra rows.
        let evil = r#"Rock' OR '1'='1"#;
        let rules = serde_json::json!([{"field":"genre","op":"is","value": evil}]).to_string();
        let id = db.create_smart_playlist("Injection Test", &rules, true).unwrap();

        // Should return no results (no track has that exact genre string).
        let tracks = db.smart_playlist_tracks(id).unwrap();
        assert_eq!(tracks.len(), 0, "injection must not return extra rows");
    }

    #[test]
    fn smart_playlist_unknown_field_skipped() {
        let mut db = Db::open_in_memory().unwrap();
        seed_smart_tracks(&mut db);

        // An unknown field is silently skipped → empty WHERE → all tracks returned.
        let rules = r#"[{"field":"unknownfield","op":"is","value":"anything"}]"#;
        let id = db.create_smart_playlist("No Filter", rules, true).unwrap();

        let tracks = db.smart_playlist_tracks(id).unwrap();
        assert_eq!(tracks.len(), 4, "no valid rules → all tracks returned");
    }

    #[test]
    fn smart_playlist_list_and_delete() {
        let mut db = Db::open_in_memory().unwrap();

        let id1 = db.create_smart_playlist("Zz Last", r#"[]"#, true).unwrap();
        let id2 = db.create_smart_playlist("Aa First", r#"[]"#, false).unwrap();

        let list = db.list_smart_playlists().unwrap();
        assert_eq!(list.len(), 2);
        // Ordered by name: Aa First, Zz Last
        assert_eq!(list[0].name, "Aa First");
        assert!(!list[0].match_all);
        assert_eq!(list[1].name, "Zz Last");
        assert!(list[1].match_all);

        db.delete_smart_playlist(id1).unwrap();
        let list2 = db.list_smart_playlists().unwrap();
        assert_eq!(list2.len(), 1);
        assert_eq!(list2[0].id, id2);
    }

    #[test]
    fn smart_playlist_album_contains_rule() {
        let mut db = Db::open_in_memory().unwrap();
        seed_smart_tracks(&mut db);

        let rules = r#"[{"field":"album","op":"contains","value":"jazz"}]"#;
        let id = db.create_smart_playlist("Jazz Albums", rules, true).unwrap();

        let tracks = db.smart_playlist_tracks(id).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Blue Note");
    }

    #[test]
    fn smart_playlist_year_lt_rule() {
        let mut db = Db::open_in_memory().unwrap();
        seed_smart_tracks(&mut db);

        // year < 2015 should return only "Blue Note" (1998)
        let rules = r#"[{"field":"year","op":"lt","value":"2015"}]"#;
        let id = db.create_smart_playlist("Old Tracks", rules, true).unwrap();

        let tracks = db.smart_playlist_tracks(id).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Blue Note");
    }
}
