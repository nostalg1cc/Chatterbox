//! Typing signals and who's online, over the same public Realtime channels main Nitro uses.
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::mpsc,
    time::{Duration, Instant},
};
use tungstenite::{Message, stream::MaybeTlsStream};

type Socket = tungstenite::WebSocket<MaybeTlsStream<std::net::TcpStream>>;

enum Command {
    Watch(String),
    Typing(String),
}

pub struct TypingChannel {
    commands: mpsc::Sender<Command>,
    events: mpsc::Receiver<String>,
    watching: Option<String>,
    last_sent: Option<Instant>,
}

impl TypingChannel {
    /// Stops by itself once this is dropped.
    pub fn start() -> Self {
        let (commands, command_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        std::thread::spawn(move || run(command_rx, event_tx));
        Self { commands, events, watching: None, last_sent: None }
    }

    pub fn watch(&mut self, conversation: &str) {
        if self.watching.as_deref() != Some(conversation) {
            self.watching = Some(conversation.to_string());
            self.last_sent = None;
            let _ = self.commands.send(Command::Watch(conversation.to_string()));
        }
    }

    /// At most every 2 seconds, as in main Nitro.
    pub fn send_typing(&mut self, user: &str) {
        if self.last_sent.is_none_or(|sent| sent.elapsed() >= Duration::from_secs(2)) {
            self.last_sent = Some(Instant::now());
            let _ = self.commands.send(Command::Typing(user.to_string()));
        }
    }

    /// Users heard typing in the watched conversation since the last call.
    pub fn take(&self) -> Vec<String> {
        self.events.try_iter().collect()
    }
}

fn topic(conversation: &str) -> String {
    format!("realtime:typing:{conversation}")
}

fn connect() -> Option<(Socket, String)> {
    let (url, key, token) = crate::backend::voice_credentials().ok()?;
    let host = url.strip_prefix("https://")?;
    let endpoint = format!("wss://{host}/realtime/v1/websocket?apikey={key}&vsn=1.0.0");
    let (mut socket, _) = tungstenite::connect(endpoint).ok()?;
    let stream = match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => stream,
        MaybeTlsStream::NativeTls(stream) => stream.get_mut(),
        _ => return None,
    };
    stream.set_read_timeout(Some(Duration::from_millis(100))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(3))).ok()?;
    Some((socket, token))
}

fn send(socket: &mut Socket, topic: &str, event: &str, payload: Value, sequence: &mut u64) -> bool {
    *sequence += 1;
    let message = json!({"topic":topic,"event":event,"payload":payload,"ref":sequence.to_string()});
    socket.send(Message::Text(message.to_string().into())).is_ok()
}

fn join(socket: &mut Socket, conversation: &str, token: &str, sequence: &mut u64) -> bool {
    send(
        socket,
        &topic(conversation),
        "phx_join",
        json!({
            "config":{
                "broadcast":{"self":false,"ack":false},
                "presence":{"key":""},
                "postgres_changes":[],
                "private":false,
            },
            "access_token":token,
        }),
        sequence,
    )
}

fn run(commands: mpsc::Receiver<Command>, events: mpsc::Sender<String>) {
    let mut watching = None::<String>;
    let mut backoff = 1;
    loop {
        // Nothing to listen to yet, so don't hold a connection.
        while watching.is_none() {
            match commands.recv() {
                Ok(Command::Watch(conversation)) => watching = Some(conversation),
                Ok(Command::Typing(_)) => {}
                Err(_) => return,
            }
        }
        let Some((mut socket, token)) = connect() else {
            // Drain during the wait, so the newest conversation is the one joined next.
            let until = Instant::now() + Duration::from_secs(backoff);
            backoff = (backoff * 2).min(30);
            while Instant::now() < until {
                match commands.recv_timeout(Duration::from_millis(250)) {
                    Ok(Command::Watch(conversation)) => watching = Some(conversation),
                    Ok(Command::Typing(_)) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            continue;
        };
        let mut sequence = 0;
        let mut joined = watching.clone();
        let mut healthy = join(&mut socket, joined.as_deref().unwrap_or(""), &token, &mut sequence);
        let mut heartbeat = Instant::now();
        while healthy {
            loop {
                match commands.try_recv() {
                    Ok(Command::Watch(conversation)) => {
                        if let Some(old) = joined.take() {
                            healthy &= send(&mut socket, &topic(&old), "phx_leave", json!({}), &mut sequence);
                        }
                        healthy &= join(&mut socket, &conversation, &token, &mut sequence);
                        watching = Some(conversation.clone());
                        joined = Some(conversation);
                    }
                    Ok(Command::Typing(user)) => {
                        if let Some(conversation) = &joined {
                            healthy &= send(
                                &mut socket,
                                &topic(conversation),
                                "broadcast",
                                json!({"type":"broadcast","event":"typing","payload":{"userId":user}}),
                                &mut sequence,
                            );
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            }
            if heartbeat.elapsed() > Duration::from_secs(25) {
                healthy &= send(&mut socket, "phoenix", "heartbeat", json!({}), &mut sequence);
                heartbeat = Instant::now();
            }
            match socket.read() {
                Ok(Message::Text(text)) => {
                    backoff = 1;
                    let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                    match value["event"].as_str() {
                        Some("broadcast") if value["payload"]["event"] == "typing" => {
                            if let Some(user) = value["payload"]["payload"]["userId"].as_str()
                                && events.send(user.to_string()).is_err()
                            {
                                return;
                            }
                        }
                        // For example an expired token. Reconnecting gets a fresh one.
                        Some("phx_error" | "phx_close") => healthy = false,
                        Some("phx_reply") if value["payload"]["status"] == "error" => healthy = false,
                        _ => {}
                    }
                }
                Ok(Message::Close(_)) => healthy = false,
                Ok(_) => {}
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => healthy = false,
            }
        }
        let until = Instant::now() + Duration::from_secs(backoff);
        backoff = (backoff * 2).min(30);
        while Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Who's online, by user id, with when each was last active. Being in this channel is also
/// what makes this user show as online to everyone else.
pub struct PresenceChannel {
    active: mpsc::Sender<()>,
    events: mpsc::Receiver<HashMap<String, i64>>,
    last_active: Instant,
    online: HashMap<String, i64>,
}

impl PresenceChannel {
    /// Stops by itself once this is dropped.
    pub fn start(user: String) -> Self {
        let (active, active_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        std::thread::spawn(move || run_presence(user, active_rx, event_tx));
        Self { active, events, last_active: Instant::now(), online: HashMap::new() }
    }

    /// At most every 30 seconds, as in main Nitro.
    pub fn note_active(&mut self) {
        if self.last_active.elapsed() >= Duration::from_secs(30) {
            self.last_active = Instant::now();
            let _ = self.active.send(());
        }
    }

    pub fn refresh(&mut self) {
        for online in self.events.try_iter() {
            self.online = online;
        }
    }

    pub fn status(&self, user: &str) -> &'static str {
        presence_status(self.online.get(user).copied(), now_ms())
    }
}

/// Same rule as main Nitro: away after 10 minutes without activity.
fn presence_status(active_at: Option<i64>, now: i64) -> &'static str {
    match active_at {
        None => "offline",
        Some(at) if now - at > 10 * 60_000 => "away",
        Some(_) => "online",
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

/// Whole-day arithmetic for the UTC timestamps both apps write.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year.rem_euclid(400);
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    era * 146_097 + of_era * 365 + of_era / 4 - of_era / 100 + of_year - 719_468
}

fn parse_timestamp(text: &str) -> Option<i64> {
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let days = days_from_civil(number(0..4)?, number(5..7)?, number(8..10)?);
    let seconds = number(11..13)? * 3600 + number(14..16)? * 60 + number(17..19)?;
    let millis = text.get(20..23).and_then(|part| part.parse::<i64>().ok()).unwrap_or(0);
    Some((days * 86_400 + seconds) * 1000 + millis)
}

fn format_timestamp(ms: i64) -> String {
    let (days, rest) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let of_era = shifted.rem_euclid(146_097);
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * of_year + 2) / 153;
    let day = of_year - (153 * month_index + 2) / 5 + 1;
    let month = month_index + if month_index < 10 { 3 } else { -9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1000 % 60,
        rest % 1000,
    )
}

/// Applies a presence_state or presence_diff message. Each connection is its own entry.
fn apply_presence(
    seen: &mut HashMap<String, HashMap<String, i64>>,
    event: &str,
    payload: &Value,
    now: i64,
) {
    let add = |seen: &mut HashMap<String, HashMap<String, i64>>, entries: &Value| {
        for (user, entry) in entries.as_object().into_iter().flatten() {
            for meta in entry["metas"].as_array().into_iter().flatten() {
                let Some(id) = meta["phx_ref"].as_str() else { continue };
                let at = meta["active_at"].as_str().or(meta["online_at"].as_str());
                seen.entry(user.clone())
                    .or_default()
                    .insert(id.to_string(), at.and_then(parse_timestamp).unwrap_or(now));
            }
        }
    };
    if event == "presence_state" {
        seen.clear();
        add(seen, payload);
        return;
    }
    // A re-track arrives as a join plus a leave of the entry it replaces, in one message.
    add(seen, &payload["joins"]);
    for (user, entry) in payload["leaves"].as_object().into_iter().flatten() {
        if let Some(entries) = seen.get_mut(user) {
            for meta in entry["metas"].as_array().into_iter().flatten() {
                if let Some(id) = meta["phx_ref"].as_str() {
                    entries.remove(id);
                }
            }
            if entries.is_empty() {
                seen.remove(user);
            }
        }
    }
}

fn run_presence(
    user: String,
    active: mpsc::Receiver<()>,
    events: mpsc::Sender<HashMap<String, i64>>,
) {
    const TOPIC: &str = "realtime:online";
    let wait = |seconds: u64| {
        let until = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < until {
            if let Err(mpsc::RecvTimeoutError::Disconnected) =
                active.recv_timeout(Duration::from_millis(250))
            {
                return false;
            }
        }
        true
    };
    let mut backoff = 1;
    loop {
        let Some((mut socket, token)) = connect() else {
            if !wait(backoff) {
                return;
            }
            backoff = (backoff * 2).min(30);
            continue;
        };
        let mut sequence = 0;
        let mut healthy = send(
            &mut socket,
            TOPIC,
            "phx_join",
            json!({
                "config":{
                    "broadcast":{"self":false,"ack":false},
                    "presence":{"key":user,"enabled":true},
                    "postgres_changes":[],
                    "private":false,
                },
                "access_token":token,
            }),
            &mut sequence,
        );
        let join_ref = sequence.to_string();
        let mut joined = false;
        let mut heartbeat = Instant::now();
        let mut seen = HashMap::<String, HashMap<String, i64>>::new();
        let track = |socket: &mut Socket, sequence: &mut u64| {
            let stamp = format_timestamp(now_ms());
            send(
                socket,
                TOPIC,
                "presence",
                json!({"type":"presence","event":"track","payload":{"online_at":stamp,"active_at":stamp}}),
                sequence,
            )
        };
        while healthy {
            loop {
                match active.try_recv() {
                    Ok(()) if joined => healthy &= track(&mut socket, &mut sequence),
                    Ok(()) => {}
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            }
            if heartbeat.elapsed() > Duration::from_secs(25) {
                healthy &= send(&mut socket, "phoenix", "heartbeat", json!({}), &mut sequence);
                heartbeat = Instant::now();
            }
            match socket.read() {
                Ok(Message::Text(text)) => {
                    let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                    match value["event"].as_str() {
                        Some("phx_reply") if value["ref"] == join_ref.as_str() => {
                            if value["payload"]["status"] == "ok" {
                                joined = true;
                                backoff = 1;
                                healthy &= track(&mut socket, &mut sequence);
                            } else {
                                healthy = false;
                            }
                        }
                        Some(event @ ("presence_state" | "presence_diff")) => {
                            apply_presence(&mut seen, event, &value["payload"], now_ms());
                            let online = seen
                                .iter()
                                .filter_map(|(id, entries)| {
                                    Some((id.clone(), *entries.values().max()?))
                                })
                                .collect();
                            if events.send(online).is_err() {
                                return;
                            }
                        }
                        // For example an expired token. Reconnecting gets a fresh one.
                        Some("phx_error" | "phx_close") => healthy = false,
                        _ => {}
                    }
                }
                Ok(Message::Close(_)) => healthy = false,
                Ok(_) => {}
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => healthy = false,
            }
        }
        // Without a connection nobody's status is known, so don't keep showing stale dots.
        if events.send(HashMap::new()).is_err() || !wait(backoff) {
            return;
        }
        backoff = (backoff * 2).min(30);
    }
}

#[cfg(test)]
mod presence_tests {
    use super::*;

    #[test]
    fn timestamps_round_trip_and_read_other_formats() {
        let ms = 1_793_962_660_704;
        assert_eq!(parse_timestamp(&format_timestamp(ms)), Some(ms));
        assert_eq!(parse_timestamp("1970-01-01T00:00:01.500Z"), Some(1500));
        assert_eq!(parse_timestamp("2024-02-29T12:00:00Z"), Some(1_709_208_000_000));
        assert_eq!(format_timestamp(1_709_208_000_000), "2024-02-29T12:00:00.000Z");
        assert_eq!(parse_timestamp("nonsense"), None);
    }

    #[test]
    fn people_come_and_go_and_a_second_device_keeps_them_online() {
        let mut seen = HashMap::new();
        let meta = |id: &str| json!({"metas":[{"phx_ref":id,"active_at":"2024-02-29T12:00:00.000Z"}]});
        apply_presence(&mut seen, "presence_state", &json!({"a":meta("1")}), 0);
        assert!(seen.contains_key("a"));
        apply_presence(&mut seen, "presence_diff", &json!({"joins":{"b":meta("2")},"leaves":{}}), 0);
        assert!(seen.contains_key("b"));
        apply_presence(&mut seen, "presence_diff", &json!({"joins":{"a":meta("3")},"leaves":{"a":meta("1")}}), 0);
        assert!(seen.contains_key("a"));
        apply_presence(&mut seen, "presence_diff", &json!({"joins":{},"leaves":{"a":meta("3")}}), 0);
        assert!(!seen.contains_key("a"));
    }

    #[test]
    fn status_follows_main_nitros_rule() {
        assert_eq!(presence_status(None, 0), "offline");
        assert_eq!(presence_status(Some(0), 60_000), "online");
        assert_eq!(presence_status(Some(0), 11 * 60_000), "away");
    }
}
