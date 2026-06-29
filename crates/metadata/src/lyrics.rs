/// One line of lyrics, optionally timestamped.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LyricLine {
    /// Timestamp in seconds, or None for unsynced lines.
    #[serde(rename = "t")]
    pub t_secs: Option<f64>,
    pub text: String,
}

/// A full set of lyrics: either synced (from .lrc) or unsynced (embedded).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Lyrics {
    pub synced: bool,
    pub lines: Vec<LyricLine>,
}

/// Parse an LRC-format string into timestamped lyric lines.
///
/// Rules:
/// - Recognises `[mm:ss.xx]`, `[mm:ss.x]`, `[mm:ss]` timestamps.
/// - A line with N leading timestamps emits N `LyricLine` entries (same text).
/// - ID tags (`[ti:…]`, `[ar:…]`, `[al:…]`, `[by:…]`, `[length:…]`) are ignored.
/// - Lines with no recognised timestamp AND no recognised ID tag become
///   LyricLine with `t_secs: None` (unsupported but graceful).
/// - Blank lines are skipped.
/// - Result is sorted ascending by `t_secs` (None entries go last).
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    // Regex-free parser. Per line:
    //   1. Consume all leading `[...]` bracket groups.
    //   2. For each bracket group: try to parse as [mm:ss.xx] / [mm:ss.x] / [mm:ss].
    //      If it matches a known ID tag label (ti/ar/al/by/length) → skip entire line.
    //      If it looks like a time → record timestamp.
    //      Else → ignore that bracket (treat line as having no time if no others matched).
    //   3. Remaining text after all leading brackets is the lyric body.
    //   4. Emit one LyricLine per collected timestamp (or one with t_secs:None if none).
    //   5. Skip blank lyric bodies.
    // Finally sort: timed lines ascending, None-time lines at end.

    const ID_TAGS: &[&str] = &["ti", "ar", "al", "by", "length", "offset", "re", "ve"];

    let mut result: Vec<LyricLine> = Vec::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let mut timestamps: Vec<f64> = Vec::new();
        let mut is_id_tag = false;
        let mut rest = line;

        // Consume consecutive leading `[...]` groups
        while let Some(close) = rest.find(']') {
            if !rest.starts_with('[') {
                break;
            }
            let inner = &rest[1..close];
            rest = &rest[close + 1..];

            // Check for ID tag: contains a colon and the part before colon is a known label
            if let Some(colon) = inner.find(':') {
                let label = inner[..colon].trim().to_ascii_lowercase();
                if ID_TAGS.contains(&label.as_str()) {
                    is_id_tag = true;
                    break;
                }
                // Try to parse as time: mm:ss or mm:ss.xx
                let mm_str = inner[..colon].trim();
                let rest_of_time = inner[colon + 1..].trim();
                // rest_of_time may be "ss.xx", "ss.x", or "ss"
                if let (Ok(mm), Ok(ss_f)) = (
                    mm_str.parse::<u32>(),
                    rest_of_time.parse::<f64>(),
                ) {
                    let t = mm as f64 * 60.0 + ss_f;
                    timestamps.push(t);
                }
                // else: unrecognised bracket — just ignore it, continue
            }
            // No colon in bracket: ignore (e.g. "[xx]" without colon)
        }

        if is_id_tag {
            continue;
        }

        // rest is now the lyric text (trim leading space after brackets)
        let text_body = rest.trim().to_owned();
        if text_body.is_empty() && timestamps.is_empty() {
            continue;
        }

        if timestamps.is_empty() {
            // Bracket was present but unparseable — emit a line with no timestamp
            // only if there's actual text content
            if !text_body.is_empty() {
                result.push(LyricLine { t_secs: None, text: text_body });
            }
        } else {
            for t in timestamps {
                result.push(LyricLine { t_secs: Some(t), text: text_body.clone() });
            }
        }
    }

    // Sort: timed lines ascending, None-time lines at end (stable)
    result.sort_by(|a, b| match (a.t_secs, b.t_secs) {
        (Some(at), Some(bt)) => at.partial_cmp(&bt).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None)      => std::cmp::Ordering::Less,
        (None, Some(_))      => std::cmp::Ordering::Greater,
        (None, None)         => std::cmp::Ordering::Equal,
    });

    result
}

/// Read lyrics for the audio file at `path`.
///
/// Resolution order:
/// 1. Sidecar `.lrc` file (same name, extension replaced with `lrc`).
///    If it exists and parses to at least one line → `Lyrics { synced: true, … }`.
/// 2. Embedded unsynced lyrics tag (lofty `Lyrics` / `UnsyncLyrics` item keys).
///    If present → `Lyrics { synced: false, … }` with one `LyricLine` per text line.
/// 3. Otherwise → `Lyrics { synced: false, lines: [] }`.
///
/// Never panics; I/O and parse errors are handled gracefully.
pub fn read_lyrics(path: &std::path::Path) -> crate::Result<Lyrics> {
    // ── 1. Sidecar .lrc ──────────────────────────────────────────────────────
    let lrc_path = path.with_extension("lrc");
    if lrc_path.exists() {
        let text = std::fs::read_to_string(&lrc_path)?;
        let lines = parse_lrc(&text);
        if !lines.is_empty() {
            return Ok(Lyrics { synced: true, lines });
        }
    }

    // ── 2. Embedded tag ───────────────────────────────────────────────────────
    use lofty::prelude::TaggedFileExt;
    use lofty::tag::ItemKey;

    // read_from_path is fallible; on error skip to empty result.
    let embedded = (|| -> Option<String> {
        let tagged = lofty::read_from_path(path).ok()?;
        let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
        // Try the generic Lyrics key first (works for Vorbis/FLAC/APEv2/MP4).
        if let Some(s) = tag.get_string(ItemKey::Lyrics) {
            return Some(s.to_owned());
        }
        // Fallback to unsynchronised lyrics (ID3v2 USLT).
        tag.get_string(ItemKey::UnsyncLyrics).map(|s| s.to_owned())
    })();

    if let Some(text) = embedded {
        let lines: Vec<LyricLine> = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| LyricLine { t_secs: None, text: l.to_owned() })
            .collect();
        if !lines.is_empty() {
            return Ok(Lyrics { synced: false, lines });
        }
    }

    // ── 3. Nothing found ──────────────────────────────────────────────────────
    Ok(Lyrics { synced: false, lines: vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build a timestamped LRC string from (mm, ss, centiseconds, text)
    // e.g. lrc_line(0, 10, 20, "Hello") → "[00:10.20]Hello"
    fn tl(mm: u32, ss: u32, cs: u32, text: &str) -> String {
        format!("[{:02}:{:02}.{:02}]{}", mm, ss, cs, text)
    }

    #[test]
    fn parses_three_line_synced_lyric() {
        let lrc = format!(
            "{}\n{}\n{}",
            tl(0, 1, 0, "Line one"),
            tl(0, 15, 50, "Line two"),
            tl(1, 2, 10, "Line three"),
        );
        let lines = parse_lrc(&lrc);
        assert_eq!(lines.len(), 3);
        assert!((lines[0].t_secs.unwrap() - 1.0).abs() < 0.01);
        assert_eq!(lines[0].text, "Line one");
        assert!((lines[1].t_secs.unwrap() - 15.5).abs() < 0.01);
        assert_eq!(lines[1].text, "Line two");
        assert!((lines[2].t_secs.unwrap() - 62.1).abs() < 0.01);
        assert_eq!(lines[2].text, "Line three");
    }

    #[test]
    fn multi_timestamp_line_expands() {
        // "[00:05.00][00:30.00]Chorus" → two LyricLines with the same text
        let lrc = "[00:05.00][00:30.00]Chorus";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 2);
        assert!((lines[0].t_secs.unwrap() - 5.0).abs() < 0.01);
        assert_eq!(lines[0].text, "Chorus");
        assert!((lines[1].t_secs.unwrap() - 30.0).abs() < 0.01);
        assert_eq!(lines[1].text, "Chorus");
    }

    #[test]
    fn skips_id_tags_and_blank_lines() {
        let lrc = "[ti:My Song]\n[ar:Some Artist]\n[al:Album]\n[by:Creator]\n[length:3:00]\n\n[00:01.00]Real line";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Real line");
    }

    #[test]
    fn malformed_bracket_ignored_gracefully() {
        // "[xx]" is not a time or known ID tag: produce one unsupported LyricLine
        // The important thing: no panic, and the real lines still parse.
        let lrc = "[xx]gibberish\n[00:02.00]Good line";
        let lines = parse_lrc(lrc);
        // We only care that parse_lrc doesn't panic and the timed line is present.
        let timed: Vec<_> = lines.iter().filter(|l| l.t_secs.is_some()).collect();
        assert_eq!(timed.len(), 1);
        assert_eq!(timed[0].text, "Good line");
    }

    #[test]
    fn sorts_ascending_by_time() {
        // Out-of-order timestamps should be sorted.
        let lrc = "[00:30.00]Second\n[00:05.00]First";
        let lines = parse_lrc(&lrc);
        assert_eq!(lines.len(), 2);
        assert!((lines[0].t_secs.unwrap() - 5.0).abs() < 0.01);
        assert_eq!(lines[0].text, "First");
        assert!((lines[1].t_secs.unwrap() - 30.0).abs() < 0.01);
        assert_eq!(lines[1].text, "Second");
    }

    #[test]
    fn handles_mm_ss_no_centiseconds() {
        let lrc = "[00:45]Verse";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 1);
        assert!((lines[0].t_secs.unwrap() - 45.0).abs() < 0.01);
    }

    #[test]
    fn read_lyrics_sidecar_lrc() {
        // Write a temp audio file path and a sidecar .lrc next to it.
        let dir = tempfile::tempdir().expect("tmp dir");
        let audio_path = dir.path().join("song.mp3");
        // The audio file need not actually exist for sidecar detection;
        // read_lyrics checks lrc_path.exists() independently.
        let lrc_path = dir.path().join("song.lrc");
        let lrc_content = "[00:01.00]Hello world\n[00:05.50]Goodbye";
        std::fs::write(&lrc_path, lrc_content).expect("write lrc");

        let lyrics = read_lyrics(&audio_path).expect("read_lyrics");
        assert!(lyrics.synced, "expected synced=true for .lrc sidecar");
        assert_eq!(lyrics.lines.len(), 2);
        assert_eq!(lyrics.lines[0].text, "Hello world");
        assert!((lyrics.lines[0].t_secs.unwrap() - 1.0).abs() < 0.01);
        assert_eq!(lyrics.lines[1].text, "Goodbye");
        assert!((lyrics.lines[1].t_secs.unwrap() - 5.5).abs() < 0.01);
    }
}
