use lofty::tag::{Accessor, Tag};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub duration_ms: Option<u64>,
}

/// Pure mapping from a single lofty Tag to TrackTags (no I/O).
pub fn tags_from_tag(tag: &Tag) -> TrackTags {
    use lofty::tag::ItemKey;
    TrackTags {
        title: tag.title().map(|s| s.into_owned()),
        artist: tag.artist().map(|s| s.into_owned()),
        album: tag.album().map(|s| s.into_owned()),
        album_artist: tag.get_string(ItemKey::AlbumArtist).map(|s| s.to_owned()),
        track_no: tag.track(),
        disc_no: tag.disk(),
        // date() returns Option<Timestamp>; Timestamp.year is u16
        year: tag.date().map(|ts| ts.year as i32),
        genre: tag.genre().map(|s| s.into_owned()),
        duration_ms: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::tag::{Tag, Accessor};
    use lofty::tag::TagType;

    #[test]
    fn maps_primary_tag_fields() {
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title("Awake".to_string());
        tag.set_artist("Tycho".to_string());
        tag.set_album("Blue Hour".to_string());
        tag.set_track(3);
        let tt = tags_from_tag(&tag);
        assert_eq!(tt.title.as_deref(), Some("Awake"));
        assert_eq!(tt.artist.as_deref(), Some("Tycho"));
        assert_eq!(tt.album.as_deref(), Some("Blue Hour"));
        assert_eq!(tt.track_no, Some(3));
    }
}
