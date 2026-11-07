//! Images and videos attached in the composer. As in main Nitro the original goes up, and
//! Cloudinary makes the 1920px WebP or the MP4 that people actually download.
use crate::compress::{self, Progress};
use std::{path::Path, sync::Arc};

const MAX_BYTES: usize = 10 * 1024 * 1024;
const MAX_PIXELS: u64 = 25_000_000;
const LARGEST_SIDE: u32 = 1920;
const THUMBNAIL_SIDE: u32 = 96;
/// The server's limits for a video, and the same as main Nitro's.
const MAX_VIDEO_BYTES: u64 = 100 * 1024 * 1024;
const MAX_VIDEO_SECONDS: f64 = 120.;

pub struct PendingMedia {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub upload_mime: &'static str,
    pub width: u32,
    pub height: u32,
    /// Set for a video.
    pub seconds: Option<f64>,
    pub thumbnail: Arc<gpui::RenderImage>,
}

/// What the upload needs, free of anything tied to the UI thread.
pub struct PendingUpload {
    pub bytes: Arc<Vec<u8>>,
    pub upload_mime: &'static str,
    pub width: u32,
    pub height: u32,
    pub seconds: Option<f64>,
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
    const UNREADABLE: &str = "Only images and videos can be attached.";
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
        bytes: Arc::new(bytes),
        upload_mime,
        width,
        height,
        seconds: None,
        thumbnail: Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)])),
    })
}

/// The upload type for a video file, going by its name. Cloudinary works out the rest.
fn video_mime(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "wmv" => "video/x-ms-wmv",
        "ts" | "mts" | "m2ts" => "video/mp2t",
        "mpg" | "mpeg" => "video/mpeg",
        "flv" => "video/x-flv",
        "3gp" => "video/3gpp",
        _ => return None,
    })
}

/// The stored size of a video: what the database allows, with even sides.
fn fit_video(width: u32, height: u32) -> (u32, u32) {
    let (long, short) = (width.max(height) as f64, width.min(height) as f64);
    let scale = (1280. / long).min(720. / short).min(1.);
    let even = |side: u32| (((side as f64 * scale / 2.).round() as u32) * 2).max(2);
    (even(width), even(height))
}

fn from_video(path: &Path, upload_mime: &'static str, progress: &Arc<Progress>) -> Result<PendingMedia, String> {
    const UNSUPPORTED: &str = "This video format isn't supported.";
    let size = std::fs::metadata(path).map_err(|_| "That file couldn't be opened.")?.len();
    let source = crate::video::probe(path);
    // Without ffmpeg it can still go up as it is, if the server would take it.
    let can_compress = compress::available();
    let seconds = source.as_ref().map(|info| info.seconds).filter(|seconds| *seconds > 0.).or_else(|| compress::duration(path));
    let seconds = seconds.ok_or(UNSUPPORTED)?;
    if seconds > MAX_VIDEO_SECONDS {
        return Err("Videos can be up to 2 minutes.".to_string());
    }
    let fits_as_it_is = source.as_ref().is_some_and(|info| {
        let (width, height) = (info.first_frame.width, info.first_frame.height);
        fit_video(width, height) == (width, height)
    });
    // What's already lean only loses quality by being compressed again.
    let lean = upload_mime == "video/mp4" && fits_as_it_is && compress::already_small(size, seconds);
    let compressed = if can_compress && !lean {
        let output = compress::compress(path, seconds, progress);
        if progress.cancelled.load(std::sync::atomic::Ordering::Acquire) {
            return Err(String::new());
        }
        Some(output?)
    } else {
        None
    };
    // It could come out larger than a source that was already efficient.
    let compressed = compressed.filter(|output| {
        let smaller = std::fs::metadata(&output.0).is_ok_and(|info| info.len() < size);
        smaller || upload_mime != "video/mp4" || !fits_as_it_is
    });
    let (file, upload_mime): (&Path, &'static str) = match &compressed {
        Some(output) => (&output.0, "video/mp4"),
        None => (path, upload_mime),
    };
    let size = std::fs::metadata(file).map_err(|_| "That file couldn't be opened.")?.len();
    if size > MAX_VIDEO_BYTES {
        return Err("Videos can be up to 100 MiB.".to_string());
    }
    let info = crate::video::probe(file).ok_or(UNSUPPORTED)?;
    let frame = info.first_frame;
    let (width, height) = fit_video(frame.width, frame.height);
    let picture = image::RgbaImage::from_raw(frame.width, frame.height, frame.pixels).ok_or(UNSUPPORTED)?;
    let pixels = image::DynamicImage::ImageRgba8(picture).thumbnail(THUMBNAIL_SIDE, THUMBNAIL_SIDE).to_rgba8();
    Ok(PendingMedia {
        name: path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(),
        bytes: Arc::new(std::fs::read(file).map_err(|_| "That file couldn't be opened.")?),
        upload_mime,
        width,
        height,
        seconds: Some(if info.seconds > 0. { info.seconds } else { seconds }),
        thumbnail: Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)])),
    })
}

pub fn from_file(path: &Path, progress: &Arc<Progress>) -> Result<PendingMedia, String> {
    if let Some(upload_mime) = video_mime(path) {
        return from_video(path, upload_mime, progress);
    }
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
    fn videos_are_stored_within_what_the_database_allows() {
        assert_eq!(fit_video(1920, 1080), (1280, 720));
        assert_eq!(fit_video(1080, 1920), (720, 1280));
        assert_eq!(fit_video(640, 360), (640, 360));
        assert_eq!(fit_video(3840, 720), (1280, 240));
        assert_eq!(video_mime(Path::new("clip.MP4")), Some("video/mp4"));
        assert_eq!(video_mime(Path::new("photo.png")), None);
    }

    #[test]
    fn things_that_are_not_images_are_refused() {
        assert!(from_bytes(b"not an image".to_vec(), "a.txt".into()).is_err());
    }
}
