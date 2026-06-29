//! lyra-db: SQLite+FTS5 library database.
mod schema;
pub mod model;
pub mod queries;

pub use model::{Album, Artist, Genre, NewTrack, Track};

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

    #[test]
    fn migration_adds_genre_column() {
        let db = Db::open_in_memory().unwrap();
        // Should not error — genre column must exist after migrations.
        db.conn.execute("INSERT INTO tracks(path,title,mtime,genre) VALUES('/t.flac','T',1,'Rock')", []).unwrap();
        let g: Option<String> = db.conn
            .query_row("SELECT genre FROM tracks WHERE path='/t.flac'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(g.as_deref(), Some("Rock"));
    }
}
