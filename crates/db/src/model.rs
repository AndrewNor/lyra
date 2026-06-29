//! Domain types for lyra-db.

/// An album row with joined artist name and aggregate track info.
#[derive(Debug, Clone)]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub year: Option<i32>,
    pub track_count: i64,
    pub cover_thumb: Option<String>,
}

/// An artist row with aggregate album/track counts.
#[derive(Debug, Clone)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub album_count: i64,
    pub track_count: i64,
}

/// Data needed to insert or update a track.
#[derive(Debug, Clone)]
pub struct NewTrack {
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub duration_ms: Option<u64>,
    pub mtime: i64,
    pub cover_thumb: Option<String>,
}

/// A track row as read from the database.
#[derive(Debug, Clone)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<u32>,
    pub duration_ms: Option<u64>,
    pub cover_thumb: Option<String>,
}
