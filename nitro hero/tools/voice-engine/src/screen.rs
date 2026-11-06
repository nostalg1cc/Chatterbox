//! Screen sharing. Separate peer connections from voice, so a share can't starve the call.
use libwebrtc::{
    desktop_capturer::{
        CaptureError, CaptureSource, DesktopCaptureSourceType, DesktopCapturer,
        DesktopCapturerOptions, DesktopFrame,
    },
    ice_candidate::IceCandidate,
    media_stream_track::MediaStreamTrack,
    native::yuv_helper,
    peer_connection::{AnswerOptions, IceGatheringState, OfferOptions, PeerConnection},
    peer_connection_factory::{
        PeerConnectionFactory, RtcConfiguration, native::PeerConnectionFactoryExt,
    },
    rtp_parameters::{DegradationPreference, RtpEncodingParameters},
    rtp_transceiver::{RtpTransceiverDirection, RtpTransceiverInit},
    session_description::{SdpType, SessionDescription},
    video_frame::{I420Buffer, VideoBuffer, VideoFrame, VideoRotation},
    video_source::{VideoResolution, native::NativeVideoSource},
    video_stream::native::NativeVideoStream,
    video_track::RtcVideoTrack,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    net::TcpStream,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tokio_stream::StreamExt;

/// The most a share may send, matching main Nitro. Egress is what Cloudflare bills.
pub const MAX_BITRATE: u64 = 2_500_000;
const MAX_FRAMERATE: f64 = 30.;
const CAPTURE_INTERVAL: Duration = Duration::from_millis(33);
const MAX_CAPTURE_WIDTH: u32 = 1920;
/// What's handed to the app for display. The panel is rarely wider than this.
const MAX_VIEW_WIDTH: u32 = 1600;
const VIEW_INTERVAL: Duration = Duration::from_millis(33);
/// Your own share, shown back to you. It stays on this machine, so it can be gentler.
const MAX_PREVIEW_WIDTH: u32 = 960;
const PREVIEW_INTERVAL: Duration = Duration::from_millis(66);
// Cloudflare is ice-lite on a public address, so a share never waits on slow candidates.
const GATHERING_TIMEOUT: Duration = Duration::from_millis(1500);

pub fn sources() -> Vec<Value> {
    let mut list = Vec::new();
    for (kind, source_type) in [
        ("screen", DesktopCaptureSourceType::Screen),
        ("window", DesktopCaptureSourceType::Window),
    ] {
        let Some(capturer) = DesktopCapturer::new(capturer_options(source_type)) else {
            continue;
        };
        for (index, source) in capturer.get_source_list().into_iter().enumerate() {
            let title = source.title();
            let title = if kind == "screen" {
                format!("Screen {}", index + 1)
            } else if title.trim().is_empty() {
                continue;
            } else {
                title
            };
            list.push(json!({"id":source.id().to_string(),"kind":kind,"title":title}));
        }
    }
    list
}

fn capturer_options(source_type: DesktopCaptureSourceType) -> DesktopCapturerOptions {
    let mut options = DesktopCapturerOptions::new(source_type);
    options.set_include_cursor(true);
    options
}

fn find_source(capturer: &DesktopCapturer, id: u64) -> Option<CaptureSource> {
    capturer.get_source_list().into_iter().find(|source| source.id() == id)
}

fn even_size(width: u32, height: u32, largest: u32) -> (u32, u32) {
    let (width, height) = if width > largest {
        (largest, (height as u64 * largest as u64 / width as u64) as u32)
    } else {
        (width, height)
    };
    ((width & !1).max(2), (height & !1).max(2))
}

/// Hands frames to the app over a private local socket, at a limited rate and size.
struct FrameOutput {
    socket: TcpStream,
    largest: u32,
    interval: Duration,
    last: Instant,
    pixels: Vec<u8>,
}

impl FrameOutput {
    fn connect(port: u16, token: &str, largest: u32, interval: Duration) -> Option<Self> {
        let mut socket = TcpStream::connect(("127.0.0.1", port)).ok()?;
        socket.set_nodelay(true).ok()?;
        socket.write_all(token.as_bytes()).ok()?;
        Some(Self { socket, largest, interval, last: Instant::now() - interval, pixels: Vec::new() })
    }

    /// Header, then BGRA pixels: width and height as little-endian u32s. False once closed.
    fn send(&mut self, buffer: &mut I420Buffer) -> bool {
        if self.last.elapsed() < self.interval {
            return true;
        }
        self.last = Instant::now();
        let (width, height) = even_size(buffer.width(), buffer.height(), self.largest);
        let scaled;
        let shown = if width != buffer.width() || height != buffer.height() {
            scaled = buffer.scale(width as i32, height as i32);
            &scaled
        } else {
            &*buffer
        };
        self.pixels.resize((width * height * 4) as usize, 0);
        let (y_stride, u_stride, v_stride) = shown.strides();
        let (y, u, v) = shown.data();
        // libyuv's ARGB is B, G, R, A in memory, which is the order GPUI wants.
        yuv_helper::i420_to_argb(
            y,
            y_stride,
            u,
            u_stride,
            v,
            v_stride,
            &mut self.pixels,
            width * 4,
            width as i32,
            height as i32,
        );
        let header = [width.to_le_bytes(), height.to_le_bytes()].concat();
        self.socket.write_all(&header).is_ok() && self.socket.write_all(&self.pixels).is_ok()
    }
}

/// Sends one BGRA frame into the video source, scaled down if it's too large.
fn send_bgra(
    source: &NativeVideoSource,
    preview: &mut Option<FrameOutput>,
    data: &[u8],
    stride: u32,
    width: u32,
    height: u32,
) {
    let (even_width, even_height) = (width & !1, height & !1);
    if even_width < 2 || even_height < 2 {
        return;
    }
    let mut buffer = I420Buffer::new(even_width, even_height);
    let (y_stride, u_stride, v_stride) = buffer.strides();
    let (y, u, v) = buffer.data_mut();
    yuv_helper::argb_to_i420(
        data,
        stride,
        y,
        y_stride,
        u,
        u_stride,
        v,
        v_stride,
        even_width as i32,
        even_height as i32,
    );
    let (out_width, out_height) = even_size(even_width, even_height, MAX_CAPTURE_WIDTH);
    let mut buffer = if out_width != even_width {
        buffer.scale(out_width as i32, out_height as i32)
    } else {
        buffer
    };
    // A failed preview must not interrupt the share itself.
    if let Some(output) = preview
        && !output.send(&mut buffer)
    {
        *preview = None;
    }
    let mut frame = VideoFrame::new(VideoRotation::VideoRotation0, buffer);
    frame.timestamp_us = 0;
    source.capture_frame(&frame);
}

/// Holds a steady rate: the time a frame took to capture counts toward its interval, and a
/// late frame goes out straight away rather than pushing all the later ones back.
fn pace(next: &mut Instant) {
    *next += CAPTURE_INTERVAL;
    let now = Instant::now();
    if *next > now {
        std::thread::sleep(*next - now);
    } else {
        *next = now;
    }
}

fn run_capture(
    source: NativeVideoSource,
    mut preview: Option<FrameOutput>,
    kind: String,
    id: u64,
    stop: Arc<AtomicBool>,
    tx: mpsc::SyncSender<Value>,
) {
    let source_type = if kind == "window" {
        DesktopCaptureSourceType::Window
    } else {
        DesktopCaptureSourceType::Screen
    };
    let Some(mut capturer) = DesktopCapturer::new(capturer_options(source_type)) else {
        let _ = tx.try_send(json!({"event":"screen-ended","reason":"capture-unavailable"}));
        return;
    };
    let Some(selected) = find_source(&capturer, id) else {
        let _ = tx.try_send(json!({"event":"screen-ended","reason":"source-gone"}));
        return;
    };
    let failed = Arc::new(AtomicBool::new(false));
    let failure = failed.clone();
    capturer.start_capture(
        Some(selected),
        move |result: Result<DesktopFrame, CaptureError>| match result {
            Ok(frame) => send_bgra(
                &source,
                &mut preview,
                frame.data(),
                frame.stride(),
                frame.width() as u32,
                frame.height() as u32,
            ),
            // A temporary failure is normal, for example during a display mode change.
            Err(CaptureError::Temporary) => {}
            Err(CaptureError::Permanent) => failure.store(true, Ordering::Release),
        },
    );
    let mut next = Instant::now();
    while !stop.load(Ordering::Acquire) {
        capturer.capture_frame();
        if failed.load(Ordering::Acquire) {
            let _ = tx.try_send(json!({"event":"screen-ended","reason":"source-gone"}));
            return;
        }
        pace(&mut next);
    }
}

/// A moving pattern for transport tests, which never touches the real screen.
fn run_test_pattern(source: NativeVideoSource, mut preview: Option<FrameOutput>, stop: Arc<AtomicBool>) {
    const WIDTH: u32 = 1280;
    const HEIGHT: u32 = 720;
    let mut tick = 0u32;
    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    let mut next = Instant::now();
    while !stop.load(Ordering::Acquire) {
        // Rows of a moving pattern, cheap to draw and not trivial to encode.
        for (y, row) in pixels.chunks_exact_mut((WIDTH * 4) as usize).enumerate() {
            let phase = (y as u32 / 8 * 37 + tick * 6) as u8;
            for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                pixel[0] = phase.wrapping_add((x / 16 * 53) as u8);
                pixel[1] = phase.wrapping_mul(3);
                pixel[2] = 200;
                pixel[3] = 255;
            }
        }
        send_bgra(&source, &mut preview, &pixels, WIDTH * 4, WIDTH, HEIGHT);
        tick += 1;
        pace(&mut next);
    }
}

async fn wait_for_gathering(peer: &PeerConnection) {
    let started = Instant::now();
    while peer.ice_gathering_state() != IceGatheringState::Complete
        && started.elapsed() < GATHERING_TIMEOUT
    {
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
}

/// The library exposes no pending local description, so a local offer's gathered candidates
/// are added to its SDP by hand. Cloudflare's API doesn't accept trickled candidates.
fn with_candidates(sdp: &str, candidates: &[(i32, String)]) -> String {
    let mut out = String::new();
    let mut section = -1i32;
    let flush = |out: &mut String, section: i32| {
        for (index, candidate) in candidates {
            if *index == section {
                out.push_str("a=");
                out.push_str(candidate);
                out.push_str("\r\n");
            }
        }
    };
    for line in sdp.split_inclusive('\n') {
        if line.starts_with("m=") {
            flush(&mut out, section);
            section += 1;
        }
        out.push_str(line);
    }
    flush(&mut out, section);
    out
}

fn watch_state(peer: &PeerConnection, event: &'static str, tx: &mpsc::SyncSender<Value>) {
    let tx = tx.clone();
    peer.on_connection_state_change(Some(Box::new(move |state| {
        let _ = tx.try_send(json!({"event":event,"state":format!("{state:?}").to_lowercase()}));
    })));
}

pub struct Publisher {
    peer: PeerConnection,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Publisher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.peer.close();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Publisher {
    pub async fn start(
        factory: &PeerConnectionFactory,
        config: RtcConfiguration,
        kind: &str,
        id: u64,
        test_pattern: bool,
        preview_to: Option<(u16, String)>,
        tx: mpsc::SyncSender<Value>,
    ) -> Result<Self, String> {
        // Not flagged as a screencast. That mode favours sharp stills and lets the frame rate
        // fall very low, which is what made shares feel laggy. Main Nitro's "motion" hint
        // asks for the same thing.
        let source = NativeVideoSource::new(
            VideoResolution { width: MAX_CAPTURE_WIDTH, height: 1080 },
            false,
        );
        let track: RtcVideoTrack = factory.create_video_track("nitro-screen", source.clone());
        let peer = factory
            .create_peer_connection(config)
            .map_err(|_| "Screen connection could not initialize")?;
        let candidates = Arc::new(Mutex::new(Vec::new()));
        let found = candidates.clone();
        peer.on_ice_candidate(Some(Box::new(move |candidate: IceCandidate| {
            found.lock().unwrap().push((candidate.sdp_mline_index(), candidate.candidate()));
        })));
        watch_state(&peer, "screen-state", &tx);
        let transceiver = peer
            .add_transceiver(
                MediaStreamTrack::Video(track),
                RtpTransceiverInit {
                    direction: RtpTransceiverDirection::SendOnly,
                    stream_ids: vec!["nitro-screen".into()],
                    send_encodings: vec![RtpEncodingParameters {
                        max_bitrate: Some(MAX_BITRATE),
                        max_framerate: Some(MAX_FRAMERATE),
                        ..Default::default()
                    }],
                },
            )
            .map_err(|_| "Screen track could not attach")?;
        let sender = transceiver.sender();
        let mut parameters = sender.parameters();
        // Measured between two engines: the default "balanced" held about 10 fps at a quarter
        // of the size, the cause of the old lag, and favouring frame rate started at 320 wide
        // and took ten seconds to sharpen. This holds full size and full rate from the start.
        parameters.set_degradation_preference(DegradationPreference::MaintainFramerateAndResolution);
        let _ = sender.set_parameters(parameters);
        let offer = peer
            .create_offer(OfferOptions::default())
            .await
            .map_err(|_| "Screen offer failed")?;
        peer.set_local_description(offer.clone())
            .await
            .map_err(|_| "Screen offer failed")?;
        wait_for_gathering(&peer).await;
        let mid = transceiver.mid().ok_or("Screen track has no media id")?;
        let sdp = with_candidates(&offer.to_string(), &candidates.lock().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let kind = kind.to_string();
        let capture_tx = tx.clone();
        let thread = std::thread::spawn(move || {
            let preview = preview_to.and_then(|(port, token)| {
                FrameOutput::connect(port, &token, MAX_PREVIEW_WIDTH, PREVIEW_INTERVAL)
            });
            if test_pattern {
                run_test_pattern(source, preview, end);
            } else {
                run_capture(source, preview, kind, id, end, capture_tx);
            }
        });
        let _ = tx.send(json!({"event":"screen-offer","sdp":sdp,"mid":mid}));
        Ok(Self { peer, stop, thread: Some(thread) })
    }

    pub async fn answer(&self, sdp: &str) -> Result<(), String> {
        let answer = SessionDescription::parse(sdp, SdpType::Answer)
            .map_err(|_| "Invalid screen answer")?;
        self.peer
            .set_remote_description(answer)
            .await
            .map_err(|_| "Screen answer failed".to_string())
    }
}

pub struct Viewer {
    peer: PeerConnection,
    stop: Arc<AtomicBool>,
}

impl Drop for Viewer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.peer.close();
    }
}

impl Viewer {
    /// Answers Cloudflare's offer. Video frames go to the app over a private local socket.
    pub async fn start(
        factory: &PeerConnectionFactory,
        config: RtcConfiguration,
        offer: &str,
        port: u16,
        token: String,
        tx: mpsc::SyncSender<Value>,
    ) -> Result<Self, String> {
        let peer = factory
            .create_peer_connection(config)
            .map_err(|_| "Screen viewer could not initialize")?;
        watch_state(&peer, "screen-view-state", &tx);
        let stop = Arc::new(AtomicBool::new(false));
        let end = stop.clone();
        let runtime = tokio::runtime::Handle::current();
        let frames_tx = tx.clone();
        let started = Arc::new(AtomicBool::new(false));
        peer.on_track(Some(Box::new(move |event| {
            let MediaStreamTrack::Video(track) = event.track else { return };
            if started.swap(true, Ordering::AcqRel) {
                return;
            }
            let (runtime, end, frames_tx, token) =
                (runtime.clone(), end.clone(), frames_tx.clone(), token.clone());
            std::thread::spawn(move || deliver(track, port, token, runtime, end, frames_tx));
        })));
        let remote = SessionDescription::parse(offer, SdpType::Offer)
            .map_err(|_| "Invalid screen offer")?;
        peer.set_remote_description(remote)
            .await
            .map_err(|_| "Screen offer was refused")?;
        let answer = peer
            .create_answer(AnswerOptions::default())
            .await
            .map_err(|_| "Screen answer failed")?;
        peer.set_local_description(answer)
            .await
            .map_err(|_| "Screen answer failed")?;
        wait_for_gathering(&peer).await;
        let sdp = peer
            .current_local_description()
            .ok_or("Screen answer is missing")?
            .to_string();
        let _ = tx.send(json!({"event":"screen-view-answer","sdp":sdp}));
        Ok(Self { peer, stop })
    }
}

fn deliver(
    track: RtcVideoTrack,
    port: u16,
    token: String,
    runtime: tokio::runtime::Handle,
    stop: Arc<AtomicBool>,
    tx: mpsc::SyncSender<Value>,
) {
    let Some(mut output) = FrameOutput::connect(port, &token, MAX_VIEW_WIDTH, VIEW_INTERVAL) else {
        let _ = tx.try_send(json!({"event":"screen-view-ended","reason":"display-unavailable"}));
        return;
    };
    let mut stream = NativeVideoStream::new(track);
    let mut first = true;
    while !stop.load(Ordering::Acquire) {
        let next = runtime.block_on(async {
            tokio::time::timeout(Duration::from_millis(200), stream.next()).await
        });
        let frame = match next {
            Ok(Some(frame)) => frame,
            Ok(None) => return,
            Err(_) => continue,
        };
        if first {
            first = false;
            let _ = tx.try_send(json!({"event":"screen-first-frame"}));
        }
        if !output.send(&mut frame.buffer.to_i420()) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_join_their_own_media_section() {
        let sdp = "v=0\r\nm=video 9 UDP\r\na=mid:0\r\nm=audio 9 UDP\r\na=mid:1\r\n";
        let out = with_candidates(sdp, &[(0, "candidate:a".into()), (1, "candidate:b".into())]);
        assert_eq!(
            out,
            "v=0\r\nm=video 9 UDP\r\na=mid:0\r\na=candidate:a\r\nm=audio 9 UDP\r\na=mid:1\r\na=candidate:b\r\n"
        );
    }

    #[test]
    fn large_captures_shrink_to_even_sizes_at_the_same_shape() {
        assert_eq!(even_size(3840, 2160, 1920), (1920, 1080));
        assert_eq!(even_size(1281, 721, 1920), (1280, 720));
    }
}
