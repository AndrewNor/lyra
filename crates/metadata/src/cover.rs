use crate::Result;
use std::path::Path;

/// An embedded cover image extracted from an audio file.
pub struct Cover {
    pub mime: String,
    pub data: Vec<u8>,
}

/// Read the first embedded picture from the file, if any.
pub fn read_cover(path: &Path) -> Result<Option<Cover>> {
    use lofty::prelude::TaggedFileExt;
    let tagged = lofty::read_from_path(path)?;
    let pic = tagged
        .tags()
        .iter()
        .flat_map(|t| t.pictures())
        .next();
    Ok(pic.map(|p| Cover {
        mime: p.mime_type().map(|m| m.as_str().to_owned()).unwrap_or_default(),
        data: p.data().to_vec(),
    }))
}

/// Write a cover picture into the primary tag of the file (for testing).
pub fn write_cover(path: &Path, mime: &str, data: &[u8]) -> Result<()> {
    use lofty::prelude::{TaggedFileExt, TagExt};
    use lofty::picture::{MimeType, Picture, PictureType};
    use lofty::tag::Tag;
    let mut tagged = lofty::read_from_path(path)?;
    if tagged.primary_tag().is_none() {
        let tt = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tt));
    }
    let tag = tagged.primary_tag_mut().expect("tag inserted above");
    let picture = Picture::unchecked(data.to_vec())
        .mime_type(MimeType::from_str(mime))
        .pic_type(PictureType::CoverFront)
        .build();
    tag.push_picture(picture);
    tag.save_to_path(path, lofty::config::WriteOptions::default())?;
    Ok(())
}
