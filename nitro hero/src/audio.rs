//! Interface sounds and soundboard playback. Sounds match the main app: short synthesized
//! tones for ordinary interaction, recorded WAVs for mute, messages, reactions and voice.
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source, buffer::SamplesBuffer};
use std::{
    io::Cursor,
    path::Path,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
        mpsc,
    },
};

#[derive(Clone, Copy, Debug)]
pub enum UiSound {
    Click,
    MenuOpen,
    MenuClose,
    MuteOn,
    MuteOff,
    Message,
    Reaction,
    VoiceJoin,
    VoiceLeave,
}

/// The main app attenuates its full-scale WAVs by this much.
const FILE_GAIN: f32 = 0.45;
const SAMPLE_RATE: u32 = 48_000;

static ENABLED: AtomicBool = AtomicBool::new(true);
static VOLUME: AtomicU8 = AtomicU8::new(65);
pub static SOUNDS_PLAYED: AtomicUsize = AtomicUsize::new(0);

/// Set from preferences each frame.
pub fn configure(enabled: bool, volume: u8) {
    ENABLED.store(enabled, Ordering::Relaxed);
    VOLUME.store(volume, Ordering::Relaxed);
}

/// The output stream must stay on the thread that created it, so it lives on its own.
fn output() -> Option<&'static OutputStreamHandle> {
    static HANDLE: OnceLock<Option<OutputStreamHandle>> = OnceLock::new();
    HANDLE
        .get_or_init(|| {
            let (sender, receiver) = mpsc::channel();
            std::thread::spawn(move || match OutputStream::try_default() {
                Ok((stream, handle)) => {
                    let _ = sender.send(Some(handle));
                    let _stream: OutputStream = stream;
                    loop {
                        std::thread::park();
                    }
                }
                Err(_) => {
                    let _ = sender.send(None);
                }
            });
            receiver.recv().ok().flatten()
        })
        .as_ref()
}

struct Tone {
    frequency: f32,
    offset: f32,
    duration: f32,
    volume: f32,
}

const fn tone(frequency: f32, offset: f32, duration: f32, volume: f32) -> Tone {
    Tone { frequency, offset, duration, volume }
}

const ACTION: [Tone; 2] = [tone(466.16, 0., 0.035, 0.013), tone(554.37, 0.04, 0.052, 0.018)];
const REVERSE: [Tone; 2] = [tone(554.37, 0., 0.052, 0.018), tone(466.16, 0.04, 0.035, 0.013)];

fn synthesize(tones: &[Tone], scale: f32) -> Vec<f32> {
    let end = tones
        .iter()
        .map(|tone| tone.offset + tone.duration)
        .fold(0., f32::max);
    let mut samples = vec![0f32; ((end + 0.02) * SAMPLE_RATE as f32) as usize];
    for tone in tones {
        let start = (tone.offset * SAMPLE_RATE as f32) as usize;
        let count = (tone.duration * SAMPLE_RATE as f32) as usize;
        let attack = 0.008 * SAMPLE_RATE as f32;
        for index in 0..count {
            let time = index as f32;
            let peak = (tone.volume * scale).max(0.0001);
            // Exponential ramps, like the main app's oscillator gain.
            let gain = if time < attack {
                0.0001 * (peak / 0.0001).powf(time / attack)
            } else {
                peak * (0.0001 / peak).powf((time - attack) / (count as f32 - attack).max(1.))
            };
            let phase = std::f32::consts::TAU * tone.frequency * time / SAMPLE_RATE as f32;
            if let Some(sample) = samples.get_mut(start + index) {
                *sample += phase.sin() * gain;
            }
        }
    }
    samples
}

/// Plays an interface sound if they're enabled.
pub fn play(sound: UiSound) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let scale = VOLUME.load(Ordering::Relaxed) as f32 / 100.;
    let tones: Option<&[Tone]> = match sound {
        UiSound::Click | UiSound::MenuOpen => Some(&ACTION),
        UiSound::MenuClose => Some(&REVERSE),
        _ => None,
    };
    let bytes: &'static [u8] = match sound {
        UiSound::MuteOn => include_bytes!("../assets/sounds/toggle_negative.wav"),
        UiSound::MuteOff => include_bytes!("../assets/sounds/toggle_positive.wav"),
        UiSound::Message => include_bytes!("../assets/sounds/message_new.wav"),
        UiSound::Reaction => include_bytes!("../assets/sounds/reaction.wav"),
        UiSound::VoiceJoin => include_bytes!("../assets/sounds/voice_connected_join.wav"),
        UiSound::VoiceLeave => include_bytes!("../assets/sounds/voice_disconnect.wav"),
        _ => &[],
    };
    std::thread::spawn(move || {
        let Some(handle) = output() else { return };
        let Ok(sink) = Sink::try_new(handle) else { return };
        if let Some(tones) = tones {
            sink.append(SamplesBuffer::new(1, SAMPLE_RATE, synthesize(tones, scale)));
        } else if let Ok(source) = Decoder::new(Cursor::new(bytes)) {
            sink.set_volume(scale * FILE_GAIN);
            sink.append(source.convert_samples::<f32>());
        } else {
            return;
        }
        SOUNDS_PLAYED.fetch_add(1, Ordering::Relaxed);
        sink.detach();
    });
}

/// A decoded soundboard clip.
pub struct DecodedClip {
    pub samples: Vec<f32>,
    pub channels: u16,
}

impl DecodedClip {
    pub fn duration_ms(&self) -> u64 {
        let frames = self.samples.len() as u64 / self.channels.max(1) as u64;
        frames * 1000 / SAMPLE_RATE as u64
    }
}

/// Decodes a WebM/Opus soundboard file, the format the main app uploads.
pub fn decode_clip(path: &Path) -> Result<DecodedClip, String> {
    use matroska_demuxer::{Frame, MatroskaFile};
    let file = std::fs::File::open(path).map_err(|err| err.to_string())?;
    let mut matroska = MatroskaFile::open(std::io::BufReader::new(file))
        .map_err(|_| "This sound's file couldn't be read.".to_string())?;
    let track = matroska
        .tracks()
        .iter()
        .find(|track| track.codec_id() == "A_OPUS")
        .ok_or("This sound isn't in a supported format.")?;
    let track_number = track.track_number().get();
    let channels = track
        .audio()
        .map(|audio| audio.channels().get() as usize)
        .unwrap_or(2)
        .clamp(1, 2);
    let mut decoder =
        opus_decoder::OpusDecoder::new(SAMPLE_RATE, channels).map_err(|err| format!("{err:?}"))?;
    let mut samples = Vec::new();
    let mut packet = vec![0f32; decoder.max_frame_size_per_channel() * channels];
    let mut frame = Frame::default();
    while matroska
        .next_frame(&mut frame)
        .map_err(|_| "This sound's file is damaged.".to_string())?
    {
        if frame.track != track_number {
            continue;
        }
        // A bad clip must only fail that clip, never take the app down.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            decoder.decode_float(&frame.data, &mut packet, false)
        }));
        match result {
            Ok(Ok(decoded)) => samples.extend_from_slice(&packet[..decoded * channels]),
            Ok(Err(_)) => {}
            Err(_) => return Err("This sound couldn't be decoded.".into()),
        }
    }
    if samples.is_empty() {
        return Err("This sound has no audio.".into());
    }
    Ok(DecodedClip { samples, channels: channels as u16 })
}

static CURRENT_CLIP: Mutex<Option<Sink>> = Mutex::new(None);

/// Plays mono samples in the clip slot, such as a trim preview.
pub fn play_samples(samples: Vec<f32>, volume: u8) -> Result<(), String> {
    play_clip(&DecodedClip { samples, channels: 1 }, volume)
}

/// Loudness is heard on a curve, so the slider follows one: 50% is a quarter of the signal,
/// where a straight line would still sound almost as loud as 100%.
fn clip_gain(volume: u8) -> f32 {
    let level = volume as f32 / 100.;
    level * level
}

/// Starts a clip, replacing any clip already playing.
pub fn play_clip(clip: &DecodedClip, volume: u8) -> Result<(), String> {
    let handle = output().ok_or("No audio output is available.")?;
    let sink = Sink::try_new(handle).map_err(|err| err.to_string())?;
    sink.set_volume(clip_gain(volume));
    sink.append(SamplesBuffer::new(clip.channels, SAMPLE_RATE, clip.samples.clone()));
    SOUNDS_PLAYED.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut current) = CURRENT_CLIP.lock() {
        *current = Some(sink);
    }
    Ok(())
}

pub fn stop_clip() {
    if let Ok(mut current) = CURRENT_CLIP.lock() {
        *current = None;
    }
}

pub fn set_clip_volume(volume: u8) {
    if let Ok(current) = CURRENT_CLIP.lock()
        && let Some(sink) = current.as_ref()
    {
        sink.set_volume(clip_gain(volume));
    }
}
