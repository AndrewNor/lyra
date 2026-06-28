use lyra_db::Db;
use lyra_library::scan;
use std::io::Write;

/// Minimal valid 16-bit PCM WAV (44-byte header + 4 bytes data).
fn write_min_wav(path: &std::path::Path) {
    let data: [u8; 4] = [0, 0, 0, 0]; // one stereo 16-bit frame of silence
    let mut f = std::fs::File::create(path).unwrap();
    let n = data.len() as u32;
    let riff = 36 + n;
    f.write_all(b"RIFF").unwrap();
    f.write_all(&riff.to_le_bytes()).unwrap();
    f.write_all(b"WAVE").unwrap();
    f.write_all(b"fmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    f.write_all(&2u16.to_le_bytes()).unwrap(); // 2 ch
    f.write_all(&44100u32.to_le_bytes()).unwrap();
    f.write_all(&176400u32.to_le_bytes()).unwrap(); // byte rate
    f.write_all(&4u16.to_le_bytes()).unwrap(); // block align
    f.write_all(&16u16.to_le_bytes()).unwrap(); // bits
    f.write_all(b"data").unwrap();
    f.write_all(&n.to_le_bytes()).unwrap();
    f.write_all(&data).unwrap();
}

/// Create a WAV with tags written into it.
fn make_tagged_wav(path: &std::path::Path, title: &str, artist: &str, album: &str) {
    write_min_wav(path);
    let mut tags = lyra_metadata::TrackTags::default();
    tags.title = Some(title.into());
    tags.artist = Some(artist.into());
    tags.album = Some(album.into());
    lyra_metadata::write_tags(path, &tags).unwrap();
}

#[test]
fn scan_is_incremental() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    let b = dir.path().join("b.wav");
    make_tagged_wav(&a, "A", "Artist", "Album");
    make_tagged_wav(&b, "B", "Artist", "Album");
    let mut db = Db::open_in_memory().unwrap();

    // First scan: both files are new → added=2
    let s1 = scan(dir.path(), &mut db).unwrap();
    assert_eq!((s1.added, s1.updated), (2, 0), "first scan: expected 2 added");
    assert_eq!(db.list_tracks().unwrap().len(), 2);

    // Second scan: nothing changed → unchanged=2
    let s2 = scan(dir.path(), &mut db).unwrap();
    assert_eq!(
        (s2.added, s2.updated, s2.unchanged),
        (0, 0, 2),
        "second scan: expected 2 unchanged"
    );

    // Bump mtime on a.wav by rewriting its tags (which modifies the file, changing mtime)
    // Sleep briefly to guarantee the filesystem mtime advances past the stored value
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut tags = lyra_metadata::TrackTags::default();
    tags.title = Some("A (updated)".into());
    tags.artist = Some("Artist".into());
    tags.album = Some("Album".into());
    lyra_metadata::write_tags(&a, &tags).unwrap();

    // Third scan: a.wav mtime changed → updated >= 1
    let s3 = scan(dir.path(), &mut db).unwrap();
    assert!(
        s3.updated >= 1,
        "third scan: expected at least 1 updated, got {:?}",
        s3
    );
}
