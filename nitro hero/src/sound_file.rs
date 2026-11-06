//! Prepares a soundboard clip the way main Nitro does: any common audio file becomes a
//! trimmed, normalized, 48 kHz mono Opus WebM of at most 15 seconds and 512 KiB.
use std::path::Path;

pub const MAX_SOURCE_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_CLIP_MS: u32 = 15_000;
pub const MIN_CLIP_MS: u32 = 100;
const MAX_CLIP_BYTES: usize = 512 * 1024;
/// Far beyond any clip, and it keeps a long file from filling memory while decoding.
const MAX_SOURCE_SECONDS: usize = 10 * 60;
const RATE: usize = 48_000;
const BITRATE: i32 = 96_000;
const FRAME: usize = 960;

/// Reads an audio file as 48 kHz mono.
pub fn decode_source(path: &Path) -> Result<Vec<f32>, String> {
    let size = std::fs::metadata(path).map_err(|_| "That file couldn't be opened.")?.len();
    if size > MAX_SOURCE_BYTES {
        return Err("Source audio can be up to 25 MiB.".into());
    }
    let samples = match crate::audio::decode_clip(path) {
        // Main Nitro's own clips, which the general decoder below can't read.
        Ok(clip) => to_mono(&clip.samples, clip.channels as usize),
        Err(_) => decode_general(path)?,
    };
    if samples.len() < RATE * MIN_CLIP_MS as usize / 1000 {
        return Err("The sound is too short.".into());
    }
    Ok(samples)
}

fn to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

fn decode_general(path: &Path) -> Result<Vec<f32>, String> {
    use symphonia::core::{
        audio::SampleBuffer, codecs::{CODEC_TYPE_NULL, DecoderOptions}, errors::Error,
        formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
    };
    const UNREADABLE: &str = "This audio format couldn't be read.";
    let file = std::fs::File::open(path).map_err(|_| "That file couldn't be opened.")?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|_| UNREADABLE)?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(UNREADABLE)?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|_| UNREADABLE)?;
    let mut mono = Vec::new();
    let mut source_rate = track.codec_params.sample_rate.unwrap_or(RATE as u32) as usize;
    let mut buffer: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(_)) => break, // The end of the file.
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // One damaged packet shouldn't lose the rest of the sound.
            Err(Error::DecodeError(_)) => continue,
            Err(_) => break,
        };
        let spec = *decoded.spec();
        source_rate = spec.rate as usize;
        let channels = spec.channels.count();
        let target = buffer.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
        if target.capacity() < decoded.capacity() * channels {
            *target = SampleBuffer::new(decoded.capacity() as u64, spec);
        }
        target.copy_interleaved_ref(decoded);
        mono.extend(to_mono(target.samples(), channels));
        if mono.len() > source_rate * MAX_SOURCE_SECONDS {
            return Err("Source audio can be up to 10 minutes.".into());
        }
    }
    if mono.is_empty() {
        return Err(UNREADABLE.into());
    }
    Ok(resample(&mono, source_rate))
}

/// Linear between samples. When shrinking, each output sample averages the inputs it
/// covers, which keeps high-rate sources from folding noise into the audible range.
fn resample(samples: &[f32], from: usize) -> Vec<f32> {
    if from == RATE {
        return samples.to_vec();
    }
    let step = from as f64 / RATE as f64;
    let length = (samples.len() as f64 / step) as usize;
    (0..length)
        .map(|index| {
            let position = index as f64 * step;
            if step > 1. {
                let start = position as usize;
                let end = (((index + 1) as f64 * step) as usize).clamp(start + 1, samples.len());
                samples[start..end].iter().sum::<f32>() / (end - start) as f32
            } else {
                let start = position as usize;
                let next = samples.get(start + 1).copied().unwrap_or(samples[start]);
                samples[start] + (next - samples[start]) * (position - start as f64) as f32
            }
        })
        .collect()
}

/// The selected part, without silence at its edges, at a consistent loudness.
fn selection(samples: &[f32], start_ms: u32, end_ms: u32) -> &[f32] {
    let at = |ms: u32| (ms as usize * RATE / 1000).min(samples.len());
    let part = &samples[at(start_ms)..at(end_ms)];
    let threshold = 0.002;
    let first = part.iter().position(|value| value.abs() >= threshold).unwrap_or(0);
    let last = part.iter().rposition(|value| value.abs() >= threshold).map_or(part.len(), |index| index + 1);
    let padding = RATE / 50;
    &part[first.saturating_sub(padding)..(last + padding).min(part.len())]
}

fn normalized(part: &[f32]) -> Vec<f32> {
    let peak = part.iter().fold(0f32, |peak, value| peak.max(value.abs()));
    let rms = (part.iter().map(|value| value * value).sum::<f32>() / part.len().max(1) as f32).sqrt();
    let gain = 4f32.min(0.92 / peak.max(0.001)).min(0.14 / rms.max(0.001));
    part.iter().map(|value| value * gain).collect()
}

/// The clip as it will sound once saved, for previewing.
pub fn preview(samples: &[f32], start_ms: u32, end_ms: u32) -> Vec<f32> {
    normalized(selection(samples, start_ms, end_ms))
}

pub struct Prepared {
    pub webm: Vec<u8>,
    pub duration_ms: u32,
}

pub fn prepare(samples: &[f32], start_ms: u32, end_ms: u32) -> Result<Prepared, String> {
    let clip = preview(samples, start_ms, end_ms);
    let duration_ms = (clip.len() * 1000 / RATE) as u32;
    if !(MIN_CLIP_MS..=MAX_CLIP_MS).contains(&duration_ms) {
        return Err("Choose a clip between 0.1 and 15 seconds.".into());
    }
    let webm = encode(&clip).map_err(|_| "This sound couldn't be compressed.".to_string())?;
    if webm.len() > MAX_CLIP_BYTES {
        return Err("This clip is too large once compressed. Try a shorter selection.".into());
    }
    Ok(Prepared { webm, duration_ms })
}

fn encode(samples: &[f32]) -> Result<Vec<u8>, opus::Error> {
    let mut encoder = opus::Encoder::new(RATE as u32, opus::Channels::Mono, opus::Application::Audio)?;
    encoder.set_bitrate(opus::Bitrate::Bits(BITRATE))?;
    let pre_skip = encoder.get_lookahead()? as u16;
    // Past the end by the encoder's delay, so the last real samples aren't cut off.
    let mut input = samples.to_vec();
    let total = (input.len() + pre_skip as usize).div_ceil(FRAME) * FRAME;
    input.resize(total, 0.);
    let mut packets = Vec::with_capacity(total / FRAME);
    for frame in input.chunks_exact(FRAME) {
        packets.push(encoder.encode_vec_float(frame, 1500)?);
    }
    Ok(mux_webm(&packets, pre_skip, samples.len()))
}

fn vint(mut value: u64) -> Vec<u8> {
    let mut length = 1;
    while length < 8 && value >= (1u64 << (7 * length)) - 1 {
        length += 1;
    }
    value |= 1u64 << (7 * length);
    value.to_be_bytes()[8 - length..].to_vec()
}

fn element(id: &[u8], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend(vint(body.len() as u64));
    out.extend_from_slice(body);
    out
}

fn uint(id: &[u8], value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
    element(id, &bytes[first..])
}

fn float(id: &[u8], value: f64) -> Vec<u8> {
    element(id, &value.to_be_bytes())
}

/// The WebM that browsers and Hero's own decoder both read: one Opus track and one cluster,
/// which is enough since a clip can't outgrow a cluster's 32 seconds.
fn mux_webm(packets: &[Vec<u8>], pre_skip: u16, sample_count: usize) -> Vec<u8> {
    let header = [
        element(&[0x42, 0x86], &[1]),
        element(&[0x42, 0xF7], &[1]),
        element(&[0x42, 0xF2], &[4]),
        element(&[0x42, 0xF3], &[8]),
        element(&[0x42, 0x82], b"webm"),
        element(&[0x42, 0x87], &[4]),
        element(&[0x42, 0x85], &[2]),
    ]
    .concat();
    let duration_ms = sample_count as f64 * 1000. / RATE as f64;
    let info = element(
        &[0x15, 0x49, 0xA9, 0x66],
        &[
            uint(&[0x2A, 0xD7, 0xB1], 1_000_000),
            element(&[0x4D, 0x80], b"Nitro"),
            element(&[0x57, 0x41], b"Nitro"),
            float(&[0x44, 0x89], duration_ms),
        ]
        .concat(),
    );
    let mut opus_head = b"OpusHead".to_vec();
    opus_head.push(1);
    opus_head.push(1);
    opus_head.extend_from_slice(&pre_skip.to_le_bytes());
    opus_head.extend_from_slice(&(RATE as u32).to_le_bytes());
    opus_head.extend_from_slice(&[0, 0, 0]);
    let audio = element(
        &[0xE1],
        &[float(&[0xB5], RATE as f64), uint(&[0x9F], 1), uint(&[0x62, 0x64], 32)].concat(),
    );
    let track = element(
        &[0xAE],
        &[
            uint(&[0xD7], 1),
            uint(&[0x73, 0xC5], 1),
            uint(&[0x83], 2),
            element(&[0x86], b"A_OPUS"),
            element(&[0x63, 0xA2], &opus_head),
            uint(&[0x56, 0xAA], pre_skip as u64 * 1_000_000_000 / RATE as u64),
            uint(&[0x56, 0xBB], 80_000_000),
            audio,
        ]
        .concat(),
    );
    let tracks = element(&[0x16, 0x54, 0xAE, 0x6B], &track);
    let mut cluster = uint(&[0xE7], 0);
    for (index, packet) in packets.iter().enumerate() {
        let time = (index * FRAME * 1000 / RATE) as i16;
        let mut block = vec![0x81];
        block.extend_from_slice(&time.to_be_bytes());
        block.push(0x80); // A keyframe, as every Opus packet can start decoding.
        block.extend_from_slice(packet);
        cluster.extend(element(&[0xA3], &block));
    }
    let cluster = element(&[0x1F, 0x43, 0xB6, 0x75], &cluster);
    let segment = element(&[0x18, 0x53, 0x80, 0x67], &[info, tracks, cluster].concat());
    [element(&[0x1A, 0x45, 0xDF, 0xA3], &header), segment].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(seconds * RATE as f32) as usize)
            .map(|index| (index as f32 * 440. * std::f32::consts::TAU / RATE as f32).sin() * amplitude)
            .collect()
    }

    fn temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("hero-sound-{}-{name}", uuid::Uuid::new_v4()));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_saved_clip_decodes_again_at_the_right_length_and_pitch() {
        let prepared = prepare(&tone(2., 0.5), 0, 2000).unwrap();
        assert_eq!(prepared.duration_ms, 2000);
        assert!(prepared.webm.len() < MAX_CLIP_BYTES);
        let path = temp("clip.webm", &prepared.webm);
        let clip = crate::audio::decode_clip(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(clip.channels, 1);
        let seconds = clip.samples.len() as f32 / RATE as f32;
        assert!((seconds - 2.).abs() < 0.05, "{seconds}");
        // Count rising zero crossings well inside the clip: 440 per second.
        let middle = &clip.samples[RATE / 2..RATE / 2 + RATE];
        let crossings = middle.windows(2).filter(|pair| pair[0] < 0. && pair[1] >= 0.).count();
        assert!((430..=450).contains(&crossings), "{crossings}");
    }

    /// Writes a sample clip to check in a browser, which is how main Nitro plays these.
    #[test]
    #[ignore]
    fn write_sample_clip() {
        let out = std::env::var("NITRO_SAMPLE_CLIP").unwrap();
        std::fs::write(out, prepare(&tone(0.5, 0.5), 0, 500).unwrap().webm).unwrap();
    }

    #[test]
    fn trim_limits_are_enforced() {
        let samples = tone(20., 0.5);
        assert!(prepare(&samples, 0, 16_000).is_err());
        assert!(prepare(&samples, 0, 50).is_err());
        assert_eq!(prepare(&samples, 1000, 4000).unwrap().duration_ms, 3000);
    }

    #[test]
    fn the_longest_clip_fits_the_servers_limit() {
        let noisy: Vec<f32> = tone(15., 0.5)
            .iter()
            .enumerate()
            .map(|(index, value)| value + ((index * 7919) % 1000) as f32 / 4000.)
            .collect();
        assert!(prepare(&noisy, 0, 15_000).unwrap().webm.len() <= MAX_CLIP_BYTES);
    }

    #[test]
    fn silence_at_the_edges_goes_and_loudness_is_evened_out() {
        let mut samples = vec![0.; RATE];
        samples.extend(tone(1., 0.05));
        samples.extend(vec![0.; RATE]);
        let clip = preview(&samples, 0, 3000);
        assert!(clip.len() < RATE + RATE / 10, "{}", clip.len());
        let peak = clip.iter().fold(0f32, |peak, value| peak.max(value.abs()));
        assert!(peak > 0.15 && peak <= 0.93, "{peak}");
    }

    #[test]
    fn a_wav_file_is_read_at_any_rate_as_48k_mono() {
        // One second of stereo at 44.1 kHz.
        let frames = 44_100u32;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + frames * 4).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&44_100u32.to_le_bytes());
        wav.extend_from_slice(&(44_100u32 * 4).to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(frames * 4).to_le_bytes());
        for index in 0..frames {
            let value = ((index as f32 * 440. * std::f32::consts::TAU / 44_100.).sin() * 8000.) as i16;
            wav.extend_from_slice(&value.to_le_bytes());
            wav.extend_from_slice(&value.to_le_bytes());
        }
        let path = temp("clip.wav", &wav);
        let samples = decode_source(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!((samples.len() as i32 - RATE as i32).abs() < 2400, "{}", samples.len());
    }

    #[test]
    fn something_that_is_not_audio_is_refused() {
        let path = temp("clip.mp3", b"this is not audio at all");
        assert!(decode_source(&path).is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn element_sizes_use_the_shortest_length_form() {
        assert_eq!(vint(0), [0x80]);
        assert_eq!(vint(126), [0xFE]);
        assert_eq!(vint(127), [0x40, 0x7F]);
    }
}
