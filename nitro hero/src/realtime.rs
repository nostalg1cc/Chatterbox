//! Typing signals, over the same public Realtime channel main Nitro uses.
use serde_json::{Value, json};
use std::{
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
