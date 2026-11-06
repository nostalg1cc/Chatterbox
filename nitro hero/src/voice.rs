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
impl Api {
    fn new(c: &Start) -> Result<Self, String> {
        Ok(Api {
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
        })
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
    poke: Arc<AtomicBool>,
) -> (Poller, mpsc::Receiver<Poll>) {
    let stop = Arc::new(AtomicBool::new(false));
    let end = stop.clone();
    let (tx, rx) = mpsc::sync_channel(8);
    // On its own thread so the relay credentials never wait behind membership requests,
    // or the other way around. They last 12 hours, so they're rarely asked for.
    let (turn_api, turn_tx, turn_end) = (api.clone(), tx.clone(), stop.clone());
    let (turn_session, turn_conversation) = (session.clone(), conversation.clone());
    std::thread::spawn(move || {
        let mut next = Instant::now();
        while !turn_end.load(Ordering::Acquire) {
            if Instant::now() >= next {
                let reply = turn_api.request(
                    "functions/v1/realtime-credentials",
                    Some(json!({"conversationId":turn_conversation,"voiceSessionId":turn_session})),
                    &[],
                );
                let delivered = reply.is_ok_and(|v| {
                    v["iceServers"].is_array() && turn_tx.try_send(Poll::Turn(v)).is_ok()
                });
                next = Instant::now()
                    + Duration::from_secs(if delivered { 6 * 3600 } else { 15 });
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    });
    std::thread::spawn(move || {
        let mut lease = Instant::now() - Duration::from_secs(45);
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
            for _ in 0..20 {
                if end.load(Ordering::Acquire) {
                    return;
                }
                if poke.swap(false, Ordering::AcqRel) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
    (Poller(stop), rx)
}
type Socket = tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>;
/// Ends this engine so the same call carries on with a new one. A second connection inside
/// one engine connects and receives packets but never decodes them, so anything that
/// needs a new connection gets a new engine, under the same seat.
const FRESH_ENGINE: &str = "fresh-engine";
/// What to try next while a call has no audio path. It never gives up on its own: the
/// call lasts until someone leaves, and a partner who's gone drops off the member list.
#[derive(Debug, PartialEq)]
enum Recovery {
    Restart { relay: bool },
    Rebuild,
}
fn recovery_step(attempt: u32) -> Recovery {
    match attempt % 3 {
        0 => Recovery::Restart { relay: false },
        1 => Recovery::Restart { relay: true },
        _ => Recovery::Rebuild,
    }
}
fn recovery_delay(attempt: u32) -> Duration {
    Duration::from_secs((6 + 2 * attempt as u64).min(20))
}
/// Same limits as main Nitro: enough lost packets or jitter to sound robotic.
fn unhealthy(received: u64, lost: i64, jitter: f64) -> bool {
    let total = received as i64 + lost.max(0);
    total >= 40 && (lost.max(0) as f64 / total as f64 >= 0.08 || jitter >= 0.08)
}
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
        let api = match Api::new(&config) {
            Ok(api) => api,
            Err(error) => {
                state.status = "failed".into();
                state.error = Some(error);
                emit(&state);
                return;
            }
        };
        // Anything that goes wrong after joining picks the same call back up with a fresh
        // engine, under the same seat, so one fault doesn't throw anyone out. A call that
        // really has moved to another device is refused by the join and ends here.
        let mut retries = 0;
        let error = loop {
            let mut helper = None::<Helper>;
            let mut lease = None::<Lease>;
            let began = Instant::now();
            let result = run(&config, &api, &rx, &mut state, &emit, &mut helper, &mut lease);
            if began.elapsed() > Duration::from_secs(60) {
                retries = 0;
            }
            let error = match result {
                Ok(()) => break None,
                Err(error) => error,
            };
            let fresh = error == FRESH_ENGINE;
            if lease.is_none() || (retries >= 5 && !fresh) {
                break Some(error);
            }
            if let Some(helper) = helper.as_mut() {
                let _ = helper.send(json!({"command":"forget-lease"}));
            }
            drop(helper);
            std::mem::forget(lease);
            retries += u32::from(!fresh);
            state.status = "reconnecting".into();
            state.receiving = false;
            state.screen = Default::default();
            emit(&state);
            let wait = Instant::now();
            let mut stopped = false;
            let pause = Duration::from_millis(if fresh { 200 } else { 2000 });
            while wait.elapsed() < pause && !stopped {
                std::thread::sleep(Duration::from_millis(50));
                stopped = !setup_controls(&rx, &mut state, &api, None).unwrap_or(false);
            }
            if stopped {
                drop(Lease { api: api.clone(), session: config.session.clone() });
                break None;
            }
        };
        match error {
            Some(error) => {
                state.status = "failed".into();
                state.error = Some(error);
            }
            None => state.status = "idle".into(),
        }
        emit(&state);
    });
    Handle { tx }
}
fn run(
    c: &Start,
    api: &Api,
    controls: &mpsc::Receiver<Control>,
    state: &mut State,
    emit: &impl Fn(&State),
    helper_slot: &mut Option<Helper>,
    lease_slot: &mut Option<Lease>,
) -> Result<(), String> {
    let helper = helper_slot.insert(Helper::start()?);
    helper.send(json!({"command":"prepare","input":c.input,"output":c.output}))?;
    let ready = Instant::now();
    loop {
        // The helper processes stdin sequentially: controls received during preparation
        // run immediately afterwards, before membership/signaling starts. Never discard
        // a route selection merely because the prepared event has not arrived yet.
        if !setup_controls(controls, state, api, Some(&mut *helper))? {
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
    let (joined, lease) = loop {
        if !setup_controls(controls, state, api, Some(&mut *helper))? {
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
    *lease_slot = lease;
    if !setup_controls(controls, state, api, Some(&mut *helper))? {
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
    let poke = Arc::new(AtomicBool::new(false));
    let (_poller, polls) = poll(
        api.clone(),
        c.session.clone(),
        c.conversation.clone(),
        sharing_screen.clone(),
        poke.clone(),
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
    let mut dialing = None::<mpsc::Receiver<Result<Socket, String>>>;
    let mut ever_subscribed = false;
    let mut subscribed = false;
    let mut reconnect = Instant::now();
    let mut backoff = 1u64;
    let mut sequence = 1;
    let mut heartbeat = Instant::now();
    let mut last_ack = Instant::now();
    let started = Instant::now();
    let mut turn_ready = false;
    let mut attempt = 0u32;
    let mut next_recovery = Instant::now();
    let mut relay_preferred = false;
    let mut health_at = Instant::now();
    let mut health_last = None::<(u64, i64)>;
    let mut health_bad = 0;
    let mut membership_misses = 0;
    let mut slow_acks = 0;
    let mut last_rebuild = Instant::now();
    let mut had_peer = false;
    // Signals from someone the member list hasn't shown yet. Dropping them left both
    // sides waiting: the other side ignores offers while its own offer is unanswered.
    let mut early = VecDeque::<(Value, Instant)>::new();
    let mut incoming = VecDeque::<Value>::new();
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
                        turn_ready = true;
                    }
                }
                Poll::Members(rows) => {
                    let Some(rows) = rows.as_array() else { continue };
                    // One odd read isn't enough to end somebody's call.
                    if rows
                        .iter()
                        .any(|r| r["user_id"] == c.user && r["session_id"] == c.session)
                    {
                        membership_misses = 0;
                    } else {
                        membership_misses += 1;
                        if membership_misses >= 3 {
                            return Err("Call membership ended or moved to another device".into());
                        }
                        continue;
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
        // The first offer should already include the relay, so it isn't needed as a second
        // try. Credentials that are slow to arrive don't hold the call up for long.
        if subscribed
            && partner_session.is_some()
            && peer_session != partner_session
            && (turn_ready || started.elapsed() > Duration::from_secs(3))
        {
            if had_peer {
                return Err(FRESH_ENGINE.into());
            }
            had_peer = true;
            last_rebuild = Instant::now();
            helper.send(
                json!({"command":"peer","polite":c.user>c.partner,"epoch":partner_session,"relay":relay_preferred}),
            )?;
            peer_session = partner_session.clone();
            next_recovery = Instant::now() + Duration::from_secs(8);
            attempt = 0;
            health_last = None;
            health_bad = 0;
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
                // The other side rebuilt its connection in a way this one can't accept, so
                // this one has to start fresh as well.
                Some("warning")
                    if v["command"] == "description"
                        && peer_session.is_some()
                        && last_rebuild.elapsed() > Duration::from_secs(5) =>
                {
                    return Err(FRESH_ENGINE.into());
                }
                Some("audio-health") => {
                    let now = (v["received"].as_u64().unwrap_or(0), v["lost"].as_i64().unwrap_or(0));
                    if let Some(before) = health_last {
                        // Their report counts too: that's the side that hears the damage.
                        let bad = unhealthy(
                            now.0.saturating_sub(before.0),
                            now.1 - before.1,
                            v["jitter"].as_f64().unwrap_or(0.),
                        ) || v["sentLoss"].as_f64().unwrap_or(0.) >= 0.08;
                        health_bad = if bad { health_bad + 1 } else { 0 };
                        if health_bad >= 2 && !relay_preferred && turn_ready {
                            relay_preferred = true;
                            helper.send(json!({"command":"restart","relay":true}))?;
                        }
                    }
                    health_last = Some(now);
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
                // A connection that was just replaced saying it has closed.
                Some("state") if v["state"] == "closed" => {}
                Some("state") => {
                    state.status = v["state"].as_str().unwrap_or("connecting").into();
                    if state.status == "new" {
                        state.status = "connecting".into();
                    }
                    if state.status == "disconnected" || state.status == "failed" {
                        // Disconnected often heals by itself. Failed never does.
                        let wait = if state.status == "failed" { 1 } else { 6 };
                        next_recovery = next_recovery.min(Instant::now() + Duration::from_secs(wait));
                        state.status = "reconnecting".into();
                    }
                    if state.status == "connected" {
                        attempt = 0;
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
        // Offers need signaling, so an outage there waits until it's back.
        if subscribed
            && peer_session.is_some()
            && state.status != "connected"
            && Instant::now() >= next_recovery
        {
            match recovery_step(attempt) {
                Recovery::Restart { relay } => {
                    helper.send(json!({"command":"restart","relay":relay || relay_preferred}))?;
                }
                Recovery::Rebuild => return Err(FRESH_ENGINE.into()),
            }
            attempt += 1;
            next_recovery = Instant::now() + recovery_delay(attempt);
            if state.status != "reconnecting" {
                state.status = "reconnecting".into();
                emit(state);
            }
        }
        if state.status == "connected" && health_at.elapsed() >= Duration::from_secs(3) {
            health_at = Instant::now();
            helper.send(json!({"command":"stats"}))?;
        }
        // Connecting can hang for a long time on a bad network, and this loop also has to
        // keep the call's audio events and controls moving.
        if socket.is_none() && dialing.is_none() && Instant::now() >= reconnect {
            let (done, result) = mpsc::sync_channel(1);
            let (dial_api, dial_topic, dial_session) = (api.clone(), topic.clone(), c.session.clone());
            std::thread::spawn(move || {
                let _ = done.send(connect(&dial_api, &dial_topic, &dial_session));
            });
            dialing = Some(result);
        }
        if let Some(result) = &dialing {
            match result.try_recv() {
                Ok(Ok(s)) => {
                    socket = Some(s);
                    dialing = None;
                    last_ack = Instant::now();
                    heartbeat = Instant::now();
                }
                Err(mpsc::TryRecvError::Empty) => {}
                _ => {
                    dialing = None;
                    reconnect = Instant::now() + Duration::from_secs(backoff);
                    backoff = (backoff * 2).min(8);
                }
            }
        }
        let mut lost = false;
        if let Some(s) = socket.as_mut() {
            if last_ack.elapsed() > Duration::from_secs(if subscribed { 45 } else { 10 }) {
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
                // A slow reply isn't a dead connection, and a repeat is ignored if both land.
                outbox.push_front((v.clone(), Instant::now()));
                in_flight = None;
                slow_acks += 1;
                if slow_acks >= 2 {
                    slow_acks = 0;
                    lost = true;
                }
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
                    let Ok(v) = serde_json::from_str::<Value>(text.as_str()) else {
                        continue;
                    };
                    match v["event"].as_str() {
                        Some("phx_reply") if v["ref"] == "1" => {
                            if v["payload"]["status"] != "ok" {
                                // It let this login in before, so this is a passing
                                // problem such as a token that just expired.
                                if ever_subscribed {
                                    socket = None;
                                    subscribed = false;
                                    reconnect = Instant::now() + Duration::from_secs(backoff);
                                    backoff = (backoff * 2).min(8);
                                    continue;
                                }
                                return Err(format!(
                                    "Private voice signaling authorization failed: {}",
                                    v["payload"]["response"]["reason"].as_str().unwrap_or("no reason given")
                                ));
                            }
                            subscribed = true;
                            ever_subscribed = true;
                            backoff = 1;
                            last_ack = Instant::now();
                            // Anything sent while signaling was down never arrived.
                            if state.status != "connected" {
                                next_recovery = next_recovery.min(Instant::now() + Duration::from_secs(1));
                            }
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
                                    slow_acks = 0;
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
                            if accepts(signal, generation, &c.session, peer_session.as_deref()) {
                                incoming.push_back(signal.clone());
                            } else if accepts(signal, generation, &c.session, signal["fromSessionId"].as_str()) {
                                early.push_back((signal.clone(), Instant::now()));
                                while early.len() > 128 {
                                    early.pop_front();
                                }
                                poke.store(true, Ordering::Release);
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
        if peer_session.is_some() {
            while let Some((signal, at)) = early.pop_front() {
                if signal["fromSessionId"].as_str() == peer_session.as_deref()
                    && at.elapsed() < Duration::from_secs(20)
                {
                    incoming.push_back(signal);
                }
            }
        } else if early.front().is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(20)) {
            early.pop_front();
        }
        while let Some(signal) = incoming.pop_front() {
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
            for effect in screen.on_signal(&signal) {
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
            match signal["type"].as_str() {
                Some("description") => {
                    helper.send(json!({"command":"description","description":signal["description"]}))?
                }
                Some("ice-candidate") => {
                    helper.send(json!({"command":"candidate","candidate":signal["candidate"]}))?
                }
                _ => {}
            }
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
    fn recovery_tries_every_route_and_keeps_going() {
        assert_eq!(recovery_step(0), Recovery::Restart { relay: false });
        assert_eq!(recovery_step(1), Recovery::Restart { relay: true });
        assert_eq!(recovery_step(2), Recovery::Rebuild);
        assert_eq!(recovery_step(3), Recovery::Restart { relay: false });
        assert!(recovery_delay(1000) <= Duration::from_secs(20));
    }
    #[test]
    fn only_real_packet_trouble_counts_as_unhealthy() {
        assert!(!unhealthy(10, 5, 0.));
        assert!(!unhealthy(100, 2, 0.01));
        assert!(unhealthy(90, 10, 0.01));
        assert!(unhealthy(100, 0, 0.1));
        assert!(!unhealthy(100, -3, 0.));
    }
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
