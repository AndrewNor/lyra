//! Database query implementations.

use crate::{model::{Album, Artist, Genre, NewTrack, Playlist, Track}, Db, Result};

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
}
