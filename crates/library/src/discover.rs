use std::path::{Path, PathBuf};

const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "m4a", "aac", "ogg", "opus", "wav", "aiff", "alac", "wv",
];

/// Recursively discovers audio files under `root`.
/// Returns only regular files whose lowercased extension is a known audio format.
/// Symlink loops are not followed.
pub fn discover(root: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| AUDIO_EXTS.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .map(|entry| entry.into_path())
        .collect()
}

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
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        found.sort();
        assert_eq!(found, vec!["a.flac", "b.MP3", "c.ogg"]);
    }
}
