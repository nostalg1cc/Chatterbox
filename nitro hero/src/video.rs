//! Reading and playing chat videos with Windows Media Foundation, which is already on nearly
//! every Windows machine, so no video library has to ship with the app.
use crate::screen_view::Frame;
use std::{
    ffi::c_void,
    path::Path,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Media::MediaFoundation::*,
        System::{
            Com::{COINIT_MULTITHREADED, CoInitializeEx},
            LibraryLoader::{GetProcAddress, LoadLibraryW},
        },
    },
    core::{HRESULT, HSTRING, Interface, PCSTR, PCWSTR, s, w},
};

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const AUDIO: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
const ALL: u32 = MF_SOURCE_READER_ALL_STREAMS.0 as u32;
const ANY: u32 = MF_SOURCE_READER_ANY_STREAM.0 as u32;
const WHOLE_FILE: u32 = MF_SOURCE_READER_MEDIASOURCE.0 as u32;
const ENDED: u32 = MF_SOURCE_READERF_ENDOFSTREAM.0 as u32;
const FAILED: u32 = MF_SOURCE_READERF_ERROR.0 as u32;
const CHANGED: u32 = MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32;
/// A frame later than this is skipped, so a slow moment doesn't drift from the sound.
const LATE: Duration = Duration::from_millis(100);

/// What the file itself says, with its first picture.
pub struct VideoInfo {
    pub seconds: f64,
    pub first_frame: Frame,
}

/// Found when first needed, not linked. "N" editions of Windows ship without Media Foundation,
/// and a linked import would stop the whole app from starting there, not only video.
struct Library {
    create_attributes: unsafe extern "system" fn(*mut *mut c_void, u32) -> HRESULT,
    create_type: unsafe extern "system" fn(*mut *mut c_void) -> HRESULT,
    create_reader: unsafe extern "system" fn(PCWSTR, *mut c_void, *mut *mut c_void) -> HRESULT,
}

fn library() -> Option<&'static Library> {
    static LIBRARY: OnceLock<Option<Library>> = OnceLock::new();
    LIBRARY
        .get_or_init(|| unsafe {
            let platform = LoadLibraryW(w!("mfplat.dll")).ok()?;
            let readers = LoadLibraryW(w!("mfreadwrite.dll")).ok()?;
            let find = |module, name: PCSTR| GetProcAddress(module, name);
            let startup: unsafe extern "system" fn(u32, u32) -> HRESULT =
                std::mem::transmute(find(platform, s!("MFStartup"))?);
            let library = Library {
                create_attributes: std::mem::transmute(find(platform, s!("MFCreateAttributes"))?),
                create_type: std::mem::transmute(find(platform, s!("MFCreateMediaType"))?),
                create_reader: std::mem::transmute(find(readers, s!("MFCreateSourceReaderFromURL"))?),
            };
            startup(MF_VERSION, MFSTARTUP_FULL).ok().ok()?;
            Some(library)
        })
        .as_ref()
}

fn new_media_type(library: &Library) -> windows::core::Result<IMFMediaType> {
    unsafe {
        let mut raw = std::ptr::null_mut();
        (library.create_type)(&mut raw).ok()?;
        Ok(IMFMediaType::from_raw(raw))
    }
}

/// How a decoded picture sits in its buffer. Decoders pad to a multiple of 16, so a 360-row
/// video can arrive as 368 rows, with `shown` naming the part that's really the picture.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Layout {
    width: u32,
    height: u32,
    stride: i32,
    /// Left, top, width and height.
    shown: (u32, u32, u32, u32),
}

/// Reads an `MFVideoArea`: two offsets of a fraction and a whole number, then two sizes. It's
/// kept to the buffer, as these numbers come from the file.
fn shown_area(blob: &[u8], width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    let whole = |at: usize| Some(i16::from_le_bytes([*blob.get(at)?, *blob.get(at + 1)?]));
    let size = |at: usize| Some(i32::from_le_bytes(blob.get(at..at + 4)?.try_into().ok()?));
    let (left, top) = (whole(2)?, whole(6)?);
    let (shown_width, shown_height) = (size(8)?, size(12)?);
    if left < 0 || top < 0 || shown_width <= 0 || shown_height <= 0 {
        return None;
    }
    let (left, top) = (left as u32, top as u32);
    (left + shown_width as u32 <= width && top + shown_height as u32 <= height)
        .then_some((left, top, shown_width as u32, shown_height as u32))
}

struct Reader {
    reader: IMFSourceReader,
    layout: Layout,
    /// Channels and rate, when the file has sound and sound was asked for.
    audio: Option<(u16, u32)>,
}

impl Reader {
    /// Reads the picture's size and row layout, which can change once decoding has started.
    fn read_picture_layout(&mut self) -> windows::core::Result<()> {
        unsafe {
            let actual = self.reader.GetCurrentMediaType(VIDEO)?;
            let size = actual.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let (width, height) = ((size >> 32) as u32, size as u32);
            // With no stride given, plain RGB is stored bottom row first.
            let stride = actual.GetUINT32(&MF_MT_DEFAULT_STRIDE).map_or(-(width as i32) * 4, |stride| stride as i32);
            let mut blob = [0u8; 16];
            let shown = actual
                .GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut blob, None)
                .ok()
                .and_then(|()| shown_area(&blob, width, height))
                .unwrap_or((0, 0, width, height));
            self.layout = Layout { width, height, stride, shown };
        }
        Ok(())
    }
}

fn open(path: &Path, with_audio: bool) -> windows::core::Result<Reader> {
    let library = library().ok_or_else(windows::core::Error::empty)?;
    unsafe {
        // Already being set up on this thread is fine.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let mut raw = std::ptr::null_mut();
        (library.create_attributes)(&mut raw, 1).ok()?;
        let attributes = IMFAttributes::from_raw(raw);
        // Lets the reader convert whatever the file holds into plain pixels.
        attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
        let mut raw = std::ptr::null_mut();
        (library.create_reader)(PCWSTR(HSTRING::from(path.as_os_str()).as_ptr()), attributes.as_raw(), &mut raw).ok()?;
        let reader = IMFSourceReader::from_raw(raw);
        reader.SetStreamSelection(ALL, false)?;
        reader.SetStreamSelection(VIDEO, true)?;

        let wanted = new_media_type(library)?;
        wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
        reader.SetCurrentMediaType(VIDEO, None, &wanted)?;

        let audio = if with_audio { select_audio(library, &reader) } else { None };
        let mut opened = Reader { reader, layout: Layout::default(), audio };
        opened.read_picture_layout()?;
        Ok(opened)
    }
}

/// Sound is optional: plenty of clips have none, or have a kind Windows can't decode.
fn select_audio(library: &Library, reader: &IMFSourceReader) -> Option<(u16, u32)> {
    unsafe {
        reader.SetStreamSelection(AUDIO, true).ok()?;
        let wanted = new_media_type(library).ok()?;
        wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).ok()?;
        wanted.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float).ok()?;
        let layout = reader.SetCurrentMediaType(AUDIO, None, &wanted).ok().and_then(|()| {
            let actual = reader.GetCurrentMediaType(AUDIO).ok()?;
            let channels = actual.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).ok()?;
            let rate = actual.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).ok()?;
            (channels > 0 && channels <= u32::from(u16::MAX) && rate > 0).then_some((channels as u16, rate))
        });
        if layout.is_none() {
            let _ = reader.SetStreamSelection(AUDIO, false);
        }
        layout
    }
}

/// The real index of the first video stream, since `ReadSample` reports real indexes.
fn video_stream_index(reader: &IMFSourceReader) -> u32 {
    let mut index = 0;
    while let Ok(kind) = unsafe { reader.GetNativeMediaType(index, 0) } {
        if unsafe { kind.GetGUID(&MF_MT_MAJOR_TYPE) }.is_ok_and(|major| major == MFMediaType_Video) {
            return index;
        }
        index += 1;
    }
    0
}

fn sample_bytes(sample: &IMFSample) -> windows::core::Result<Vec<u8>> {
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer()?;
        let mut start = std::ptr::null_mut();
        let mut length = 0;
        buffer.Lock(&mut start, None, Some(&mut length))?;
        let bytes = std::slice::from_raw_parts(start, length as usize).to_vec();
        let _ = buffer.Unlock();
        Ok(bytes)
    }
}

/// The shown part as top-down BGRA with a solid alpha, however it's stored.
fn to_frame(bytes: &[u8], layout: Layout) -> Option<Frame> {
    let Layout { width, height, stride, shown: (left, top, shown_width, shown_height) } = layout;
    let pitch = stride.unsigned_abs() as usize;
    let row = shown_width as usize * 4;
    let coded_row = width as usize * 4;
    if row == 0 || shown_height == 0 || pitch < coded_row || bytes.len() < pitch * (height as usize - 1) + coded_row {
        return None;
    }
    let mut pixels = Vec::with_capacity(row * shown_height as usize);
    for line in (top..top + shown_height).map(|line| line as usize) {
        let stored = if stride < 0 { height as usize - 1 - line } else { line };
        let start = stored * pitch + left as usize * 4;
        pixels.extend_from_slice(&bytes[start..start + row]);
    }
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    Some(Frame { width: shown_width, height: shown_height, pixels })
}

/// Runs on a thread of its own. Media Foundation needs COM set up a particular way on the
/// thread it uses, and the app's shared threads also run things like the file picker.
fn on_own_thread<T: Send>(work: impl FnOnce() -> T + Send) -> Option<T> {
    std::thread::scope(|scope| scope.spawn(work).join().ok())
}

/// The length and first picture, or `None` if Windows can't read this file.
pub fn probe(path: &Path) -> Option<VideoInfo> {
    on_own_thread(|| read_first_frame(path)).flatten()
}

fn read_first_frame(path: &Path) -> Option<VideoInfo> {
    let mut opened = open(path, false).ok()?;
    let seconds = unsafe {
        let duration = opened.reader.GetPresentationAttribute(WHOLE_FILE, &MF_PD_DURATION).ok()?;
        u64::try_from(&duration).ok()? as f64 / 10_000_000.
    };
    for _ in 0..60 {
        let (mut flags, mut sample) = (0, None);
        unsafe {
            opened.reader.ReadSample(VIDEO, 0, None, Some(&mut flags), None, Some(&mut sample)).ok()?;
        }
        if flags & (ENDED | FAILED) != 0 {
            return None;
        }
        if flags & CHANGED != 0 {
            opened.read_picture_layout().ok()?;
        }
        if let Some(sample) = sample {
            let bytes = sample_bytes(&sample).ok()?;
            let first_frame = to_frame(&bytes, opened.layout)?;
            return Some(VideoInfo { seconds, first_frame });
        }
    }
    None
}

/// Saves a video's first picture beside it, for showing in chat before it's played again.
pub fn save_poster(path: &Path, poster: &Path) -> Option<()> {
    let frame = probe(path)?.first_frame;
    let mut pixels = frame.pixels;
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let picture = image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(frame.width, frame.height, pixels)?);
    let sized = picture.resize(840, 840, image::imageops::FilterType::Triangle).to_rgb8();
    let mut bytes = Vec::new();
    sized.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg).ok()?;
    crate::backend::write_whole(poster, &bytes).ok()
}

/// What the interface and the playing thread share.
#[derive(Default)]
struct Shared {
    latest: Mutex<Option<Frame>>,
    stop: AtomicBool,
    ended: AtomicBool,
    muted: AtomicBool,
    paused: AtomicBool,
    /// 0 to 100.
    volume: AtomicU8,
    /// Where to jump to, in seconds, when it's next looked at. Only the newest matters.
    seek: Mutex<Option<f64>>,
    position_ms: AtomicU64,
    length_ms: AtomicU64,
}

impl Shared {
    /// Heard on a curve, as with soundboard clips.
    fn level(&self) -> f32 {
        if self.muted.load(Ordering::Relaxed) {
            return 0.;
        }
        let share = f32::from(self.volume.load(Ordering::Relaxed)) / 100.;
        share * share
    }
}

/// A video playing on its own thread. Dropping it stops the picture and the sound.
pub struct Player(Arc<Shared>);

impl Drop for Player {
    fn drop(&mut self) {
        self.0.stop.store(true, Ordering::Release);
    }
}

impl Player {
    pub fn start(path: &Path, muted: bool) -> Result<Self, ()> {
        // Opened here so that a file Windows can't read is known about straight away.
        on_own_thread(|| open(path, false).is_ok()).filter(|readable| *readable).ok_or(())?;
        let shared = Arc::new(Shared::default());
        shared.muted.store(muted, Ordering::Relaxed);
        shared.volume.store(85, Ordering::Relaxed);
        let (path, thread_shared) = (path.to_path_buf(), shared.clone());
        std::thread::spawn(move || {
            let _ = play(&path, &thread_shared);
            thread_shared.ended.store(true, Ordering::Release);
        });
        Ok(Self(shared))
    }

    /// The newest picture since the last call.
    pub fn take(&self) -> Option<Frame> {
        self.0.latest.lock().ok()?.take()
    }

    pub fn ended(&self) -> bool {
        self.0.ended.load(Ordering::Acquire)
    }

    /// For when the person mutes it, or deafens themselves in a call.
    pub fn set_muted(&self, muted: bool) {
        self.0.muted.store(muted, Ordering::Relaxed);
    }

    /// 0 to 100.
    pub fn set_volume(&self, volume: u8) {
        self.0.volume.store(volume.min(100), Ordering::Relaxed);
    }

    pub fn paused(&self) -> bool {
        self.0.paused.load(Ordering::Acquire)
    }

    pub fn set_paused(&self, paused: bool) {
        self.0.paused.store(paused, Ordering::Release);
    }

    pub fn position(&self) -> f64 {
        self.0.position_ms.load(Ordering::Relaxed) as f64 / 1000.
    }

    /// Its length in seconds, or 0 until that's known.
    pub fn length(&self) -> f64 {
        self.0.length_ms.load(Ordering::Relaxed) as f64 / 1000.
    }

    /// Jumps to a time. The position shows it at once, so a dragged handle doesn't spring back.
    pub fn seek(&self, seconds: f64) {
        let length = self.length();
        let seconds = if length > 0. { seconds.clamp(0., length) } else { seconds.max(0.) };
        self.0.position_ms.store((seconds * 1000.) as u64, Ordering::Relaxed);
        if let Ok(mut seek) = self.0.seek.lock() {
            *seek = Some(seconds);
        }
    }
}

fn seek_reader(reader: &IMFSourceReader, seconds: f64) -> windows::core::Result<()> {
    let position = windows::Win32::System::Com::StructuredStorage::PROPVARIANT::from((seconds * 10_000_000.) as i64);
    unsafe { reader.SetCurrentPosition(&windows::core::GUID::zeroed(), &position) }
}

fn play(path: &Path, shared: &Shared) -> windows::core::Result<()> {
    let mut opened = open(path, true)?;
    let video_index = video_stream_index(&opened.reader);
    if let Ok(length) = unsafe { opened.reader.GetPresentationAttribute(WHOLE_FILE, &MF_PD_DURATION) }
        && let Ok(length) = u64::try_from(&length)
    {
        shared.length_ms.store(length / 10_000, Ordering::Relaxed);
    }
    let sink = opened.audio.and_then(|_| crate::audio::media_sink());
    let streams = if opened.audio.is_some() { 2 } else { 1 };
    let mut streams_left = streams;
    // The moment a timestamp of zero would have been shown. Everything is timed against this.
    let mut zero = None::<Instant>;
    // After a jump: nothing earlier than this is shown or heard.
    let mut skip_before = 0i64;
    // After a jump while paused, one picture still has to appear.
    let mut show_one = false;
    let mut pause_began = None::<Instant>;
    loop {
        if shared.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(sink) = &sink {
            sink.set_volume(shared.level());
        }
        if let Some(seconds) = shared.seek.lock().ok().and_then(|mut seek| seek.take()) {
            seek_reader(&opened.reader, seconds)?;
            if let Some(sink) = &sink {
                sink.clear();
            }
            zero = None;
            skip_before = (seconds * 10_000_000.) as i64;
            streams_left = streams;
            show_one = true;
        }
        let paused = shared.paused.load(Ordering::Acquire);
        match (paused, pause_began) {
            (true, None) => {
                pause_began = Some(Instant::now());
                if let Some(sink) = &sink {
                    sink.pause();
                }
            }
            (false, Some(began)) => {
                // The clock skips the time spent paused.
                zero = zero.map(|zero| zero + began.elapsed());
                pause_began = None;
            }
            _ => {}
        }
        if !paused
            && let Some(sink) = &sink
            && sink.is_paused()
        {
            sink.play();
        }
        if paused && !show_one {
            std::thread::sleep(Duration::from_millis(15));
            continue;
        }
        if streams_left == 0 {
            // The picture can finish first, with sound still queued.
            if sink.as_ref().is_none_or(rodio::Sink::empty) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(30));
            continue;
        }
        let (mut stream, mut flags, mut time, mut sample) = (0, 0, 0i64, None);
        unsafe {
            opened.reader.ReadSample(ANY, 0, Some(&mut stream), Some(&mut flags), Some(&mut time), Some(&mut sample))?;
        }
        // After this the reader won't deliver anything more, so asking again would spin.
        if flags & FAILED != 0 {
            return Ok(());
        }
        if flags & ENDED != 0 {
            streams_left -= 1;
            // Jumped past the last picture while paused: there's none to wait for.
            show_one = false;
        }
        if flags & CHANGED != 0 && stream == video_index {
            opened.read_picture_layout()?;
        }
        let Some(sample) = sample else { continue };
        // A jump lands on the keyframe before it, which has to be decoded through, not shown.
        if time < skip_before {
            continue;
        }
        let bytes = sample_bytes(&sample)?;
        let at = Duration::from_nanos(time.max(0) as u64 * 100);
        let started = *zero.get_or_insert_with(|| Instant::now().checked_sub(at).unwrap_or_else(Instant::now));
        if stream != video_index {
            if let (Some((channels, rate)), Some(sink)) = (opened.audio, &sink) {
                let samples = bytes
                    .chunks_exact(4)
                    .map(|sound| f32::from_le_bytes([sound[0], sound[1], sound[2], sound[3]]))
                    .collect::<Vec<_>>();
                sink.append(rodio::buffer::SamplesBuffer::new(channels, rate, samples));
            }
            continue;
        }
        // Waits for this picture's moment, while staying ready to stop, pause or jump.
        let mut interrupted = false;
        while !show_one && started.elapsed() < at {
            let waiting = shared.stop.load(Ordering::Acquire)
                || shared.paused.load(Ordering::Acquire)
                || shared.seek.lock().is_ok_and(|seek| seek.is_some());
            if waiting {
                interrupted = true;
                break;
            }
            std::thread::sleep((at - started.elapsed()).min(Duration::from_millis(8)));
        }
        // Paused while waiting: this picture still shows, so nothing is lost on resuming.
        let late = !show_one && !interrupted && started.elapsed() > at + LATE;
        if !late
            && let Some(frame) = to_frame(&bytes, opened.layout)
            && let Ok(mut slot) = shared.latest.lock()
        {
            *slot = Some(frame);
        }
        show_one = false;
        if shared.seek.lock().is_ok_and(|seek| seek.is_none()) {
            shared.position_ms.store((time.max(0) / 10_000) as u64, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn frames_come_out_top_down_with_solid_alpha() {
        // Two rows of one pixel each, with four bytes of padding per row.
        let bytes = [1, 2, 3, 0, 9, 9, 9, 9, 4, 5, 6, 0, 9, 9, 9, 9];
        let layout = |stride| Layout { width: 1, height: 2, stride, shown: (0, 0, 1, 2) };
        assert_eq!(to_frame(&bytes, layout(8)).unwrap().pixels, [1, 2, 3, 255, 4, 5, 6, 255]);
        assert_eq!(to_frame(&bytes, layout(-8)).unwrap().pixels, [4, 5, 6, 255, 1, 2, 3, 255]);
        assert!(to_frame(&bytes[..4], layout(8)).is_none());
        assert!(to_frame(&[], Layout::default()).is_none());
    }

    #[test]
    fn decoder_padding_is_cut_off_either_way_up() {
        // Three rows of two pixels, of which only the middle row's second pixel is the picture.
        let bytes: Vec<u8> = (0..24).collect();
        let layout = |stride| Layout { width: 2, height: 3, stride, shown: (1, 1, 1, 1) };
        assert_eq!(to_frame(&bytes, layout(8)).unwrap().pixels, [12, 13, 14, 255]);
        assert_eq!(to_frame(&bytes, layout(-8)).unwrap().pixels, [12, 13, 14, 255]);
    }

    #[test]
    fn a_shown_area_outside_the_picture_is_ignored() {
        let area = |left: i16, top: i16, width: i32, height: i32| {
            let mut blob = vec![0, 0];
            blob.extend(left.to_le_bytes());
            blob.extend([0, 0]);
            blob.extend(top.to_le_bytes());
            blob.extend(width.to_le_bytes());
            blob.extend(height.to_le_bytes());
            blob
        };
        assert_eq!(shown_area(&area(0, 0, 640, 360), 640, 368), Some((0, 0, 640, 360)));
        assert_eq!(shown_area(&area(8, 4, 640, 360), 640, 368), None);
        assert_eq!(shown_area(&area(-1, 0, 10, 10), 640, 368), None);
        assert_eq!(shown_area(&area(0, 0, 0, 10), 640, 368), None);
        assert_eq!(shown_area(&[0; 4], 640, 368), None);
    }

    #[test]
    fn something_that_is_not_a_video_is_not_read_as_one() {
        let file = std::env::temp_dir().join(format!("hero-not-video-{}.mp4", uuid::Uuid::new_v4()));
        std::fs::write(&file, b"not a video").unwrap();
        assert!(probe(&file).is_none());
        assert!(Player::start(&file, true).is_err());
        let _ = std::fs::remove_file(file);
    }

    /// A real file, red on top and blue below, so a flipped or skewed picture shows up.
    /// Skipped where ffmpeg isn't installed.
    fn sample_video(width: u32, height: u32, extension: &str, with_sound: bool) -> Option<std::path::PathBuf> {
        let file = std::env::temp_dir().join(format!("hero-video-{}.{extension}", uuid::Uuid::new_v4()));
        let half = height / 2;
        let picture = format!("color=c=red:s={width}x{height}:d=1:r=30,drawbox=x=0:y={half}:w={width}:h={half}:color=blue:t=fill");
        let mut command = Command::new("ffmpeg");
        command.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", &picture]);
        if with_sound {
            command.args(["-f", "lavfi", "-i", "sine=frequency=440:duration=1"]);
        }
        command.args(["-pix_fmt", "yuv420p"]).arg(&file);
        command.output().ok().filter(|done| done.status.success()).map(|_| file)
    }

    fn colour_at(frame: &Frame, x: u32, y: u32) -> [u8; 3] {
        let at = (y * frame.width + x) as usize * 4;
        [frame.pixels[at + 2], frame.pixels[at + 1], frame.pixels[at]]
    }

    #[test]
    fn real_files_read_the_right_way_up_at_awkward_sizes() {
        for (width, height) in [(640, 360), (1920, 1080), (322, 182), (360, 640)] {
            let Some(file) = sample_video(width, height, "mp4", false) else { return };
            let info = probe(&file).unwrap_or_else(|| panic!("{width}x{height} wasn't readable"));
            let _ = std::fs::remove_file(&file);
            assert!((info.seconds - 1.).abs() < 0.2, "{width}x{height}: {} seconds", info.seconds);
            let frame = info.first_frame;
            assert_eq!((frame.width, frame.height), (width, height));
            let (top, bottom) = (colour_at(&frame, width / 2, height / 8), colour_at(&frame, width / 2, height * 7 / 8));
            assert!(top[0] > 180 && top[2] < 80, "{width}x{height}: top is {top:?}");
            assert!(bottom[2] > 180 && bottom[0] < 80, "{width}x{height}: bottom is {bottom:?}");
        }
    }

    /// Not a check, as this depends on what's installed: run with --nocapture to see the list.
    #[test]
    fn report_which_file_types_this_machine_reads() {
        for extension in ["mp4", "m4v", "mov", "webm", "mkv", "avi", "wmv"] {
            let Some(file) = sample_video(320, 180, extension, false) else {
                println!("{extension}: ffmpeg couldn't make one");
                continue;
            };
            println!("{extension}: {}", if probe(&file).is_some() { "reads" } else { "NOT READABLE" });
            let _ = std::fs::remove_file(file);
        }
    }

    #[test]
    fn a_real_file_pauses_seeks_and_resumes() {
        let Some(file) = long_sample() else { return };
        let player = Player::start(&file, true).unwrap();
        let wait_until = |test: &dyn Fn(&Player) -> bool, what: &str| {
            let begun = Instant::now();
            while !test(&player) {
                assert!(begun.elapsed() < Duration::from_secs(5), "timed out: {what}");
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        wait_until(&|player| player.position() > 0.3, "it to start");
        assert!((player.length() - 6.).abs() < 0.3, "{}", player.length());

        // Paused, time stands still.
        player.set_paused(true);
        std::thread::sleep(Duration::from_millis(200));
        let held = player.position();
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(player.position(), held, "it moved while paused");
        let _ = player.take();

        // A jump while paused still shows the picture there.
        player.seek(4.);
        assert_eq!(player.position(), 4., "the position should show a jump at once");
        wait_until(&|player| player.take().is_some(), "a picture after jumping");
        assert!((player.position() - 4.).abs() < 0.2, "{}", player.position());
        std::thread::sleep(Duration::from_millis(200));
        assert!((player.position() - 4.).abs() < 0.2, "still paused, but it's at {}", player.position());

        // Resumed, it carries on from there in real time, not in a rush to catch up.
        player.set_paused(false);
        let resumed = Instant::now();
        wait_until(&|player| player.position() > 4.5, "it to resume");
        assert!(resumed.elapsed() > Duration::from_millis(350), "it rushed");

        // Backwards works too, and it still reaches the end.
        player.seek(0.5);
        wait_until(&|player| player.position() < 1.5, "the jump back");
        player.seek(5.8);
        wait_until(&Player::ended, "the end");
        let _ = std::fs::remove_file(file);
    }

    fn long_sample() -> Option<std::path::PathBuf> {
        let file = std::env::temp_dir().join(format!("hero-video-{}.mp4", uuid::Uuid::new_v4()));
        let mut command = Command::new("ffmpeg");
        command.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=s=320x180:d=6:r=30",
            "-f", "lavfi", "-i", "sine=frequency=440:duration=6", "-g", "15", "-pix_fmt", "yuv420p"]).arg(&file);
        command.output().ok().filter(|done| done.status.success()).map(|_| file)
    }

    #[test]
    fn a_real_file_plays_to_its_end_and_can_be_stopped() {
        let Some(file) = sample_video(320, 180, "mp4", true) else { return };
        let player = Player::start(&file, true).unwrap();
        let begun = Instant::now();
        let mut frames = 0;
        while !player.ended() && begun.elapsed() < Duration::from_secs(10) {
            if let Some(frame) = player.take() {
                assert_eq!((frame.width, frame.height), (320, 180));
                frames += 1;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(player.ended(), "still playing after ten seconds");
        assert!(frames > 15, "only {frames} frames arrived");
        assert!(begun.elapsed() > Duration::from_millis(700), "it should play in real time");

        let playing = Player::start(&file, true).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let shared = playing.0.clone();
        drop(playing);
        let stopped = Instant::now();
        while !shared.ended.load(Ordering::Acquire) && stopped.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(shared.ended.load(Ordering::Acquire), "stopping should end the thread");
        let _ = std::fs::remove_file(file);
    }
}
