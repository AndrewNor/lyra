//! XDG-aware path helpers for lyra-ui.

use std::path::PathBuf;

/// Return the absolute path to the lyra library database.
///
/// Respects `$XDG_DATA_HOME`; falls back to `$HOME/.local/share` when the
/// variable is absent or empty.  Creates the parent directory if needed
/// (ignores an already-exists error; all other I/O errors are silently
/// discarded rather than panicking).
pub fn library_db_path() -> PathBuf {
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
            PathBuf::from(home).join(".local").join("share")
        });

    let db_dir = data_home.join("lyra");
    // Best-effort: ignore the error if the directory already exists, and
    // silently continue if creation fails for any other reason (the db open
    // will then surface a more descriptive error to the caller).
    let _ = std::fs::create_dir_all(&db_dir);

    db_dir.join("library.db")
}

/// Return the absolute path to the lyra album-art thumbnail cache directory.
///
/// Respects `$XDG_CACHE_HOME`; falls back to `$HOME/.cache` when the variable
/// is absent or empty.  Creates the directory if needed (ignores errors).
pub fn art_cache_dir() -> std::path::PathBuf {
    let cache_home = std::env::var("XDG_CACHE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
            std::path::PathBuf::from(home).join(".cache")
        });

    let art_dir = cache_home.join("lyra").join("art");
    let _ = std::fs::create_dir_all(&art_dir);
    art_dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_ends_with_lyra_library_db() {
        let p = library_db_path();
        // The last two components must be "lyra" and "library.db".
        let mut comps = p.components().rev();
        let file = comps.next().map(|c| c.as_os_str().to_string_lossy().into_owned());
        let dir  = comps.next().map(|c| c.as_os_str().to_string_lossy().into_owned());
        assert_eq!(file.as_deref(), Some("library.db"), "filename must be library.db, got: {:?}", p);
        assert_eq!(dir.as_deref(),  Some("lyra"),       "parent dir must be lyra, got: {:?}", p);
    }
}
