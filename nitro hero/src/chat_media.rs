//! Images attached in the composer. As in main Nitro the original goes up, and Cloudinary
//! makes the 1920px WebP that people actually download.
use std::{path::Path, sync::Arc};

const MAX_BYTES: usize = 10 * 1024 * 1024;
const MAX_PIXELS: u64 = 25_000_000;
const LARGEST_SIDE: u32 = 1920;
const THUMBNAIL_SIDE: u32 = 96;

pub struct PendingMedia {
    pub name: String,
    pub bytes: Vec<u8>,
    pub upload_mime: &'static str,
    pub width: u32,
    pub height: u32,
    pub thumbnail: Arc<gpui::RenderImage>,
}

/// What the upload needs, free of anything tied to the UI thread.
pub struct PendingUpload {
    pub bytes: Vec<u8>,
    pub upload_mime: &'static str,
    pub width: u32,
    pub height: u32,
}

/// The shown size. It's stored with the message, and the database refuses anything larger.
fn fit(width: u32, height: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= LARGEST_SIDE {
        return (width, height);
    }
    let scale = |side: u32| ((side as u64 * LARGEST_SIDE as u64 / longest as u64) as u32).max(1);
    (scale(width), scale(height))
}

fn mime_for(format: image::ImageFormat) -> Option<&'static str> {
    Some(match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Gif => "image/gif",
        image::ImageFormat::WebP => "image/webp",
        _ => return None,
    })
}

pub fn from_bytes(mut bytes: Vec<u8>, name: String) -> Result<PendingMedia, String> {
    const UNREADABLE: &str = "Only images can be attached for now.";
    let format = image::guess_format(&bytes).map_err(|_| UNREADABLE)?;
    let reader = image::ImageReader::with_format(std::io::Cursor::new(&bytes), format);
    let (width, height) = reader.into_dimensions().map_err(|_| UNREADABLE)?;
    if width as u64 * height as u64 > MAX_PIXELS {
        return Err("Images can be up to 25 megapixels.".into());
    }
    let decoded = image::load_from_memory_with_format(&bytes, format).map_err(|_| UNREADABLE)?;
    let upload_mime = match mime_for(format) {
        Some(mime) => mime,
        // A clipboard bitmap, which Cloudinary would store far larger than this.
        None => {
            let mut png = Vec::new();
            decoded
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .map_err(|_| UNREADABLE)?;
            bytes = png;
            "image/png"
        }
    };
    if bytes.len() > MAX_BYTES {
        return Err("Images can be up to 10 MiB.".into());
    }
    let mut pixels = decoded.thumbnail(THUMBNAIL_SIDE, THUMBNAIL_SIDE).to_rgba8();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let (width, height) = fit(width, height);
    Ok(PendingMedia {
        name,
        bytes,
        upload_mime,
        width,
        height,
        thumbnail: Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)])),
    })
}

pub fn from_file(path: &Path) -> Result<PendingMedia, String> {
    let size = std::fs::metadata(path).map_err(|_| "That file couldn't be opened.")?.len();
    if size > MAX_BYTES as u64 {
        return Err("Images can be up to 10 MiB.".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "That file couldn't be opened.")?;
    let name = path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
    from_bytes(bytes, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
        let mut out = Vec::new();
        image::RgbImage::new(width, height)
            .write_to(&mut std::io::Cursor::new(&mut out), format)
            .unwrap();
        out
    }

    #[test]
    fn the_stored_size_never_exceeds_what_the_database_allows() {
        assert_eq!(fit(4000, 2000), (1920, 960));
        assert_eq!(fit(2000, 4000), (960, 1920));
        assert_eq!(fit(800, 600), (800, 600));
        assert_eq!(fit(100_000, 1).1, 1);
    }

    #[test]
    fn originals_go_up_unchanged_and_bitmaps_become_png() {
        let jpeg = encoded(40, 20, image::ImageFormat::Jpeg);
        let media = from_bytes(jpeg.clone(), "a.jpg".into()).unwrap();
        assert_eq!((media.upload_mime, media.bytes.len()), ("image/jpeg", jpeg.len()));
        assert_eq!((media.width, media.height), (40, 20));
        let media = from_bytes(encoded(40, 20, image::ImageFormat::Bmp), "paste".into()).unwrap();
        assert_eq!(media.upload_mime, "image/png");
        assert_eq!(image::guess_format(&media.bytes).unwrap(), image::ImageFormat::Png);
    }

    #[test]
    fn things_that_are_not_images_are_refused() {
        assert!(from_bytes(b"not an image".to_vec(), "a.txt".into()).is_err());
    }
}
