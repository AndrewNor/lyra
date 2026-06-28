//! lyra-metadata: audio tag read/write + cover extraction.

mod tags;
mod cover;

pub use tags::{tags_from_tag, TrackTags};
pub use cover::{Cover, read_cover, write_cover};

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Lofty(#[from] lofty::error::LoftyError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

pub fn read_tags(path: &Path) -> Result<TrackTags> {
    use lofty::prelude::{AudioFile, TaggedFileExt};
    let tagged = lofty::read_from_path(path)?;
    let mut tt = tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .map(tags_from_tag)
        .unwrap_or_default();
    tt.duration_ms = Some(tagged.properties().duration().as_millis() as u64);
    Ok(tt)
}

pub fn write_tags(path: &Path, tags: &TrackTags) -> Result<()> {
    use lofty::prelude::{Accessor, TaggedFileExt, TagExt};
    use lofty::tag::Tag;
    let mut tagged = lofty::read_from_path(path)?;
    if tagged.primary_tag().is_none() {
        let tt = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tt));
    }
    let tag = tagged.primary_tag_mut().expect("tag inserted above");
    if let Some(v) = &tags.title  { tag.set_title(v.clone()); }
    if let Some(v) = &tags.artist { tag.set_artist(v.clone()); }
    if let Some(v) = &tags.album  { tag.set_album(v.clone()); }
    if let Some(n) = tags.track_no { tag.set_track(n); }
    tag.save_to_path(path, lofty::config::WriteOptions::default())?;
    Ok(())
}
