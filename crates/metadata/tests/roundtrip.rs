use lyra_metadata::{read_tags, read_cover, write_tags, write_cover, TrackTags};
use std::io::Write;

/// Minimal valid 16-bit PCM WAV (44-byte header + 1 sample frame), enough for
/// lofty to recognise the file and attach a tag.
fn write_min_wav(path: &std::path::Path) {
    let data: [u8; 4] = [0, 0, 0, 0]; // one stereo 16-bit frame of silence
    let mut f = std::fs::File::create(path).unwrap();
    let n = data.len() as u32;
    let riff = 36 + n;
    f.write_all(b"RIFF").unwrap(); f.write_all(&riff.to_le_bytes()).unwrap();
    f.write_all(b"WAVE").unwrap();
    f.write_all(b"fmt ").unwrap(); f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap();   // PCM
    f.write_all(&2u16.to_le_bytes()).unwrap();   // 2 ch
    f.write_all(&44100u32.to_le_bytes()).unwrap();
    f.write_all(&176400u32.to_le_bytes()).unwrap(); // byte rate
    f.write_all(&4u16.to_le_bytes()).unwrap();   // block align
    f.write_all(&16u16.to_le_bytes()).unwrap();  // bits
    f.write_all(b"data").unwrap(); f.write_all(&n.to_le_bytes()).unwrap();
    f.write_all(&data).unwrap();
}

#[test]
fn write_then_read_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("t.wav");
    write_min_wav(&p);

    let mut tags = TrackTags::default();
    tags.title = Some("Verdant".into());
    tags.artist = Some("Bonobo".into());
    tags.album = Some("Migration".into());
    tags.track_no = Some(4);
    write_tags(&p, &tags).unwrap();

    let got = read_tags(&p).unwrap();
    assert_eq!(got.title.as_deref(), Some("Verdant"));
    assert_eq!(got.artist.as_deref(), Some("Bonobo"));
    assert_eq!(got.album.as_deref(), Some("Migration"));
    assert_eq!(got.track_no, Some(4));
}

#[test]
fn cover_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("c.wav");
    write_min_wav(&p);
    // 1x1 PNG (minimal valid PNG bytes)
    let png: &[u8] = &[137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,8,2,0,0,0,144,119,83,222,0,0,0,12,73,68,65,84,8,215,99,248,207,192,0,0,0,3,0,1,0,24,221,141,219,0,0,0,0,73,69,78,68,174,66,96,130];
    write_cover(&p, "image/png", png).unwrap();
    let got = read_cover(&p).unwrap().expect("cover present");
    assert_eq!(got.mime, "image/png");
    assert!(!got.data.is_empty());
}
