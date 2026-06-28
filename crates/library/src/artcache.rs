use std::path::PathBuf;

use crate::Result;

/// Content-addressed on-disk thumbnail cache for album art.
pub struct ArtCache {
    dir: PathBuf,
}

impl ArtCache {
    pub fn new(dir: PathBuf) -> Self {
        ArtCache { dir }
    }

    /// Decode `image_bytes`, resize to ≤256×256 (preserving aspect), encode as PNG,
    /// write to `<dir>/<blake3-hex>.png`, and return the path.
    /// If the output file already exists, skip re-encoding (content-addressed dedup).
    pub fn store(&self, image_bytes: &[u8]) -> Result<PathBuf> {
        let hash = blake3::hash(image_bytes);
        let filename = format!("{}.png", hash.to_hex());
        let dest = self.dir.join(&filename);

        if dest.exists() {
            return Ok(dest);
        }

        let img = image::load_from_memory(image_bytes)?;
        let thumb = img.thumbnail(256, 256);
        thumb.save(&dest)?;

        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, _| image::Rgb([(x % 256) as u8, 0, 0]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn stores_thumbnail_and_dedups_by_hash() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ArtCache::new(dir.path().to_path_buf());
        let src = png_bytes(512, 400);
        let p1 = cache.store(&src).unwrap();
        assert!(p1.exists());
        let (w, h) = image::image_dimensions(&p1).unwrap();
        assert!(w <= 256 && h <= 256 && (w == 256 || h == 256));
        let mtime1 = std::fs::metadata(&p1).unwrap().modified().unwrap();
        let p2 = cache.store(&src).unwrap(); // same input
        assert_eq!(p1, p2); // same hash path
        assert_eq!(
            mtime1,
            std::fs::metadata(&p2).unwrap().modified().unwrap()
        ); // not rewritten
    }
}
