//! lyra-library: directory scan + album-art cache.

pub mod discover;
pub mod artcache;

pub use discover::discover;
pub use artcache::ArtCache;

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Db(#[from] lyra_db::Error),
    #[error(transparent)]
    Metadata(#[from] lyra_metadata::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Image(#[from] image::ImageError),
}
pub type Result<T> = std::result::Result<T, Error>;

/// Summary of a directory scan operation.
#[derive(Debug, Default)]
pub struct ScanSummary {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub failed: usize,
}

/// Incrementally scans `root` for audio files, upserting new/changed ones into `db`.
pub fn scan(root: &Path, db: &mut lyra_db::Db) -> Result<ScanSummary> {
    use rayon::prelude::*;
    use std::time::UNIX_EPOCH;

    let known_mtimes = db.track_mtimes()?;
    let files = discover(root);

    // Partition: files that need (re-)parsing vs unchanged
    let mut to_parse: Vec<(std::path::PathBuf, i64)> = Vec::new();
    let mut unchanged_count = 0usize;

    let mut failed_count = 0usize;
    for path in &files {
        let mtime = match std::fs::metadata(path)
            .and_then(|m| m.modified())
            .map(|t| t.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64))
        {
            Ok(Ok(secs)) => secs,
            _ => {
                // Cannot read mtime: count as failed and skip to avoid endless re-parsing.
                failed_count += 1;
                continue;
            }
        };

        let path_str = path.to_string_lossy().into_owned();
        match known_mtimes.get(&path_str) {
            Some(&db_mtime) if db_mtime == mtime => {
                unchanged_count += 1;
            }
            _ => {
                to_parse.push((path.clone(), mtime));
            }
        }
    }

    // Parse tags in parallel
    let parsed: Vec<(std::path::PathBuf, i64, Result<lyra_metadata::TrackTags>)> = to_parse
        .into_par_iter()
        .map(|(path, mtime)| {
            let tags = lyra_metadata::read_tags(&path).map_err(Error::from);
            (path, mtime, tags)
        })
        .collect();

    let mut summary = ScanSummary {
        unchanged: unchanged_count,
        failed: failed_count,
        ..Default::default()
    };

    // Upsert sequentially (rusqlite Connection is not Sync)
    for (path, mtime, tags_result) in parsed {
        let tags = match tags_result {
            Ok(t) => t,
            Err(_) => {
                summary.failed += 1;
                continue;
            }
        };

        let path_str = path.to_string_lossy().into_owned();
        let was_known = known_mtimes.contains_key(&path_str);

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();
        let title = tags.title.unwrap_or(stem);

        let new_track = lyra_db::model::NewTrack {
            path: path_str,
            title,
            artist: tags.artist,
            album: tags.album,
            album_artist: tags.album_artist,
            track_no: tags.track_no,
            disc_no: tags.disc_no,
            year: tags.year,
            duration_ms: tags.duration_ms,
            mtime,
            cover_thumb: None,
        };

        match db.upsert_track(&new_track) {
            Ok(_) => {
                if was_known {
                    summary.updated += 1;
                } else {
                    summary.added += 1;
                }
            }
            Err(_) => {
                summary.failed += 1;
            }
        }
    }

    Ok(summary)
}
