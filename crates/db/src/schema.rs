use rusqlite_migration::{Migrations, M};

pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
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
            id          INTEGER PRIMARY KEY,
            path        TEXT NOT NULL UNIQUE,
            title       TEXT NOT NULL,
            artist_id   INTEGER REFERENCES artists(id),
            album_id    INTEGER REFERENCES albums(id),
            track_no    INTEGER,
            disc_no     INTEGER,
            duration_ms INTEGER,
            mtime       INTEGER NOT NULL,
            cover_thumb TEXT
        );
        CREATE INDEX idx_tracks_album ON tracks(album_id);
        CREATE INDEX idx_tracks_artist ON tracks(artist_id);

        CREATE VIRTUAL TABLE tracks_fts USING fts5(
            title, artist, album,
            content='',
            contentless_delete=1
        );
        "#,
        ),
        M::up("ALTER TABLE tracks ADD COLUMN genre TEXT;"),
    ])
}
