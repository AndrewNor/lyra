use lyra_db::Db;
use lyra_library::{scan, ArtCache};
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

/// Tiny 1×1 RGB PNG — a properly CRC-valid PNG that `image::load_from_memory` can decode.
/// Generated via Python's `struct`/`zlib` with correct chunk CRCs.
const TINY_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10,   // PNG signature
    0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0, 144, 119, 83, 222, // IHDR
    0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 0, 3, 1, 1, 0, 201, 254, 146, 239, // IDAT
    0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130, // IEND
];

#[test]
fn scan_is_incremental() {
    let dir = tempfile::tempdir().unwrap();
    let art_dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    let b = dir.path().join("b.wav");
    make_tagged_wav(&a, "A", "Artist", "Album");
    make_tagged_wav(&b, "B", "Artist", "Album");
    let mut db = Db::open_in_memory().unwrap();
    let art = ArtCache::new(art_dir.path().to_path_buf());

    // First scan: both files are new → added=2
    let s1 = scan(dir.path(), &mut db, &art).unwrap();
    assert_eq!((s1.added, s1.updated), (2, 0), "first scan: expected 2 added");
    assert_eq!(db.list_tracks().unwrap().len(), 2);

    // Second scan: nothing changed → unchanged=2
    let s2 = scan(dir.path(), &mut db, &art).unwrap();
    assert_eq!(
        (s2.added, s2.updated, s2.unchanged),
        (0, 0, 2),
        "second scan: expected 2 unchanged"
    );

    // Bump mtime on a.wav by rewriting its tags.
    // Sleep 1.1s to guarantee the filesystem mtime (1-second resolution) advances
    // past the value stored in the DB from the first scan.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let mut tags = lyra_metadata::TrackTags::default();
    tags.title = Some("A (updated)".into());
    tags.artist = Some("Artist".into());
    tags.album = Some("Album".into());
    lyra_metadata::write_tags(&a, &tags).unwrap();

    // Third scan: a.wav mtime changed → updated >= 1
    let s3 = scan(dir.path(), &mut db, &art).unwrap();
    assert!(
        s3.updated >= 1,
        "third scan: expected at least 1 updated, got {:?}",
        s3
    );
}


#[test]
fn scan_populates_cover_thumb_when_cover_present() {
    let dir = tempfile::tempdir().unwrap();
    let art_dir = tempfile::tempdir().unwrap();

    // Create a WAV with tags and an embedded cover.
    let wav_path = dir.path().join("covered.wav");
    make_tagged_wav(&wav_path, "Covered Track", "Artist", "Album");
    lyra_metadata::write_cover(&wav_path, "image/png", TINY_PNG).unwrap();

    // Create a WAV without a cover (control).
    let wav_no_cover = dir.path().join("no_cover.wav");
    make_tagged_wav(&wav_no_cover, "No Cover", "Artist", "Album");

    let mut db = Db::open_in_memory().unwrap();
    let art = ArtCache::new(art_dir.path().to_path_buf());

    let summary = scan(dir.path(), &mut db, &art).unwrap();
    assert_eq!(summary.added, 2, "both tracks should be added");

    let tracks = db.list_tracks().unwrap();
    let covered = tracks
        .iter()
        .find(|t| t.title == "Covered Track")
        .expect("covered track must be in db");
    let no_cover = tracks
        .iter()
        .find(|t| t.title == "No Cover")
        .expect("no-cover track must be in db");

    // The covered track must have a non-empty cover_thumb pointing at an existing file.
    let thumb_path = covered
        .cover_thumb
        .as_deref()
        .expect("cover_thumb must be Some for a track with embedded cover");
    assert!(!thumb_path.is_empty(), "cover_thumb must not be empty");
    assert!(
        std::path::Path::new(thumb_path).exists(),
        "cover_thumb path must point to an existing file: {thumb_path}"
    );

    // The no-cover track must have cover_thumb = None.
    assert!(
        no_cover.cover_thumb.is_none(),
        "track without embedded cover must have cover_thumb = None"
    );
}
