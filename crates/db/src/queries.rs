//! Database query implementations.

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
            rusqlite::params![
                t.path, t.title, artist_id, album_id,
                t.track_no, t.disc_no,
                t.duration_ms.map(|d| d as i64),
                t.mtime, t.cover_thumb
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
