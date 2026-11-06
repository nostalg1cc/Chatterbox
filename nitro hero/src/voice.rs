//! Native call supervisor. Owns one helper and one generation-scoped room lease.
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command as ProcessCommand, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tungstenite::{Message, stream::MaybeTlsStream};

#[derive(Clone, Default)]
pub struct State {
    pub session: String,
    pub conversation: String,
    pub status: String,
    pub muted: bool,
    pub deafened: bool,
    pub participants: Vec<String>,
    pub error: Option<String>,
    pub levels: HashMap<String, f32>,
    pub remote_muted: bool,
    pub receiving: bool,
    pub local_meter_status: String,
    pub screen: crate::screen_share::ScreenStatus,
    /// Soundboard messages from the partner, newest last, with a running count.
    pub sound_events: Vec<(u64, Value)>,
}
pub enum Control {
    Routes {
        input: Option<String>,
        output: Option<String>,
    },
    Stop,
    Controls {
        muted: bool,
        deafened: bool,
    },
    Token(String),
    /// A soundboard message for the partner.
    PeerData(Value),
    OutputLevel(f32),
    ScreenSources,
    ScreenShare { id: String, kind: String, title: String, preview_port: u16, preview_token: String },
    ScreenStop,
    ScreenWatch { port: u16, token: String },
    ScreenUnwatch,
}
pub struct Handle {
    tx: mpsc::Sender<Control>,
}
impl Handle {
    pub fn send(&self, c: Control) {
        let _ = self.tx.send(c);
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.send(Control::Stop);
    }
}
#[derive(Clone, Default)]
pub struct Controller(Arc<std::sync::Mutex<ControlSlot>>);
#[derive(Default)]
struct ControlSlot {
    session: Option<String>,
    handle: Option<Handle>,
    muted: bool,
    deafened: bool,
}
impl Controller {
    pub fn want(&self, session: String) {
        let mut slot = self.0.lock().unwrap();
        slot.handle = None;
        slot.session = Some(session);
        slot.muted = false;
        slot.deafened = false;
    }
    pub fn wanted(&self, session: &str) -> bool {
        self.0.lock().unwrap().session.as_deref() == Some(session)
    }
    pub fn attach(&self, session: &str, handle: Handle) {
        let mut slot = self.0.lock().unwrap();
        if slot.session.as_deref() == Some(session) {
            handle.send(Control::Controls {
                muted: slot.muted,
                deafened: slot.deafened,
            });
            slot.handle = Some(handle);
        }
    }
    pub fn control(&self, control: Control) {
        let mut slot = self.0.lock().unwrap();
        if matches!(control, Control::Stop) {
            slot.session = None;
            slot.handle = None;
        } else {
            if let Control::Controls { muted, deafened } = &control {
                slot.muted = *muted;
                slot.deafened = *deafened;
            }
            if let Some(handle) = &slot.handle {
                handle.send(control);
            }
        }
    }
}
pub struct Start {
    pub input: Option<String>,
    pub output: Option<String>,
    pub url: String,
    pub key: String,
    pub token: String,
    pub user: String,
    pub partner: String,
    pub conversation: String,
    pub session: String,
}

struct Helper {
    child: Child,
    input: Option<ChildStdin>,
    events: mpsc::Receiver<Value>,
}
#[derive(Clone, serde::Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
}
#[derive(Default, Clone, serde::Deserialize)]
pub struct Devices {
    pub inputs: Vec<Device>,
    pub outputs: Vec<Device>,
}
pub fn devices() -> Result<Devices, String> {
    let mut helper = Helper::start()?;
    helper.send(json!({"command":"devices"}))?;
    let value = helper
        .events
        .recv_timeout(Duration::from_secs(8))
        .map_err(|_| "Audio devices did not respond")?;
    if value["event"] != "devices" {
        return Err(value["message"]
            .as_str()
            .unwrap_or("Audio device enumeration failed")
            .into());
    }
    serde_json::from_value(value).map_err(|_| "Audio device list is invalid".into())
}
impl Helper {
    fn start() -> Result<Self, String> {
        let path = std::env::current_exe()
            .map_err(|_| "Cannot locate native audio helper")?
            .with_file_name("nitro-voice-engine.exe");
        if !path.is_file() {
            return Err(
                "The voice engine is missing. Run scripts/build-voice.ps1."
                    .into(),
            );
        }
        let mut cmd = ProcessCommand::new(path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd
            .spawn()
            .map_err(|_| "Native audio helper could not start")?;
        let input = child.stdin.take();
        let output = child
            .stdout
            .take()
            .ok_or("Native audio helper has no output")?;
        let (tx, events) = mpsc::sync_channel(256);
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                if line.len() > 262144 {
                    break;
                }
                let Ok(value) = serde_json::from_str(&line) else {
                    break;
                };
                if tx.send(value).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input,
            events,
        })
    }
    fn send(&mut self, v: Value) -> Result<(), String> {
        writeln!(self.input.as_mut().ok_or("Audio helper closed")?, "{v}")
            .and_then(|_| self.input.as_mut().unwrap().flush())
            .map_err(|_| "Native audio helper stopped".into())
    }
}
impl Drop for Helper {
    fn drop(&mut self) {
        self.input.take();
        for _ in 0..20 {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Clone)]
struct Api {
    url: String,
    key: String,
    token: Arc<std::sync::Mutex<String>>,
    http: reqwest::blocking::Client,
}
impl Api {
    fn request(
        &self,
        path: &str,
        body: Option<Value>,
        query: &[(&str, String)],
    ) -> Result<Value, String> {
        let token = self
            .token
            .lock()
            .map_err(|_| "Voice authentication unavailable")?
            .clone();
        let mut r = if body.is_some() {
            self.http.post(format!("{}/{path}", self.url))
        } else {
            self.http.get(format!("{}/{path}", self.url))
        };
        r = r
            .header("apikey", &self.key)
            .bearer_auth(token)
            .query(query);
        if let Some(body) = body {
            r = r.json(&body);
        }
        let response = r.send().map_err(|_| "Voice service could not be reached")?;
        if !response.status().is_success() {
            return Err("Voice service rejected the request. Retry or sign in again.".into());
        }
        response
            .json()
            .map_err(|_| "Voice service returned an invalid response".into())
    }
    /// The screen-share function. Its refusals say why, so they're passed on as written.
    fn screen_share(&self, body: Value) -> Result<Value, String> {
        let token = self
            .token
            .lock()
            .map_err(|_| "Voice authentication unavailable")?
            .clone();
        let response = self
            .http
            .post(format!("{}/functions/v1/screen-share", self.url))
            .header("apikey", &self.key)
            .bearer_auth(token)
            .json(&body)
            .send()
            .map_err(|_| "Screen sharing could not be reached.")?;
        let status = response.status();
        if status.as_u16() == 404 {
            return Err("Screen sharing isn't enabled on the server yet.".into());
        }
        let reply: Value = response
            .json()
            .map_err(|_| "Screen sharing returned an invalid response.")?;
        if !status.is_success() {
            return Err(reply["error"].as_str().unwrap_or("Screen sharing failed.").to_string());
        }
        Ok(reply)
    }
    fn rpc(&self, name: &str, body: Value) -> Result<Value, String> {
        self.request(&format!("rest/v1/rpc/{name}"), Some(body), &[])
    }
}
struct Lease {
    api: Api,
    session: String,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let api = self.api.clone();
        let session = self.session.clone();
        std::thread::spawn(move || {
            let _ = api.rpc("leave_voice_room", json!({"p_session_id":session}));
        });
    }
}
enum Poll {
    Members(Value),
    Turn(Value),
    LeaseLost,
}
struct Poller(Arc<AtomicBool>);
impl Drop for Poller {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
fn poll(
    api: Api,
    session: String,
    conversation: String,
    sharing: Arc<AtomicBool>,
) -> (Poller, mpsc::Receiver<Poll>) {
    let stop = Arc::new(AtomicBool::new(false));
    let end = stop.clone();
    let (tx, rx) = mpsc::sync_channel(8);
    std::thread::spawn(move || {
        let mut lease = Instant::now() - Duration::from_secs(45);
        let mut turn = Instant::now() - Duration::from_secs(3600);
        while !end.load(Ordering::Acquire) {
            if lease.elapsed() >= Duration::from_secs(30) {
                if let Ok(v) = api.rpc(
                    "heartbeat_voice_room",
                    json!({"p_session_id":session,"p_sharing_screen":sharing.load(Ordering::Acquire)}),
                ) && v["status"] == "not_found"
                {
                    let _ = tx.try_send(Poll::LeaseLost);
                    break;
                }
                lease = Instant::now();
            }
            if let Ok(v) = api.request(
                "rest/v1/voice_participants",
                None,
                &[
                    ("select", "user_id,session_id".into()),
                    ("conversation_id", format!("eq.{conversation}")),
                ],
            ) {
                let _ = tx.try_send(Poll::Members(v));
            }
            if turn.elapsed() >= Duration::from_secs(300) {
                if let Ok(v) = api.request(
                    "functions/v1/realtime-credentials",
                    Some(json!({"conversationId":conversation,"voiceSessionId":session})),
                    &[],
                ) {
                    let _ = tx.try_send(Poll::Turn(v));
                }
                turn = Instant::now();
            }
            for _ in 0..20 {
                if end.load(Ordering::Acquire) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
    (Poller(stop), rx)
}
type Socket = tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>;
fn connect(api: &Api, topic: &str, session: &str) -> Result<Socket, String> {
    let mut endpoint = reqwest::Url::parse(&api.url).map_err(|_| "Invalid voice endpoint")?;
    endpoint
        .set_scheme("wss")
        .map_err(|_| "Invalid voice endpoint")?;
    endpoint.set_path("/realtime/v1/websocket");
    endpoint
        .query_pairs_mut()
        .append_pair("apikey", &api.key)
        .append_pair("vsn", "1.0.0");
    let (mut socket, _) =
        tungstenite::connect(endpoint.as_str()).map_err(|_| "Voice signaling unavailable")?;
    let stream = match socket.get_mut() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::NativeTls(s) => s.get_mut(),
        _ => return Err("Unsupported voice transport".into()),
    };
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(|_| "Voice timeout setup failed")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| "Voice timeout setup failed")?;
    let token = api
        .token
        .lock()
        .map_err(|_| "Voice authentication unavailable")?
        .clone();
    socket.send(Message::Text(json!({"topic":topic,"event":"phx_join","ref":"1","join_ref":"1","payload":{"access_token":token,"config":{"private":true,"broadcast":{"ack":true,"self":false},"presence":{"key":session,"enabled":true},"postgres_changes":[]}}}).to_string().into())).map_err(|_|"Voice signaling join failed")?;
    Ok(socket)
}
fn send(
    socket: &mut Socket,
    topic: &str,
    event: &str,
    payload: Value,
    sequence: &mut u64,
) -> Result<String, String> {
    *sequence += 1;
    let id = sequence.to_string();
    socket
        .send(Message::Text(
            json!({"topic":topic,"event":event,"payload":payload,"ref":id,"join_ref":"1"})
                .to_string()
                .into(),
        ))
        .map_err(|_| "Voice signaling send failed")?;
    Ok(id)
}
fn note_sound(state: &mut State, count: u64, message: Value) -> u64 {
    let count = count + 1;
    state.sound_events.push((count, message));
    if state.sound_events.len() > 8 {
        state.sound_events.remove(0);
    }
    count
}
/// Calls the screen-share function off this loop, which also reads the signaling socket.
fn broker(
    api: &Api,
    scope: &Value,
    replies: &mpsc::Sender<(crate::screen_share::Op, Result<Value, String>)>,
    op: crate::screen_share::Op,
    mut body: Value,
) {
    body["conversationId"] = scope["conversationId"].clone();
    body["voiceSessionId"] = scope["voiceSessionId"].clone();
    let (api, replies) = (api.clone(), replies.clone());
    std::thread::spawn(move || {
        let _ = replies.send((op, api.screen_share(body)));
    });
}
/// `signal` is shadowed where received signals are handled.
fn signal_body(v: Value, generation: &str, session: &str, partner: Option<&str>) -> Value {
    signal(v, generation, session, partner)
}
fn signal(mut v: Value, generation: &str, session: &str, partner: Option<&str>) -> Value {
    v["version"] = json!(1);
    v["generation"] = json!(generation);
    v["fromSessionId"] = json!(session);
    v["signalId"] = json!(uuid::Uuid::new_v4().to_string());
    if let Some(partner) = partner {
        v["toSessionId"] = json!(partner);
    }
    v
}
pub fn start(config: Start, events: mpsc::Sender<State>) -> Handle {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut state = State {
            session: config.session.clone(),
            conversation: config.conversation.clone(),
            status: "joining".into(),
            ..Default::default()
        };
        let emit = |s: &State| {
            let _ = events.send(s.clone());
        };
        emit(&state);
        if let Err(error) = run(&config, &rx, &mut state, &emit) {
            state.status = "failed".into();
            state.error = Some(error);
            emit(&state);
        } else {
            state.status = "idle".into();
            emit(&state);
        }
    });
    Handle { tx }
}
fn run(
    c: &Start,
    controls: &mpsc::Receiver<Control>,
    state: &mut State,
    emit: &impl Fn(&State),
) -> Result<(), String> {
    let api = Api {
        url: c.url.clone(),
        key: c.key.clone(),
        token: Arc::new(std::sync::Mutex::new(c.token.clone())),
        http: reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .connect_timeout(Duration::from_secs(4))
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Voice HTTPS initialization failed")?,
    };
    let mut helper = Helper::start()?;
    helper.send(json!({"command":"prepare","input":c.input,"output":c.output}))?;
    let ready = Instant::now();
    loop {
        // The helper processes stdin sequentially: controls received during preparation
        // run immediately afterwards, before membership/signaling starts. Never discard
        // a route selection merely because the prepared event has not arrived yet.
        if !setup_controls(controls, state, &api, Some(&mut helper))? {
            return Ok(());
        }
        match helper.events.recv_timeout(Duration::from_millis(50)) {
            Ok(v) if v["event"] == "prepared" => break,
            Ok(v) if v["event"] == "local-meter" => {
                state.local_meter_status = v["status"].as_str().unwrap_or("unavailable").to_string();
            }
            Ok(v) if v["event"] == "error" => {
                return Err(v["message"].as_str().unwrap_or("Microphone failed").into());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Native audio engine exited".into());
            }
            _ => {}
        }
        if ready.elapsed() > Duration::from_secs(12) {
            return Err("Microphone readiness timed out".into());
        }
    }
    helper.send(json!({"command":"controls","muted":state.muted,"deafened":state.deafened}))?;
    let (membership_tx, membership_rx) = mpsc::sync_channel(1);
    let join_api = api.clone();
    let session = c.session.clone();
    let conversation = c.conversation.clone();
    std::thread::spawn(move || {
        let result = join_api.rpc(
            "join_voice_room",
            json!({"p_conversation_id":conversation,"p_session_id":session,"p_takeover":false}),
        );
        // Lease travels with the response. Cancellation drops it even if HTTP finishes late.
        let result = result.map(|value| {
            let lease = if value["status"] == "joined" {
                Some(Lease {
                    api: join_api,
                    session,
                })
            } else {
                None
            };
            (value, lease)
        });
        let _ = membership_tx.send(result);
    });
    let (joined, _lease) = loop {
        if !setup_controls(controls, state, &api, Some(&mut helper))? {
            return Ok(());
        }
        match membership_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(result) => break result?,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return Err("Voice membership worker ended".into()),
        }
    };
    if joined["status"] != "joined" {
        return Err(
            "Voice is active on another device. Leave that call before joining here.".into(),
        );
    }
    if !setup_controls(controls, state, &api, Some(&mut helper))? {
        return Ok(());
    }
    helper.send(json!({"command":"lease","url":api.url,"key":api.key,"token":api.token.lock().map_err(|_|"Voice authentication unavailable")?.clone(),"session":c.session}))?;
    let generation = joined["generation"]
        .as_str()
        .ok_or("Missing voice generation")?;
    let secret = joined["channel_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Missing private voice channel")?;
    if joined["conversation_id"] != c.conversation {
        return Err("Voice room identity mismatch".into());
    }
    let topic = format!("realtime:voice:{}:{generation}:{secret}", c.conversation);
    let sharing_screen = Arc::new(AtomicBool::new(false));
    let (_poller, polls) = poll(
        api.clone(),
        c.session.clone(),
        c.conversation.clone(),
        sharing_screen.clone(),
    );
    let mut screen = crate::screen_share::ScreenFlow::default();
    let mut sound_count = 0u64;
    let (broker_tx, broker_rx) = mpsc::channel();
    let screen_scope = json!({"conversationId":c.conversation,"voiceSessionId":c.session});
    let mut partner_session = None::<String>;
    let mut peer_session = None::<String>;
    if let Some(rows) = joined["participants"].as_array() {
        partner_session = rows
            .iter()
            .find(|r| r["user_id"] == c.partner)
            .and_then(|r| r["session_id"].as_str())
            .map(String::from);
    }
    let mut socket = None::<Socket>;
    let mut subscribed = false;
    let mut reconnect = Instant::now();
    let mut backoff = 1u64;
    let mut sequence = 1;
    let mut heartbeat = Instant::now();
    let mut last_ack = Instant::now();
    let mut connection_started = Instant::now();
    let mut restarts = 0;
    let mut outbox = VecDeque::<(Value, Instant)>::new();
    let mut in_flight = None::<(String, Value, Instant)>;
    let mut seen = VecDeque::<String>::new();
    let mut level_times = HashMap::<String, Instant>::new();
    state.participants = joined["participants"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| r["user_id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    state.status = "connecting".into();
    emit(state);
    loop {
        for (id, last) in &level_times {
            if last.elapsed() > Duration::from_millis(350)
                && state.levels.get(id).copied().unwrap_or(0.) > 0.
            {
                state.levels.insert(id.clone(), 0.);
                emit(state);
            }
        }
        let mut pending_effects = screen.tick();
        loop {
            let control = match controls.try_recv() {
                Ok(control) => control,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            };
            match control {
                Control::Routes { input, output } => {
                    helper.send(json!({"command":"routes","input":input,"output":output}))?
                }
                Control::Stop => {
                    let effects = screen.shutdown();
                    for effect in effects {
                        if let crate::screen_share::Effect::Broker(op, body) = effect {
                            broker(&api, &screen_scope, &broker_tx, op, body);
                        }
                    }
                    return Ok(());
                }
                Control::PeerData(message) => {
                    helper.send(json!({"command":"peer-data","text":message.to_string()}))?;
                }
                Control::OutputLevel(level) => {
                    helper.send(json!({"command":"output-level","level":level}))?;
                }
                Control::ScreenSources => helper.send(json!({"command":"screen-sources"}))?,
                Control::ScreenShare { id, kind, title, preview_port, preview_token } => {
                    pending_effects.extend(screen.start_share(id, kind, title, preview_port, preview_token));
                }
                Control::ScreenStop => pending_effects.extend(screen.stop_share(None)),
                Control::ScreenWatch { port, token } => {
                    pending_effects.extend(screen.start_watch(port, token));
                }
                Control::ScreenUnwatch => pending_effects.extend(screen.stop_watch(None)),
                Control::Controls { muted, deafened } => {
                    state.muted = muted;
                    state.deafened = deafened;
                    helper.send(json!({"command":"controls","muted":muted,"deafened":deafened}))?;
                    if subscribed && let Some(s) = socket.as_mut() {
                        let _ = send(
                            s,
                            &topic,
                            "presence",
                            json!({"event":"track","payload":{"userId":c.user,"sessionId":c.session,"muted":muted,"deafened":deafened,"sharingScreen":false,"joinedAt":joined["joined_at"]}}),
                            &mut sequence,
                        );
                    }
                    emit(state);
                }
                Control::Token(token) => {
                    helper.send(json!({"command":"token","token":token}))?;
                    *api.token
                        .lock()
                        .map_err(|_| "Voice authentication unavailable")? = token.clone();
                    if let Some(s) = socket.as_mut() {
                        let _ = send(
                            s,
                            &topic,
                            "access_token",
                            json!({"access_token":token}),
                            &mut sequence,
                        );
                    }
                }
            }
        }
        while let Ok(p) = polls.try_recv() {
            match p {
                Poll::LeaseLost => {
                    return Err("Call moved to another device or its room lease expired".into());
                }
                Poll::Turn(v) => {
                    if v["iceServers"].is_array() {
                        helper.send(json!({"command":"ice","servers":v["iceServers"]}))?;
                    }
                }
                Poll::Members(rows) => {
                    let rows = rows
                        .as_array()
                        .ok_or("Invalid voice participant response")?;
                    if !rows
                        .iter()
                        .any(|r| r["user_id"] == c.user && r["session_id"] == c.session)
                    {
                        return Err("Call membership ended or moved to another device".into());
                    }
                    let next = rows
                        .iter()
                        .find(|r| r["user_id"] == c.partner)
                        .and_then(|r| r["session_id"].as_str())
                        .map(String::from);
                    if next != partner_session {
                        helper.send(json!({"command":"clear-peer"}))?;
                        pending_effects.extend(screen.on_partner_left());
                        partner_session = next;
                        peer_session = None;
                        outbox.clear();
                        in_flight = None;
                        seen.clear();
                        state.remote_muted = false;
                        state.receiving = false;
                        state.levels.insert(c.partner.clone(), 0.);
                    }
                    state.participants = rows
                        .iter()
                        .filter_map(|r| r["user_id"].as_str().map(String::from))
                        .collect();
                    emit(state);
                }
            }
        }
        if subscribed && partner_session.is_some() && peer_session != partner_session {
            helper.send(
                json!({"command":"peer","polite":c.user>c.partner,"epoch":partner_session}),
            )?;
            peer_session = partner_session.clone();
            connection_started = Instant::now();
            restarts = 0;
            helper.send(
                json!({"command":"controls","muted":state.muted,"deafened":state.deafened}),
            )?;
            outbox.push_back((
                signal(
                    json!({"type":"ready"}),
                    generation,
                    &c.session,
                    partner_session.as_deref(),
                ),
                Instant::now(),
            ));
        }
        while let Ok(v) = helper.events.try_recv() {
            if v["epoch"]
                .as_str()
                .is_some_and(|epoch| !epoch.is_empty() && Some(epoch) != peer_session.as_deref())
            {
                continue;
            }
            pending_effects.extend(screen.on_engine(&v));
            match v["event"].as_str() {
                Some("peer-data") => {
                    if let Ok(message) = serde_json::from_str::<Value>(v["text"].as_str().unwrap_or("")) {
                        // Ready at once: a cache miss then costs a little sync, not the sound.
                        if message["type"] == "soundboard-prepare" {
                            let reply = json!({"type":"soundboard-ready","payload":{
                                "version":1,"id":message["payload"]["id"],"nonce":message["payload"]["nonce"],
                            }});
                            helper.send(json!({"command":"peer-data","text":reply.to_string()}))?;
                        }
                        sound_count = note_sound(state, sound_count, message);
                        emit(state);
                    }
                }
                Some("peer-data-failed") => {
                    if let Ok(message) = serde_json::from_str::<Value>(v["text"].as_str().unwrap_or(""))
                        && subscribed
                        && let Some(s) = socket.as_mut()
                    {
                        let _ = send(
                            s,
                            &topic,
                            "broadcast",
                            json!({"type":"broadcast","event":message["type"],"payload":message["payload"]}),
                            &mut sequence,
                        );
                    }
                }
                Some("local-meter") => {
                    state.local_meter_status = v["status"].as_str().unwrap_or("unavailable").to_string();
                    emit(state);
                }
                Some("level") => {
                    let id = if v["side"] == "local" {
                        &c.user
                    } else {
                        &c.partner
                    };
                    state.levels.insert(
                        id.clone(),
                        v["level"].as_f64().unwrap_or(0.).clamp(0., 1.) as f32,
                    );
                    level_times.insert(id.clone(), Instant::now());
                    emit(state);
                }
                Some("error") => {
                    return Err(v["message"]
                        .as_str()
                        .unwrap_or("Native audio operation failed")
                        .into());
                }
                Some("description") => outbox.push_back((
                    signal(
                        json!({"type":"description","description":v["description"]}),
                        generation,
                        &c.session,
                        partner_session.as_deref(),
                    ),
                    Instant::now(),
                )),
                Some("candidate") => outbox.push_back((
                    signal(
                        json!({"type":"ice-candidate","candidate":v["candidate"]}),
                        generation,
                        &c.session,
                        partner_session.as_deref(),
                    ),
                    Instant::now(),
                )),
                Some("state") => {
                    state.status = v["state"].as_str().unwrap_or("connecting").into();
                    if state.status == "new" {
                        state.status = "connecting".into();
                    }
                    if state.status == "disconnected" || state.status == "failed" {
                        state.status = "reconnecting".into();
                        connection_started = Instant::now();
                    }
                    if state.status == "connected" {
                        restarts = 0;
                    }
                    emit(state);
                }
                Some("first-decoded") if v["side"] == "remote" => {
                    helper.send(json!({"command":"remote-ready","epoch":v["epoch"]}))?;
                    state.receiving = true;
                    emit(state);
                }
                _ => {}
            }
        }
        while let Ok((op, result)) = broker_rx.try_recv() {
            pending_effects.extend(screen.on_broker(op, result));
        }
        for effect in pending_effects.drain(..) {
            match effect {
                crate::screen_share::Effect::Engine(command) => helper.send(command)?,
                crate::screen_share::Effect::Signal(body) => outbox.push_back((
                    signal(body, generation, &c.session, partner_session.as_deref()),
                    Instant::now(),
                )),
                crate::screen_share::Effect::Broker(op, body) => {
                    broker(&api, &screen_scope, &broker_tx, op, body);
                }
            }
        }
        sharing_screen.store(screen.sharing(), Ordering::Release);
        if state.screen != screen.status {
            state.screen = screen.status.clone();
            emit(state);
        }
        while outbox.len() > 64 {
            outbox.pop_front();
        }
        if partner_session.is_none() && state.status != "solo" {
            state.status = "solo".into();
            emit(state);
        }
        if peer_session.is_some()
            && state.status != "connected"
            && connection_started.elapsed() > Duration::from_secs(8)
        {
            if restarts >= 3 {
                return Err("Voice could not establish audio after relay recovery. Check your network and retry.".into());
            }
            helper.send(json!({"command":"restart"}))?;
            restarts += 1;
            connection_started = Instant::now();
        }
        if socket.is_none() && Instant::now() >= reconnect {
            match connect(&api, &topic, &c.session) {
                Ok(s) => {
                    socket = Some(s);
                    last_ack = Instant::now();
                }
                Err(_) => {
                    reconnect = Instant::now() + Duration::from_secs(backoff);
                    backoff = (backoff * 2).min(8);
                }
            }
        }
        let mut lost = false;
        if let Some(s) = socket.as_mut() {
            if last_ack.elapsed() > Duration::from_secs(45) {
                lost = true;
            }
            if heartbeat.elapsed() > Duration::from_secs(20) {
                sequence += 1;
                if s.send(Message::Text(json!({"topic":"phoenix","event":"heartbeat","ref":sequence.to_string(),"payload":{}}).to_string().into())).is_err(){lost=true;}
                heartbeat = Instant::now();
            }
            if let Some((_, v, at)) = &in_flight
                && at.elapsed() > Duration::from_secs(3)
            {
                outbox.push_front((v.clone(), Instant::now()));
                in_flight = None;
                lost = true;
            }
            if subscribed
                && in_flight.is_none()
                && let Some((v, at)) = outbox.pop_front()
                && at.elapsed() < Duration::from_secs(20)
            {
                match send(
                    s,
                    &topic,
                    "broadcast",
                    json!({"type":"broadcast","event":"signal","payload":v}),
                    &mut sequence,
                ) {
                    Ok(id) => in_flight = Some((id, v, Instant::now())),
                    Err(_) => {
                        outbox.push_front((v, at));
                        lost = true;
                    }
                }
            }
            match s.read() {
                Ok(Message::Text(text)) => {
                    let v: Value = serde_json::from_str(text.as_str())
                        .map_err(|_| "Invalid voice signaling response")?;
                    match v["event"].as_str() {
                        Some("phx_reply") if v["ref"] == "1" => {
                            if v["payload"]["status"] != "ok" {
                                return Err(format!(
                                    "Private voice signaling authorization failed: {}",
                                    v["payload"]["response"]["reason"].as_str().unwrap_or("no reason given")
                                ));
                            }
                            subscribed = true;
                            backoff = 1;
                            last_ack = Instant::now();
                            let _ = send(
                                s,
                                &topic,
                                "presence",
                                json!({"event":"track","payload":{"userId":c.user,"sessionId":c.session,"muted":state.muted,"deafened":state.deafened,"sharingScreen":false,"joinedAt":joined["joined_at"]}}),
                                &mut sequence,
                            );
                            outbox.push_back((
                                signal(
                                    json!({"type":"ready"}),
                                    generation,
                                    &c.session,
                                    partner_session.as_deref(),
                                ),
                                Instant::now(),
                            ));
                        }
                        Some("phx_reply") => {
                            last_ack = Instant::now();
                            if in_flight.as_ref().is_some_and(|(id, _, _)| v["ref"] == *id) {
                                if v["payload"]["status"] == "ok" {
                                    in_flight = None;
                                } else {
                                    lost = true;
                                }
                            }
                        }
                        Some("presence_state" | "presence_diff") => {
                            let payload = if v["event"] == "presence_diff" {
                                &v["payload"]["joins"]
                            } else {
                                &v["payload"]
                            };
                            if let Some(entries) = payload.as_object() {
                                for entry in entries.values() {
                                    if let Some(metas) = entry["metas"].as_array() {
                                        for meta in metas {
                                            if meta["userId"] == c.partner
                                                && partner_session
                                                    .as_deref()
                                                    .is_some_and(|s| meta["sessionId"] == s)
                                            {
                                                state.remote_muted = meta["muted"]
                                                    .as_bool()
                                                    .unwrap_or(false)
                                                    || meta["deafened"].as_bool().unwrap_or(false);
                                                emit(state);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some("broadcast")
                            if subscribed
                                && matches!(
                                    v["payload"]["event"].as_str(),
                                    Some("soundboard" | "soundboard-stop")
                                ) =>
                        {
                            sound_count = note_sound(
                                state,
                                sound_count,
                                json!({"type":v["payload"]["event"],"payload":v["payload"]["payload"]}),
                            );
                            emit(state);
                        }
                        Some("broadcast") if subscribed && v["payload"]["event"] == "signal" => {
                            let signal = &v["payload"]["payload"];
                            if accepts(signal, generation, &c.session, partner_session.as_deref()) {
                                let id = signal["signalId"].as_str().map(String::from);
                                if id.as_ref().is_some_and(|id| seen.contains(id)) {
                                    continue;
                                }
                                if let Some(id) = id {
                                    seen.push_back(id);
                                    while seen.len() > 256 {
                                        seen.pop_front();
                                    }
                                }
                                let screen_effects = screen.on_signal(signal);
                                for effect in screen_effects {
                                    match effect {
                                        crate::screen_share::Effect::Engine(command) => helper.send(command)?,
                                        crate::screen_share::Effect::Broker(op, body) => {
                                            broker(&api, &screen_scope, &broker_tx, op, body);
                                        }
                                        crate::screen_share::Effect::Signal(_) => {}
                                    }
                                }
                                if signal["type"] == "ready" {
                                    for effect in screen.on_partner_ready() {
                                        if let crate::screen_share::Effect::Signal(body) = effect {
                                            outbox.push_back((
                                                signal_body(body, generation, &c.session, partner_session.as_deref()),
                                                Instant::now(),
                                            ));
                                        }
                                    }
                                }
                                match signal["type"].as_str(){Some("description")=>helper.send(json!({"command":"description","description":signal["description"]}))?,Some("ice-candidate")=>helper.send(json!({"command":"candidate","candidate":signal["candidate"]}))?,_=>{}}
                            }
                        }
                        Some("phx_error" | "phx_close") => lost = true,
                        _ => {}
                    }
                }
                Ok(Message::Ping(_)) => {
                    let _ = s.flush();
                }
                Ok(Message::Close(_)) => lost = true,
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => lost = true,
            }
        } else {
            std::thread::sleep(Duration::from_millis(50));
        }
        if lost {
            socket = None;
            subscribed = false;
            if let Some((_, v, _)) = in_flight.take() {
                outbox.push_front((v, Instant::now()));
            }
            reconnect = Instant::now() + Duration::from_secs(backoff);
            backoff = (backoff * 2).min(8);
        }
    }
}
fn setup_controls(
    controls: &mpsc::Receiver<Control>,
    state: &mut State,
    api: &Api,
    mut helper: Option<&mut Helper>,
) -> Result<bool, String> {
    loop {
        match controls.try_recv() {
            Ok(Control::Stop) | Err(mpsc::TryRecvError::Disconnected) => return Ok(false),
            Err(mpsc::TryRecvError::Empty) => return Ok(true),
            Ok(Control::Token(token)) => {
                *api.token
                    .lock()
                    .map_err(|_| "Voice authentication unavailable")? = token
            }
            Ok(Control::Controls { muted, deafened }) => {
                state.muted = muted;
                state.deafened = deafened;
                if let Some(h) = helper.as_mut() {
                    h.send(json!({"command":"controls","muted":muted,"deafened":deafened}))?;
                }
            }
            // Sharing is only offered once the call is up.
            Ok(
                Control::ScreenSources
                | Control::ScreenShare { .. }
                | Control::ScreenStop
                | Control::ScreenWatch { .. }
                | Control::ScreenUnwatch
                | Control::PeerData(_)
                | Control::OutputLevel(_),
            ) => {}
            Ok(Control::Routes { input, output }) => {
                if let Some(h) = helper.as_mut() {
                    h.send(json!({"command":"routes","input":input,"output":output}))?;
                }
            }
        }
    }
}
fn accepts(v: &Value, generation: &str, session: &str, partner: Option<&str>) -> bool {
    v["version"] == 1
        && v["generation"] == generation
        && partner.is_some_and(|p| v["fromSessionId"] == p)
        && v["fromSessionId"] != session
        && v.get("toSessionId").is_none_or(|to| to == session)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signals_require_current_authenticated_seat() {
        let v = json!({"version":1,"generation":"g","fromSessionId":"peer","toSessionId":"me"});
        assert!(accepts(&v, "g", "me", Some("peer")));
        assert!(!accepts(&v, "old", "me", Some("peer")));
        assert!(!accepts(&v, "g", "me", Some("other")));
        assert!(!accepts(&v, "g", "someone", Some("peer")));
        assert!(!accepts(&v, "g", "me", None));
    }
    #[test]
    fn canceled_join_cannot_attach_and_mute_survives_pending_start() {
        let controller = Controller::default();
        controller.want("first".into());
        controller.control(Control::Controls {
            muted: true,
            deafened: false,
        });
        let (tx, rx) = mpsc::channel();
        controller.attach("first", Handle { tx });
        assert!(matches!(
            rx.try_recv(),
            Ok(Control::Controls {
                muted: true,
                deafened: false
            })
        ));
        controller.control(Control::Stop);
        assert!(!controller.wanted("first"));
        assert!(matches!(rx.try_recv(), Ok(Control::Stop)));
        let (tx, rx) = mpsc::channel();
        controller.attach("first", Handle { tx });
        assert!(matches!(rx.try_recv(), Ok(Control::Stop)));
        controller.want("second".into());
        assert!(controller.wanted("second"));
    }
}
