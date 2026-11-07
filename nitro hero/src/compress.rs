//! Compressing videos on this device before they upload, and reading a video's first picture
//! from just its start, both with the ffmpeg that ships beside the app.
use std::{
    io::{BufRead, BufReader},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Duration,
};

/// Picture quality. Lower is better and larger: 22 looks the same as the source to nearly
/// everyone at chat size, and `slow` then spends the time making it smaller, not worse.
const QUALITY: &str = "22";
const EFFORT: &str = "slow";
/// However demanding the picture, its bitrate stops here. It's plenty for 720p.
const MOST_BITS_PER_SECOND: u64 = 5_000_000;
/// The whole file's budget, so even two minutes of confetti stays reasonable for the server.
const TARGET_BYTES: u64 = 48 * 1024 * 1024;
const AUDIO_BITS_PER_SECOND: u64 = 128_000;
/// Worth leaving alone: compressing it again would only cost quality.
const ALREADY_SMALL_BITS_PER_SECOND: u64 = 1_500_000;

const NO_WINDOW: u32 = 0x0800_0000;
/// Below normal, so that compressing never disturbs a call or the app itself.
const BELOW_NORMAL: u32 = 0x0000_4000;

/// Shared with the interface while compressing.
#[derive(Default)]
pub struct Progress {
    pub compressing: AtomicBool,
    pub percent: AtomicU8,
    pub cancelled: AtomicBool,
}

/// A temporary file that's removed when it's no longer needed.
pub struct TemporaryFile(pub PathBuf);

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn temporary(extension: &str) -> TemporaryFile {
    TemporaryFile(std::env::temp_dir().join(format!("nitro-{}.{extension}", uuid::Uuid::new_v4().simple())))
}

/// The ffmpeg beside the app. A development build can also use one on the PATH.
fn ffmpeg() -> Option<PathBuf> {
    let beside = std::env::current_exe().ok()?.with_file_name("nitro-ffmpeg.exe");
    if beside.exists() {
        return Some(beside);
    }
    if cfg!(debug_assertions) { Some(PathBuf::from("ffmpeg")) } else { None }
}

pub fn available() -> bool {
    ffmpeg().is_some_and(|path| {
        command(&path).arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|done| done.success())
    })
}

fn command(path: &Path) -> Command {
    let mut command = Command::new(path);
    command.stdin(Stdio::null()).creation_flags(NO_WINDOW | BELOW_NORMAL);
    command
}

/// "Duration: 00:01:02.50" from ffmpeg's description of a file.
fn parse_duration(text: &str) -> Option<f64> {
    let after = text.split("Duration: ").nth(1)?;
    let mut parts = after.split(',').next()?.trim().split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3600. + minutes * 60. + seconds).filter(|total| *total > 0.)
}

/// How long a file runs, by asking ffmpeg to describe it. It works without a compatible decoder.
pub fn duration(path: &Path) -> Option<f64> {
    let output = command(&ffmpeg()?).args(["-hide_banner", "-i"]).arg(path).output().ok()?;
    parse_duration(&String::from_utf8_lossy(&output.stderr))
}

/// The bitrate ceiling for a clip of this length, leaving room for its sound.
fn bitrate_ceiling(seconds: f64) -> u64 {
    let budget = (TARGET_BYTES * 8) as f64 / seconds.max(1.) - AUDIO_BITS_PER_SECOND as f64;
    (budget.max(800_000.) as u64).min(MOST_BITS_PER_SECOND)
}

/// Whether a file is already lean enough to send as it is.
pub fn already_small(bytes: u64, seconds: f64) -> bool {
    bytes as f64 * 8. / seconds.max(0.1) <= ALREADY_SMALL_BITS_PER_SECOND as f64
}

/// "out_time_us=1234" from ffmpeg's progress report, as a share of the whole.
fn progress_percent(line: &str, seconds: f64) -> Option<u8> {
    let micros: f64 = line.strip_prefix("out_time_us=")?.trim().parse().ok()?;
    Some((micros / 1_000_000. / seconds.max(0.1) * 100.).clamp(0., 99.) as u8)
}

/// Makes a phone or screen recording into an H.264 MP4 of at most 720p. Location and other
/// details in the file aren't carried over. `None` with `cancelled` set means they cancelled.
pub fn compress(source: &Path, seconds: f64, progress: &Arc<Progress>) -> Result<TemporaryFile, String> {
    let failed = || "This video couldn't be compressed.".to_string();
    let program = ffmpeg().ok_or_else(failed)?;
    let output = temporary("mp4");
    progress.compressing.store(true, Ordering::Release);
    let ceiling = bitrate_ceiling(seconds);
    let mut child = command(&program)
        .args(["-hide_banner", "-nostats", "-loglevel", "error", "-y", "-progress", "pipe:1", "-i"])
        .arg(source)
        .args(["-map", "0:v:0", "-map", "0:a:0?", "-map_metadata", "-1", "-map_chapters", "-1", "-sn", "-dn"])
        // The longer side to 1280 and the shorter to 720, keeping its shape and never enlarging.
        .args(["-vf", "scale=w='min(iw,if(gte(iw,ih),1280,720))':h='min(ih,if(gte(iw,ih),720,1280))':force_original_aspect_ratio=decrease:force_divisible_by=2:flags=lanczos,format=yuv420p"])
        .args(["-fpsmax", "60", "-c:v", "libx264", "-preset", EFFORT, "-crf", QUALITY, "-profile:v", "high"])
        .args(["-maxrate", &ceiling.to_string(), "-bufsize", &(ceiling * 2).to_string()])
        .args(["-c:a", "aac", "-b:a", &AUDIO_BITS_PER_SECOND.to_string(), "-ac", "2", "-movflags", "+faststart"])
        .arg(&output.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| failed())?;
    let stdout = child.stdout.take().ok_or_else(failed)?;
    let reporter = progress.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(percent) = progress_percent(&line, seconds) {
                reporter.percent.store(percent, Ordering::Release);
            }
        }
    });
    loop {
        if progress.cancelled.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(failed());
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(output),
            Ok(Some(_)) | Err(_) => return Err(failed()),
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// A file's first picture as a JPEG. The file can be only the start of a video.
pub fn first_frame(source: &Path, poster: &Path) -> Option<()> {
    let temp = temporary("jpg");
    let done = command(&ffmpeg()?)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(source)
        .args(["-map", "0:v:0", "-frames:v", "1", "-q:v", "3", "-f", "image2"])
        .arg(&temp.0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    if !done.success() || !temp.0.exists() {
        return None;
    }
    std::fs::rename(&temp.0, poster).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test clips come from a full ffmpeg on the PATH, as the bundled one can't invent pictures
    /// or write most formats. Skipped where there isn't one.
    fn sample(arguments: &[&str], extension: &str) -> Option<TemporaryFile> {
        let file = temporary(extension);
        let done = command(Path::new("ffmpeg")).args(["-hide_banner", "-loglevel", "error", "-y"]).args(arguments).arg(&file.0).status().ok()?;
        done.success().then_some(file)
    }

    /// What people actually have: phone, camera, screen recorder and old files. Each has to go
    /// all the way through, including with whichever ffmpeg is beside the test program.
    #[test]
    fn every_common_kind_of_source_compresses() {
        if !available() {
            return;
        }
        println!("using {:?}", ffmpeg());
        let picture = "testsrc2=s=320x180:d=1:r=30";
        let sound = "sine=frequency=440:duration=1";
        let cases: &[(&str, &[&str], &str)] = &[
            ("h264 + aac", &["-c:v", "libx264", "-c:a", "aac"], "mp4"),
            ("h264 high 10-bit", &["-c:v", "libx264", "-pix_fmt", "yuv420p10le", "-c:a", "aac"], "mp4"),
            ("hevc + aac, as phones", &["-c:v", "libx265", "-x265-params", "log-level=none", "-tag:v", "hvc1", "-c:a", "aac"], "mov"),
            ("hevc 10-bit", &["-c:v", "libx265", "-x265-params", "log-level=none", "-pix_fmt", "yuv420p10le", "-c:a", "aac"], "mp4"),
            ("vp9 + opus", &["-c:v", "libvpx-vp9", "-cpu-used", "8", "-b:v", "500k", "-c:a", "libopus"], "webm"),
            ("vp8 + vorbis", &["-c:v", "libvpx", "-c:a", "libvorbis"], "webm"),
            ("av1 + opus", &["-c:v", "libaom-av1", "-cpu-used", "8", "-b:v", "300k", "-c:a", "libopus"], "webm"),
            ("h264 + mp3 in mkv", &["-c:v", "libx264", "-c:a", "libmp3lame"], "mkv"),
            ("h264 + ac3 in ts", &["-c:v", "libx264", "-c:a", "ac3"], "ts"),
            ("h264 + flac in mkv", &["-c:v", "libx264", "-c:a", "flac"], "mkv"),
            ("mpeg4 + mp3 in avi", &["-c:v", "mpeg4", "-c:a", "libmp3lame"], "avi"),
            ("wmv2 + wmav2", &["-c:v", "wmv2", "-c:a", "wmav2"], "wmv"),
            ("prores + pcm", &["-c:v", "prores_ks", "-c:a", "pcm_s16le"], "mov"),
            ("mjpeg + pcm in avi", &["-c:v", "mjpeg", "-q:v", "5", "-pix_fmt", "yuvj420p", "-c:a", "pcm_s16le"], "avi"),
            ("h264 + alac", &["-c:v", "libx264", "-c:a", "alac"], "mov"),
            ("mpeg2 + mp2", &["-c:v", "mpeg2video", "-c:a", "mp2"], "mpg"),
            ("h264 + aac in flv", &["-c:v", "libx264", "-c:a", "aac"], "flv"),
            ("h264 + aac in 3gp", &["-c:v", "libx264", "-c:a", "aac"], "3gp"),
        ];
        let mut failed = Vec::new();
        for (name, encode, extension) in cases {
            let mut arguments = vec!["-f", "lavfi", "-i", picture, "-f", "lavfi", "-i", sound];
            arguments.extend_from_slice(encode);
            let Some(source) = sample(&arguments, extension) else {
                println!("{name}: the full ffmpeg can't make this here, not tested");
                continue;
            };
            let seconds = duration(&source.0);
            let result = compress(&source.0, seconds.unwrap_or(1.), &Arc::new(Progress::default()));
            let readable = result.as_ref().ok().and_then(|output| crate::video::probe(&output.0));
            let poster = temporary("jpg");
            let framed = first_frame(&source.0, &poster.0).is_some();
            println!("{name}: length {seconds:?}, compressed {}, first picture {framed}", readable.is_some());
            if seconds.is_none() || readable.is_none() || !framed {
                failed.push(*name);
            }
        }
        assert!(failed.is_empty(), "these failed: {failed:?}");
    }

    /// Phones store a rotation instead of turning the picture.
    #[test]
    fn a_rotated_phone_video_comes_out_upright() {
        if !available() {
            return;
        }
        let Some(flat) = sample(&["-f", "lavfi", "-i", "testsrc2=s=1280x720:d=1:r=30", "-c:v", "libx264", "-pix_fmt", "yuv420p"], "mp4") else { return };
        let rotated = temporary("mp4");
        let done = command(Path::new("ffmpeg"))
            .args(["-hide_banner", "-loglevel", "error", "-y", "-display_rotation", "90", "-i"])
            .arg(&flat.0)
            .args(["-c", "copy"])
            .arg(&rotated.0)
            .status()
            .unwrap();
        assert!(done.success());
        let output = compress(&rotated.0, 1., &Arc::new(Progress::default())).unwrap();
        let frame = crate::video::probe(&output.0).unwrap().first_frame;
        assert_eq!((frame.width, frame.height), (720, 1280));
    }

    #[test]
    fn ffmpeg_output_is_understood() {
        assert_eq!(parse_duration("  Duration: 00:01:02.50, start: 0.0"), Some(62.5));
        assert_eq!(parse_duration("  Duration: N/A, bitrate: N/A"), None);
        assert_eq!(parse_duration("no such thing"), None);
        assert_eq!(progress_percent("out_time_us=1000000", 4.), Some(25));
        assert_eq!(progress_percent("out_time_us=9000000", 4.), Some(99));
        assert_eq!(progress_percent("out_time_us=N/A", 4.), None);
        assert_eq!(progress_percent("frame=12", 4.), None);
    }

    #[test]
    fn a_long_clip_gets_a_ceiling_that_keeps_it_within_budget() {
        assert_eq!(bitrate_ceiling(10.), MOST_BITS_PER_SECOND);
        let two_minutes = bitrate_ceiling(120.);
        assert!(two_minutes < MOST_BITS_PER_SECOND);
        assert!((two_minutes + AUDIO_BITS_PER_SECOND) as f64 * 120. / 8. <= TARGET_BYTES as f64 * 1.001);
        assert!(already_small(1024 * 1024, 60.));
        assert!(!already_small(100 * 1024 * 1024, 60.));
    }

    #[test]
    fn a_large_video_comes_out_smaller_at_720p_keeping_its_sound() {
        if !available() {
            return;
        }
        let Some(source) = sample(
            &["-f", "lavfi", "-i", "testsrc2=s=1920x1080:d=2:r=30", "-f", "lavfi", "-i", "sine=frequency=440:duration=2",
              "-c:v", "libx264", "-crf", "10", "-preset", "ultrafast", "-pix_fmt", "yuv420p", "-metadata", "title=secret"],
            "mp4",
        ) else { return };
        assert_eq!(duration(&source.0).map(f64::round), Some(2.));
        let progress = Arc::new(Progress::default());
        let output = compress(&source.0, 2., &progress).unwrap();
        let (before, after) = (std::fs::metadata(&source.0).unwrap().len(), std::fs::metadata(&output.0).unwrap().len());
        assert!(after * 2 < before, "{before} became {after}");
        let info = crate::video::probe(&output.0).unwrap();
        assert_eq!((info.first_frame.width, info.first_frame.height), (1280, 720));
        let described = command(&ffmpeg().unwrap()).args(["-hide_banner", "-i"]).arg(&output.0).output().unwrap();
        let described = String::from_utf8_lossy(&described.stderr).to_string();
        assert!(described.contains("Audio: aac"), "{described}");
        assert!(!described.contains("secret"), "details were carried over");
    }

    #[test]
    fn portrait_small_and_cancelled_all_behave() {
        if !available() {
            return;
        }
        let Some(tall) = sample(&["-f", "lavfi", "-i", "testsrc2=s=1080x1920:d=1:r=30", "-pix_fmt", "yuv420p"], "mp4") else { return };
        let output = compress(&tall.0, 1., &Arc::new(Progress::default())).unwrap();
        let frame = crate::video::probe(&output.0).unwrap().first_frame;
        assert_eq!((frame.width, frame.height), (720, 1280));

        // Never enlarged, and silent clips work.
        let Some(small) = sample(&["-f", "lavfi", "-i", "testsrc2=s=322x182:d=1:r=30", "-pix_fmt", "yuv420p"], "mov") else { return };
        let output = compress(&small.0, 1., &Arc::new(Progress::default())).unwrap();
        let frame = crate::video::probe(&output.0).unwrap().first_frame;
        assert_eq!((frame.width, frame.height), (322, 182));

        let progress = Arc::new(Progress::default());
        progress.cancelled.store(true, Ordering::Release);
        assert!(compress(&tall.0, 1., &progress).is_err());

        let not_video = temporary("mp4");
        std::fs::write(&not_video.0, b"not a video").unwrap();
        assert!(compress(&not_video.0, 1., &Arc::new(Progress::default())).is_err());
        assert_eq!(duration(&not_video.0), None);
    }

    #[test]
    fn the_first_picture_comes_from_just_the_start_of_a_file() {
        if !available() {
            return;
        }
        let Some(source) = sample(&["-f", "lavfi", "-i", "testsrc2=s=1280x720:d=20:r=30", "-pix_fmt", "yuv420p", "-movflags", "+faststart"], "mp4") else { return };
        let bytes = std::fs::read(&source.0).unwrap();
        let start = temporary("mp4");
        std::fs::write(&start.0, &bytes[..bytes.len() / 4]).unwrap();
        let poster = temporary("jpg");
        first_frame(&start.0, &poster.0).unwrap();
        assert_eq!(image::image_dimensions(&poster.0).unwrap(), (1280, 720));
    }
}
