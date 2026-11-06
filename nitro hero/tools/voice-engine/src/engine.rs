//! Private native audio worker. Owns a best-effort lease cleanup on parent EOF.
use libwebrtc::{
    audio_track::RtcAudioTrack,
    ice_candidate::IceCandidate,
    peer_connection::{AnswerOptions, OfferOptions, PeerConnection, SignalingState},
    peer_connection_factory::{
        IceServer, PeerConnectionFactory, RtcConfiguration, native::PeerConnectionFactoryExt,
    },
    session_description::{SdpType, SessionDescription},
};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    sync::mpsc,
    time::Duration,
};
mod routes;
mod local_meter;
mod screen;

struct Engine {
    lease: Option<Cleanup>,
    peer_monitors: std::sync::Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    test_source: bool,
    epoch: String,
    monitors: std::sync::Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    factory: PeerConnectionFactory,
    peer: Option<PeerConnection>,
    track: Option<RtcAudioTrack>,
    acquired: bool,
    local_meter: Option<local_meter::LocalMeter>,
    screen_publisher: Option<screen::Publisher>,
    screen_viewer: Option<screen::Viewer>,
    sound_channel: std::sync::Arc<std::sync::Mutex<Option<libwebrtc::data_channel::DataChannel>>>,
    output_route: String,
    output_level: f32,
    deafened: bool,
    /// Set once a remote audio track exists, so output never starts with nothing to play.
    remote_audio: std::sync::Arc<std::sync::atomic::AtomicBool>,
    polite: bool,
    ignored_offer: bool,
    pending: Vec<Value>,
    config: RtcConfiguration,
    events: mpsc::SyncSender<Value>,
}
struct Cleanup {
    url: String,
    key: String,
    token: zeroize::Zeroizing<String>,
    session: String,
}
impl Cleanup {
    fn leave(self) {
        if let Ok(http) = reqwest::blocking::Client::builder()
            .https_only(true)
            .timeout(Duration::from_secs(6))
            .connect_timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .build()
        {
            let _ = http
                .post(format!("{}/rest/v1/rpc/leave_voice_room", self.url))
                .header("apikey", self.key)
                .bearer_auth(self.token.as_str())
                .json(&json!({"p_session_id":self.session}))
                .send();
        }
    }
}
impl Engine {
    fn update_playout(&self) -> Result<(), String> {
        if !self.acquired { return Ok(()); } // Synthetic transport tests have no hardware sink.
        let ready = self.acquired && !self.deafened
            && self.remote_audio.load(std::sync::atomic::Ordering::Acquire);
        self.factory.set_adm_playout_enabled(ready);
        // Disabled platform output uses libwebrtc's synthetic pump. Do not stop
        // that pump: decoding must continue so first-decoded can open this gate.
        if ready {
            if !self.factory.playout_is_initialized() && !self.factory.init_playout() {
                return Err("Audio output initialization failed".into());
            }
            if !self.factory.start_playout() { return Err("Audio output failed".into()); }
            local_meter::set_output_level(&self.output_route, self.output_level);
        }
        Ok(())
    }
    /// Cloudflare's SFU is reached directly. Screen media never goes through TURN.
    fn screen_config(&self) -> RtcConfiguration {
        let mut config = RtcConfiguration::default();
        config.ice_servers = vec![IceServer {
            urls: vec!["stun:stun.cloudflare.com:3478".into()],
            username: String::new(),
            password: String::new(),
        }];
        config
    }
    /// Soundboard messages travel on the same channel main Nitro opens, and only one
    /// side creates it. Each received message goes to the app untouched.
    fn adopt_sound_channel(
        channel: libwebrtc::data_channel::DataChannel,
        slot: &std::sync::Mutex<Option<libwebrtc::data_channel::DataChannel>>,
        events: mpsc::SyncSender<Value>,
    ) {
        channel.on_message(Some(Box::new(move |message| {
            if !message.binary && let Ok(text) = std::str::from_utf8(message.data) {
                let _ = events.try_send(json!({"event":"peer-data","text":text}));
            }
        })));
        *slot.lock().unwrap() = Some(channel);
    }
    fn clear_peer(&mut self) {
        *self.sound_channel.lock().unwrap() = None;
        if let Some(peer) = self.peer.take() {
            peer.close();
        }
        for monitor in self.peer_monitors.lock().unwrap().drain(..) {
            monitor.abort();
        }
        self.pending.clear();
        self.ignored_offer = false;
        // The peer's gone, so nothing is left to play until the next remote track.
        self.remote_audio.store(false, std::sync::atomic::Ordering::Release);
        self.factory.set_adm_playout_enabled(false);
        self.factory.stop_playout();
    }
    fn stop(&mut self) {
        self.screen_publisher = None;
        self.screen_viewer = None;
        self.local_meter = None;
        for monitor in self.monitors.lock().unwrap().drain(..) {
            monitor.abort();
        }
        self.clear_peer();
        self.track = None;
        self.pending.clear();
        if self.acquired {
            self.factory.stop_recording();
            self.factory.stop_playout();
            self.factory.set_adm_recording_enabled(false);
            self.factory.set_adm_playout_enabled(false);
            self.factory.release_platform_adm();
            self.acquired = false;
        }
    }
    async fn description(&self, offer: bool, restart: bool) -> Result<(), String> {
        let peer = self.peer.as_ref().ok_or("Peer is not ready")?;
        let desc = if offer {
            peer.create_offer(OfferOptions {
                ice_restart: restart,
                offer_to_receive_audio: true,
                offer_to_receive_video: false,
            })
            .await
        } else {
            peer.create_answer(AnswerOptions::default()).await
        }
        .map_err(|_| "Could not create voice description")?;
        let payload = json!({"event":"description","epoch":self.epoch,"description":{"type":desc.sdp_type().to_string(),"sdp":desc.to_string()}});
        peer.set_local_description(desc)
            .await
            .map_err(|_| "Could not apply local voice description")?;
        self.events
            .send(payload)
            .map_err(|_| "Voice control closed".into())
    }
    async fn command(&mut self, v: Value) -> Result<(), String> {
        match v["command"].as_str().ok_or("Missing command")? {
            "lease" => {
                self.lease = Some(Cleanup {
                    url: v["url"].as_str().ok_or("Missing lease endpoint")?.into(),
                    key: v["key"].as_str().ok_or("Missing lease key")?.into(),
                    token: zeroize::Zeroizing::new(
                        v["token"].as_str().ok_or("Missing lease access")?.into(),
                    ),
                    session: v["session"]
                        .as_str()
                        .ok_or("Missing lease identity")?
                        .into(),
                });
            }
            "token" => {
                if let Some(lease) = &mut self.lease {
                    lease.token = zeroize::Zeroizing::new(
                        v["token"]
                            .as_str()
                            .ok_or("Missing updated lease access")?
                            .into(),
                    );
                }
            }
            "prepare" => {
                self.stop();
                if self.test_source {
                    use libwebrtc::audio_source::{AudioSourceOptions, native::NativeAudioSource};
                    let source =
                        NativeAudioSource::new(AudioSourceOptions::default(), 48000, 1, 20);
                    self.track = Some(
                        self.factory
                            .create_audio_track("diagnostic-tone", source.clone()),
                    );
                    self.monitors.lock().unwrap().push(tokio::spawn(async move {
                        let mut phase: f64 = 0.;
                        loop {
                            let mut frame = libwebrtc::audio_frame::AudioFrame::new(48000, 1, 480);
                            for sample in frame.data.to_mut() {
                                *sample = (phase.sin() * 4000.) as i16;
                                phase = (phase + std::f64::consts::TAU * 440. / 48000.)
                                    % std::f64::consts::TAU;
                            }
                            if source.capture_frame(&frame).await.is_err() {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    }));
                    self.events
                        .send(json!({"event":"prepared"}))
                        .map_err(|_| "Voice control closed")?;
                    return Ok(());
                }
                self.factory.set_adm_playout_enabled(false);
                if !self.factory.acquire_platform_adm() {
                    return Err("Audio device manager unavailable".into());
                }
                self.acquired = true;
                let input = v["input"].as_str().unwrap_or("");
                let output = v["output"].as_str().unwrap_or("");
                self.output_route = output.to_string();
                routes::input(
                    &self.factory,
                    if input.is_empty() {
                        routes::Route::SystemDefault
                    } else {
                        routes::Route::DeviceGuid(input)
                    },
                )?;
                routes::output(
                    &self.factory,
                    if output.is_empty() {
                        routes::Route::SystemDefault
                    } else {
                        routes::Route::DeviceGuid(output)
                    },
                )?;
                // The ADM can start automatically; removing explicit StartPlayout alone
                // did not gate it. Keep output disabled until decoded remote PCM exists.
                self.factory.set_adm_playout_enabled(false);
                self.factory.set_adm_recording_enabled(true);
                self.track = Some(self.factory.create_device_audio_track("nitro-microphone"));
                if !self.factory.recording_is_initialized() && !self.factory.init_recording() {
                    return Err("Microphone initialization failed".into());
                }
                if !self.factory.start_recording() {
                    return Err("Microphone capture failed".into());
                }
                self.local_meter = Some(local_meter::LocalMeter::start(
                    if input.is_empty() { self.factory.recording_device_guid(0) } else { input.to_string() },
                    self.events.clone(),
                ));
                self.events
                    .send(json!({"event":"prepared"}))
                    .map_err(|_| "Voice control closed")?;
            }
            "devices" => {
                let temporary = !self.acquired;
                if temporary && !self.factory.acquire_platform_adm() {
                    return Err("Audio devices unavailable".into());
                }
                let inputs=(0..self.factory.recording_devices().max(0) as u16).map(|i|json!({"id":self.factory.recording_device_guid(i),"name":self.factory.recording_device_name(i)})).collect::<Vec<_>>();
                let outputs=(0..self.factory.playout_devices().max(0) as u16).map(|i|json!({"id":self.factory.playout_device_guid(i),"name":self.factory.playout_device_name(i)})).collect::<Vec<_>>();
                if temporary {
                    self.factory.release_platform_adm();
                }
                self.events
                    .send(json!({"event":"devices","inputs":inputs,"outputs":outputs}))
                    .map_err(|_| "Voice control closed")?;
            }
            "routes" => {
                if !self.acquired {
                    return Err("Audio capture is not ready".into());
                }
                self.local_meter = None;
                self.factory.stop_recording();
                self.factory.stop_playout();
                let input = v["input"].as_str().unwrap_or("");
                let output = v["output"].as_str().unwrap_or("");
                self.output_route = output.to_string();
                routes::input(
                    &self.factory,
                    if input.is_empty() {
                        routes::Route::SystemDefault
                    } else {
                        routes::Route::DeviceGuid(input)
                    },
                )?;
                routes::output(
                    &self.factory,
                    if output.is_empty() {
                        routes::Route::SystemDefault
                    } else {
                        routes::Route::DeviceGuid(output)
                    },
                )?;
                if !self.factory.init_recording() || !self.factory.start_recording() {
                    return Err("Microphone change failed".into());
                }
                self.local_meter = Some(local_meter::LocalMeter::start(
                    if input.is_empty() { self.factory.recording_device_guid(0) } else { input.to_string() },
                    self.events.clone(),
                ));
                // Restart for any peer, even before its first audio: while output is gated
                // this runs the silent pump, and without it the first decode never arrives.
                if self.peer.is_some()
                    && (!self.factory.init_playout() || !self.factory.start_playout())
                {
                    return Err("Output change failed".into());
                }
            }
            "peer" => {
                self.epoch = v["epoch"].as_str().ok_or("Missing peer epoch")?.into();
                if self.track.is_none() {
                    return Err("Microphone is not ready".into());
                }
                self.clear_peer();
                self.polite = v["polite"].as_bool().ok_or("Missing peer role")?;
                self.ignored_offer = false;
                self.pending.clear();
                let peer = self
                    .factory
                    .create_peer_connection(self.config.clone())
                    .map_err(|_| "Voice peer could not initialize")?;
                let tx = self.events.clone();
                let epoch = self.epoch.clone();
                peer.on_ice_candidate(Some(Box::new(move |c| {let _=tx.try_send(json!({"event":"candidate","epoch":epoch,"candidate":{"candidate":c.candidate(),"sdpMid":c.sdp_mid(),"sdpMLineIndex":c.sdp_mline_index()}}));})));
                let tx = self.events.clone();
                let epoch = self.epoch.clone();
                peer.on_connection_state_change(Some(Box::new(move |s| {
                    let _ = tx
                        .try_send(json!({"event":"state","epoch":epoch,"state":format!("{s:?}").to_lowercase()}));
                })));
                let tx = self.events.clone();
                let epoch = self.epoch.clone();
                let runtime = tokio::runtime::Handle::current();
                let monitors = self.peer_monitors.clone();
                peer.on_track(Some(Box::new(move |event| {
                    if let libwebrtc::media_stream_track::MediaStreamTrack::Audio(track) =
                        event.track
                    {
                        let _guard = runtime.enter();
                        monitors.lock().unwrap().push(monitor(
                            track,
                            tx.clone(),
                            "remote",
                            epoch.clone(),
                        ));
                    }
                    let _ = tx.try_send(json!({"event":"remote-track","epoch":epoch}));
                })));
                peer.add_track(
                    self.track.as_ref().ok_or("No microphone")?.clone().into(),
                    &["nitro-voice"],
                )
                .map_err(|_| "Microphone could not attach")?;
                let (slot, tx) = (self.sound_channel.clone(), self.events.clone());
                peer.on_data_channel(Some(Box::new(move |channel| {
                    if channel.label() == "dislight-soundboard" {
                        Self::adopt_sound_channel(channel, &slot, tx.clone());
                    }
                })));
                if !self.polite {
                    let channel = peer
                        .create_data_channel("dislight-soundboard", Default::default())
                        .map_err(|_| "Soundboard channel could not open")?;
                    Self::adopt_sound_channel(channel, &self.sound_channel, self.events.clone());
                }
                self.peer = Some(peer);
                // Both original clients offer. Match perfect-negotiation role on collision.
                self.description(true, false).await?;
            }
            "description" => {
                let peer = self.peer.as_ref().ok_or("Peer is not ready")?;
                let kind = v["description"]["type"]
                    .as_str()
                    .ok_or("Missing description type")?;
                let typ = match kind {
                    "offer" => SdpType::Offer,
                    "answer" => SdpType::Answer,
                    _ => return Err("Invalid description type".into()),
                };
                let collision =
                    typ == SdpType::Offer && peer.signaling_state() != SignalingState::Stable;
                self.ignored_offer = collision && !self.polite;
                if self.ignored_offer {
                    return Ok(());
                }
                if collision {
                    peer.set_local_description(
                        SessionDescription::parse("", SdpType::Rollback)
                            .map_err(|_| "Rollback parse failed")?,
                    )
                    .await
                    .map_err(|_| "Voice rollback failed")?;
                }
                let desc = SessionDescription::parse(
                    v["description"]["sdp"].as_str().ok_or("Missing SDP")?,
                    typ,
                )
                .map_err(|_| "Invalid voice description")?;
                peer.set_remote_description(desc)
                    .await
                    .map_err(|_| "Remote voice description failed")?;
                for candidate in std::mem::take(&mut self.pending) {
                    self.candidate(candidate).await?;
                }
                if typ == SdpType::Offer {
                    self.description(false, false).await?;
                }
            }
            "candidate" => {
                if self.ignored_offer {
                    return Ok(());
                }
                let candidate = v["candidate"].clone();
                if self
                    .peer
                    .as_ref()
                    .and_then(|p| p.current_remote_description())
                    .is_none()
                {
                    if self.pending.len() >= 256 {
                        return Err("Voice candidate queue exceeded".into());
                    }
                    self.pending.push(candidate);
                } else {
                    self.candidate(candidate).await?;
                }
            }
            "remote-ready" => {
                if v["epoch"].as_str() == Some(self.epoch.as_str()) && self.peer.is_some() {
                    self.remote_audio.store(true, std::sync::atomic::Ordering::Release);
                    self.update_playout()?;
                }
            }
            "controls" => {
                let muted = v["muted"].as_bool().ok_or("Missing mute state")?;
                let deafened = v["deafened"].as_bool().ok_or("Missing deafen state")?;
                if let Some(track) = &self.track {
                    track.set_enabled(!muted && !deafened);
                }
                self.deafened = deafened;
                self.update_playout()?;
            }
            "ice" => {
                let mut servers = Vec::new();
                for s in v["servers"].as_array().ok_or("Missing ICE servers")? {
                    let urls = if let Some(url) = s["urls"].as_str() {
                        vec![url.into()]
                    } else {
                        s["urls"]
                            .as_array()
                            .ok_or("Invalid ICE URLs")?
                            .iter()
                            .filter_map(|u| u.as_str().map(String::from))
                            .collect()
                    };
                    servers.push(IceServer {
                        urls,
                        username: s["username"].as_str().unwrap_or("").into(),
                        password: s["credential"].as_str().unwrap_or("").into(),
                    });
                }
                self.config.ice_servers = servers;
                if let Some(peer) = &self.peer {
                    peer.set_configuration(self.config.clone())
                        .map_err(|_| "ICE configuration failed")?;
                }
            }
            "restart" => {
                self.description(true, true).await?;
            }
            "stop" => {
                self.stop();
            }
            "clear-peer" => self.clear_peer(),
            "peer-data" => {
                let text = v["text"].as_str().ok_or("Missing peer data")?;
                let sent = self.sound_channel.lock().unwrap().as_ref().is_some_and(|channel| {
                    channel.state() == libwebrtc::data_channel::DataChannelState::Open
                        && channel.send(text.as_bytes(), false).is_ok()
                });
                // Main Nitro falls back to the room channel when this isn't open.
                if !sent {
                    let _ = self.events.try_send(json!({"event":"peer-data-failed","text":text}));
                }
            }
            "output-level" => {
                self.output_level = v["level"].as_f64().unwrap_or(1.).clamp(0., 1.) as f32;
                if self.acquired {
                    local_meter::set_output_level(&self.output_route, self.output_level);
                }
            }
            "screen-sources" => {
                self.events
                    .send(json!({"event":"screen-sources","sources":screen::sources()}))
                    .map_err(|_| "Voice control closed")?;
            }
            "screen-publish" => {
                self.screen_publisher = None;
                let id = v["id"].as_str().and_then(|id| id.parse().ok()).ok_or("Missing screen source")?;
                self.screen_publisher = Some(
                    screen::Publisher::start(
                        &self.factory,
                        self.screen_config(),
                        v["kind"].as_str().unwrap_or("screen"),
                        id,
                        self.test_source,
                        v["previewPort"].as_u64().zip(v["previewToken"].as_str()).map(|(port, token)| {
                            (port as u16, token.to_string())
                        }),
                        self.events.clone(),
                    )
                    .await?,
                );
            }
            "screen-answer" => {
                self.screen_publisher
                    .as_ref()
                    .ok_or("Screen share is not starting")?
                    .answer(v["sdp"].as_str().ok_or("Missing screen answer")?)
                    .await?;
            }
            "screen-stop" => self.screen_publisher = None,
            "screen-view" => {
                self.screen_viewer = None;
                self.screen_viewer = Some(
                    screen::Viewer::start(
                        &self.factory,
                        self.screen_config(),
                        v["sdp"].as_str().ok_or("Missing screen offer")?,
                        v["port"].as_u64().ok_or("Missing frame port")? as u16,
                        v["token"].as_str().ok_or("Missing frame token")?.to_string(),
                        self.events.clone(),
                    )
                    .await?,
                );
            }
            "screen-view-stop" => self.screen_viewer = None,
            _ => return Err("Unknown voice command".into()),
        }
        Ok(())
    }
    async fn candidate(&self, v: Value) -> Result<(), String> {
        let c = IceCandidate::parse(
            v["sdpMid"].as_str().unwrap_or(""),
            v["sdpMLineIndex"].as_i64().ok_or("Missing ICE index")? as i32,
            v["candidate"].as_str().ok_or("Missing ICE candidate")?,
        )
        .map_err(|_| "Invalid ICE candidate")?;
        self.peer
            .as_ref()
            .ok_or("Peer is not ready")?
            .add_ice_candidate(c)
            .await
            .map_err(|_| "Could not apply ICE candidate".into())
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}
fn monitor(
    track: RtcAudioTrack,
    tx: mpsc::SyncSender<Value>,
    side: &'static str,
    epoch: String,
) -> tokio::task::JoinHandle<()> {
    use libwebrtc::audio_stream::native::{NativeAudioStream, NativeAudioStreamOptions};
    use tokio_stream::StreamExt;
    tokio::spawn(async move {
        let mut stream = NativeAudioStream::with_options(
            track,
            48000,
            1,
            NativeAudioStreamOptions {
                queue_size_frames: Some(3),
            },
        );
        let mut last = std::time::Instant::now() - Duration::from_secs(1);
        let mut first = true;
        while let Some(frame) = stream.next().await {
            if first {
                let _ = tx.try_send(json!({"event":"first-decoded","side":side,"epoch":epoch}));
                first = false;
            }
            if last.elapsed() >= Duration::from_millis(100) {
                let energy = frame
                    .data
                    .iter()
                    .map(|&x| {
                        let s = x as f64 / 32768.;
                        s * s
                    })
                    .sum::<f64>()
                    / frame.data.len().max(1) as f64;
                let level = (energy.sqrt() * 4.).clamp(0., 1.);
                let _ =
                    tx.try_send(json!({"event":"level","side":side,"epoch":epoch,"level":level}));
                last = std::time::Instant::now();
            }
        }
    })
}
fn main() {
    if !cfg!(debug_assertions) && std::env::args().any(|a| a == "--self-test-source") {
        std::process::exit(2);
    }
    let (tx, rx) = mpsc::sync_channel::<Value>(256);
    std::thread::spawn(move || {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        for value in rx {
            if writeln!(out, "{value}").and_then(|_| out.flush()).is_err() {
                break;
            }
        }
    });
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .expect("Voice runtime");
    let mut config = RtcConfiguration::default();
    config.ice_servers = vec![IceServer {
        urls: vec!["stun:stun.cloudflare.com:3478".into()],
        username: String::new(),
        password: String::new(),
    }];
    let mut engine = Engine {
        lease: None,
        peer_monitors: Default::default(),
        test_source: cfg!(debug_assertions) && std::env::args().any(|a| a == "--self-test-source"),
        epoch: String::new(),
        monitors: Default::default(),
        factory: PeerConnectionFactory::default(),
        peer: None,
        track: None,
        acquired: false,
        local_meter: None,
        screen_publisher: None,
        screen_viewer: None,
        sound_channel: Default::default(),
        output_route: String::new(),
        output_level: 1.,
        deafened: false,
        remote_audio: Default::default(),
        polite: false,
        ignored_offer: false,
        pending: Vec::new(),
        config,
        events: tx.clone(),
    };
    // Parent owns lifetime; EOF releases microphone, output and peer.
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.len() > 262144 {
            break;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            break;
        };
        let screen_command = v["command"].as_str().is_some_and(|c| c.starts_with("screen"));
        let result = rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(10), engine.command(v)).await
        });
        match result {
            Ok(Ok(())) => {}
            // Its own connections, so a failed share must not end the voice call.
            Ok(Err(error)) if screen_command => {
                let _ = tx.send(json!({"event":"screen-error","message":error}));
            }
            Err(_) if screen_command => {
                let _ = tx.send(json!({"event":"screen-error","message":"Screen sharing timed out."}));
            }
            Ok(Err(error)) => {
                let _ = tx.send(json!({"event":"error","message":error}));
            }
            Err(_) => {
                let _ = tx.send(json!({"event":"error","message":"Voice operation timed out"}));
                break;
            }
        }
    }
    // Stop all capture/output before HTTP. This child survives GUI-process exit.
    engine.stop();
    if let Some(lease) = engine.lease.take() {
        lease.leave();
    }
}
