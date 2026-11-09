//! Debug-only MCP that exposes the running app so tests don't need desktop automation.
//!
//! Start the app with `--dev-mcp`. It listens on 127.0.0.1 only, on a random port, and
//! requires a random bearer token; both are written to `dev-mcp.json` in the app's data
//! folder. `--mcp-bridge` runs a stdio bridge to that server for MCP clients.
//!
//! State never includes credentials, tokens or message bodies. Backend writes are refused
//! while a tool's input is being dispatched, except sends and reactions in the conversation
//! named by `--dev-mcp-write-chat=<username>`. Add tools here as tests need them.
use crate::{
    Preview,
    state::{AuthStatus, AuthTab, ShellView},
};
use gpui::{
    AnyWindowHandle, AsyncApp, ClipboardItem, Context, Focusable, Keystroke, Window, px, size,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicIsize, Ordering},
        mpsc,
    },
    time::Duration,
};

const MAX_BODY: usize = 256 * 1024;
static WINDOW_HANDLE: AtomicIsize = AtomicIsize::new(0);

pub fn enabled() -> bool {
    std::env::args().any(|arg| arg == "--dev-mcp")
}

struct Request {
    name: String,
    args: Value,
    reply: mpsc::Sender<Result<Value, String>>,
}

fn descriptor_path() -> std::path::PathBuf {
    crate::backend::data_dir().join("dev-mcp.json")
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
    })
}

fn tools() -> Value {
    json!({ "tools": [
        tool("nitro_get_state", "Redacted app state: view, session, conversations (counts only), inputs (lengths only), errors.", json!({}), &[]),
        tool("nitro_navigate", "Switch the visible view.", json!({"view":{"type":"string","enum":["chat","friends","settings","gallery"]}}), &["view"]),
        tool("nitro_auth_tab", "Switch the signed-out form.", json!({"tab":{"type":"string","enum":["login","signup"]}}), &["tab"]),
        tool("nitro_open_conversation", "Open a conversation by index. Reads only.", json!({"index":{"type":"integer","minimum":0}}), &["index"]),
        tool("nitro_set_voice_preview", "Show or hide the local in-call controls. Joins nothing.", json!({"on":{"type":"boolean"}}), &["on"]),
        tool("nitro_set_theme", "Apply a canonical Nitro theme ID or label for this run without saving it.", json!({"name":{"type":"string"}}), &["name"]),
        tool("nitro_show_banner", "Show the top banner locally. Nothing is sent to other users.", json!({"severity":{"type":"string","enum":["neutral","warning","danger"]},"message":{"type":"string"}}), &["severity","message"]),
        tool("nitro_voice", "Join or leave the real voice room. Opens the microphone. Only allowed in the chat named by --dev-mcp-write-chat.", json!({"action":{"type":"string","enum":["join","leave"]}}), &["action"]),
        tool("nitro_settings_tab", "Open a settings tab.", json!({"tab":{"type":"string","enum":["account","appearance","chat","voice","keybinds","soundboard","about"]}}), &["tab"]),
        tool("nitro_set_media", "Show or hide the media sidebar.", json!({"open":{"type":"boolean"}}), &["open"]),
        tool("nitro_sound_wheel", "Show the sound wheel without a call, point it, turn its page or close it. Plays nothing.", json!({"action":{"type":"string","enum":["open","point","page","close"]},"dx":{"type":"number"},"dy":{"type":"number"}}), &["action"]),
        tool("nitro_set_snow", "Start or stop the snow here only, until the next snapshot reads the real setting.", json!({"on":{"type":"boolean"}}), &["on"]),
        tool("nitro_dropdown", "Open a header dropdown, or close with none.", json!({"name":{"type":"string","enum":["none","switcher","soundboard","microphone","output"]}}), &["name"]),
        tool("nitro_focus_input", "Focus an input.", json!({"field":{"type":"string"}}), &["field"]),
        tool("nitro_set_input", "Replace an input's text. Refused for the password field.", json!({"field":{"type":"string"},"text":{"type":"string"}}), &["field","text"]),
        tool("nitro_press_key", "Dispatch a keystroke such as ctrl-v through the app's key handling. Backend writes are refused.", json!({"key":{"type":"string"}}), &["key"]),
        tool("nitro_attach_file", "Attach a local image to the composer, as the file dialog would. Sends nothing.", json!({"path":{"type":"string"}}), &["path"]),
        tool("nitro_click", "Dispatch a mouse click at window coordinates (logical pixels) inside the app. Backend writes are refused.", json!({"x":{"type":"number"},"y":{"type":"number"},"button":{"type":"string","enum":["left","right"]}}), &["x","y"]),
        tool("nitro_hover", "Dispatch a mouse move at window coordinates (logical pixels) to show hover states.", json!({"x":{"type":"number"},"y":{"type":"number"}}), &["x","y"]),
        tool("nitro_drag", "Drag inside this window, capture the held state, then release. Backend writes are refused.", json!({"x":{"type":"number"},"y":{"type":"number"},"end_x":{"type":"number"},"end_y":{"type":"number"}}), &["x","y","end_x","end_y"]),
        tool("nitro_wheel", "Dispatch a mouse wheel scroll at window coordinates (logical pixels). Negative notches scroll down.", json!({"x":{"type":"number"},"y":{"type":"number"},"notches":{"type":"integer","minimum":-50,"maximum":50}}), &["x","y","notches"]),
        tool("nitro_scroll_history", "Scroll chat history to the latest message or the top of what is loaded (which loads older messages).", json!({"to":{"type":"string","enum":["top","bottom"]}}), &["to"]),
        tool("nitro_resize", "Resize the window.", json!({"width":{"type":"number","minimum":480,"maximum":2400},"height":{"type":"number","minimum":320,"maximum":1600}}), &["width","height"]),
        tool("nitro_clipboard_set", "Overwrite the system clipboard with test text.", json!({"text":{"type":"string"}}), &["text"]),
        tool("nitro_screenshot", "Capture this app window only and save a PNG. Returns its path. Shows whatever is on screen, including chat.", json!({}), &[]),
        tool("nitro_test_overlay", "Show temporary synthetic overlay levels and capture them. Optional profile photo or red image calibration marker. No microphone, call or backend writes; refuses during a real call.", json!({"scale":{"type":"number","minimum":0.5,"maximum":1.5},"levels":{"type":"array","items":{"type":"number","minimum":0,"maximum":1},"minItems":1,"maxItems":16},"photo":{"type":"boolean"},"marker":{"type":"boolean"}}), &[]),
    ]})
}

// ---------------------------------------------------------------------------
// HTTP server (runs on its own thread; UI work is forwarded to the app)
// ---------------------------------------------------------------------------

fn read_request(stream: &mut TcpStream) -> Option<(String, String, String)> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break position;
        }
        if buffer.len() > MAX_BODY {
            return None;
        }
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = head.lines();
    let request_line = lines.next()?.to_string();
    let mut authorization = String::new();
    let mut length = 0usize;
    let mut has_origin = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "authorization" => authorization = value.trim().to_string(),
            "content-length" => length = value.trim().parse().unwrap_or(0),
            // Browsers always send Origin on POST; nothing legitimate here does.
            "origin" => has_origin = true,
            _ => {}
        }
    }
    if has_origin || length > MAX_BODY {
        return None;
    }
    let mut body = buffer[header_end + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Some((request_line, authorization, String::from_utf8_lossy(&body).to_string()))
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn handle_rpc(input: Value, requests: &mpsc::Sender<Request>) -> Option<Value> {
    let id = input.get("id").cloned()?;
    let method = input["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "nitro-hero-dev", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Dev-only control of the running Nitro Hero window. No credentials, tokens or message bodies in state; backend writes are refused.",
        }),
        "ping" => json!({}),
        "tools/list" => tools(),
        "tools/call" => {
            let name = input["params"]["name"].as_str().unwrap_or_default().to_string();
            let args = input["params"].get("arguments").cloned().unwrap_or(json!({}));
            let outcome = if name == "nitro_screenshot" {
                screenshot()
            } else {
                let (reply, wait) = mpsc::channel();
                requests
                    .send(Request { name, args, reply })
                    .map_err(|_| "App closed".to_string())
                    .and_then(|_| {
                        wait.recv_timeout(Duration::from_secs(10))
                            .map_err(|_| "The app didn't respond in time".to_string())
                    })
                    .and_then(|outcome| outcome)
            };
            match outcome {
                Ok(data) => json!({
                    "content": [{ "type": "text", "text": data.to_string() }],
                    "structuredContent": data,
                    "isError": false,
                }),
                Err(message) => json!({
                    "content": [{ "type": "text", "text": message }],
                    "isError": true,
                }),
            }
        }
        _ => {
            return Some(json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": "Unknown method" },
            }));
        }
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn serve(listener: TcpListener, token: String, requests: mpsc::Sender<Request>) {
    let expected = format!("Bearer {token}");
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let requests = requests.clone();
        let expected = expected.clone();
        std::thread::spawn(move || {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
            let Some((line, authorization, body)) = read_request(&mut stream) else {
                respond(&mut stream, "400 Bad Request", "{}");
                return;
            };
            if authorization != expected {
                respond(&mut stream, "401 Unauthorized", "{}");
                return;
            }
            if !line.starts_with("POST /mcp ") {
                respond(&mut stream, "404 Not Found", "{}");
                return;
            }
            let Ok(input) = serde_json::from_str::<Value>(&body) else {
                respond(&mut stream, "400 Bad Request", "{}");
                return;
            };
            match handle_rpc(input, &requests) {
                Some(reply) => respond(&mut stream, "200 OK", &reply.to_string()),
                None => respond(&mut stream, "202 Accepted", ""),
            }
        });
    }
}

/// Starts the server and the UI-thread dispatcher for this window.
pub fn attach(window: &mut Window, cx: &mut Context<Preview>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if let Ok(handle) = HasWindowHandle::window_handle(&*window)
        && let RawWindowHandle::Win32(raw) = handle.as_raw()
    {
        WINDOW_HANDLE.store(raw.hwnd.get(), Ordering::Relaxed);
    }
    let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else {
        return;
    };
    let Ok(address) = listener.local_addr() else {
        return;
    };
    let token = uuid::Uuid::new_v4().simple().to_string();
    let descriptor = json!({
        "url": format!("http://127.0.0.1:{}/mcp", address.port()),
        "token": token,
    });
    if std::fs::write(descriptor_path(), descriptor.to_string()).is_err() {
        return;
    }
    let (sender, receiver) = mpsc::channel::<Request>();
    std::thread::spawn(move || serve(listener, token, sender));

    let handle = Window::window_handle(window);
    cx.spawn(async move |view, cx| {
        loop {
            gpui::Timer::after(Duration::from_millis(25)).await;
            while let Ok(request) = receiver.try_recv() {
                let Some(view) = view.upgrade() else { return };
                let outcome = if request.name == "nitro_test_overlay" {
                    overlay_probe(&view, cx, &request.args).await
                } else if matches!(request.name.as_str(), "nitro_click" | "nitro_hover" | "nitro_wheel" | "nitro_drag") {
                    pointer_tool(&request.name, &request.args, &view, handle, cx).await
                } else {
                    dispatch(&request.name, &request.args, &view, handle, cx)
                };
                let _ = request.reply.send(outcome);
            }
        }
    })
    .detach();
}

fn dispatch(
    name: &str,
    args: &Value,
    view: &gpui::Entity<Preview>,
    handle: AnyWindowHandle,
    cx: &mut AsyncApp,
) -> Result<Value, String> {
    // Key dispatch re-enters Preview's listeners, so it runs with no Preview borrow held.
    if name == "nitro_press_key" {
        let _ = view.update(cx, |this, _| this.dev_block_writes = true);
        let result = handle
            .update(cx, |_, window, cx| {
                let key = args["key"].as_str().ok_or("key is required")?;
                let keystroke = Keystroke::parse(key).map_err(|err| err.to_string())?;
                let handled = window.dispatch_keystroke(keystroke, cx);
                Ok(json!({ "handled": handled }))
            })
            .map_err(|_| "Window closed".to_string())
            .and_then(|outcome: Result<Value, String>| outcome);
        let _ = view.update(cx, |this, _| this.dev_block_writes = false);
        return result;
    }
    handle
        .update(cx, |_, window, cx| {
            view.update(cx, |this, cx| this.dev_tool(name, args, window, cx))
        })
        .map_err(|_| "Window closed".to_string())
        .and_then(|outcome| outcome)
}

/// Clicks and hovers go through the window's message queue, so they exercise GPUI's real
/// input path without moving the user's cursor.
async fn pointer_tool(
    name: &str,
    args: &Value,
    view: &gpui::Entity<Preview>,
    handle: AnyWindowHandle,
    cx: &mut AsyncApp,
) -> Result<Value, String> {
    let (Some(x), Some(y)) = (args["x"].as_f64(), args["y"].as_f64()) else {
        return Err("x and y are required".into());
    };
    let scale = handle
        .update(cx, |_, window, _| window.scale_factor())
        .map_err(|_| "Window closed".to_string())? as f64;
    let (x, y) = ((x * scale) as i32, (y * scale) as i32);
    let hwnd = WINDOW_HANDLE.load(Ordering::Relaxed);
    if name == "nitro_drag" {
        let end_x = args["end_x"].as_f64().ok_or("end_x is required")? * scale;
        let end_y = args["end_y"].as_f64().ok_or("end_y is required")? * scale;
        let _ = view.update(cx, |this, _| this.dev_block_writes = true);
        capture::post_mouse(hwnd, x, y, 0x0200, 0);
        capture::post_mouse(hwnd, x, y, 0x0201, 1);
        for step in 1..=12 {
            gpui::Timer::after(Duration::from_millis(16)).await;
            let t = step as f64 / 12.;
            capture::post_mouse(hwnd, (x as f64 + (end_x - x as f64) * t) as i32,
                (y as f64 + (end_y - y as f64) * t) as i32, 0x0200, 1);
        }
        gpui::Timer::after(Duration::from_millis(60)).await;
        let held = screenshot();
        capture::post_mouse(hwnd, end_x as i32, end_y as i32, 0x0202, 0);
        gpui::Timer::after(Duration::from_millis(400)).await;
        let _ = view.update(cx, |this, _| this.dev_block_writes = false);
        return held;
    }
    if name == "nitro_wheel" {
        let notches = args["notches"].as_i64().ok_or("notches is required")? as i32;
        capture::post_wheel(hwnd, x, y, notches);
        gpui::Timer::after(Duration::from_millis(120)).await;
        return Ok(json!({}));
    }
    let _ = view.update(cx, |this, _| this.dev_block_writes = true);
    capture::post_mouse(hwnd, x, y, 0x0200, 0); // WM_MOUSEMOVE
    gpui::Timer::after(Duration::from_millis(60)).await;
    if name == "nitro_click" {
        let (down, up, held) = if args["button"].as_str() == Some("right") {
            (0x0204, 0x0205, 2) // WM_RBUTTONDOWN / UP
        } else {
            (0x0201, 0x0202, 1) // WM_LBUTTONDOWN / UP
        };
        capture::post_mouse(hwnd, x, y, down, held);
        gpui::Timer::after(Duration::from_millis(40)).await;
        capture::post_mouse(hwnd, x, y, up, 0);
        gpui::Timer::after(Duration::from_millis(120)).await;
    }
    let _ = view.update(cx, |this, _| this.dev_block_writes = false);
    Ok(json!({}))
}

impl Preview {
    fn dev_input(&self, field: &str) -> Option<gpui::Entity<gpui_component::input::InputState>> {
        Some(match field {
            "email" => self.auth_email_input.clone(),
            "password" => self.auth_password_input.clone(),
            "username" => self.auth_username_input.clone(),
            "display_name" => self.auth_display_name_input.clone(),
            "onboarding_name" => self.onboarding_name_input.clone(),
            "add_friend" => self.add_friend_input.clone(),
            "composer" => self.chat_input.clone(),
            _ => return None,
        })
    }

    fn dev_state(&self, window: &Window, cx: &gpui::App) -> Value {
        let inputs: Value = [
            "email",
            "password",
            "username",
            "display_name",
            "onboarding_name",
            "add_friend",
            "composer",
        ]
        .iter()
        .map(|field| {
            let entity = self.dev_input(field).expect("known field");
            let state = entity.read(cx);
            (
                field.to_string(),
                json!({
                    "length": state.value().chars().count(),
                    "focused": state.focus_handle(cx).is_focused(window),
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into();
        let conversations: Vec<Value> = self
            .state
            .conversations
            .iter()
            .enumerate()
            .map(|(index, conversation)| {
                json!({
                    "index": index,
                    "active": index == self.state.active_partner,
                    "username": conversation.username,
                    "unread": conversation.unread_count,
                    "loaded": conversation.loaded,
                    "messages": conversation.messages.len(),
                    "pending": conversation.messages.iter().filter(|m| m.pending).count(),
                    "from_me": conversation.messages.iter().filter(|m| m.is_self).count(),
                    "with_media": conversation.shared_media.len(),
                    "bursts": crate::backend::burst_groups(&conversation.messages)
                        .iter()
                        .filter(|(_, length)| *length > 1)
                        .map(|(_, length)| *length)
                        .collect::<Vec<_>>(),
                    "has_more": conversation.has_more,
                    "has_photo": conversation.photo.path.is_some(),
                    "has_decoration": conversation.photo.decoration.is_some(),
                    "voice_participants": conversation.voice_participants.len(),
                })
            })
            .collect();
        let bounds = window.bounds();
        json!({
            "auth_status": match self.state.auth_status {
                AuthStatus::SignedOut => "signed_out",
                AuthStatus::Onboarding => "onboarding",
                AuthStatus::SignedIn => "signed_in",
            },
            "auth_tab": match self.auth_tab {
                AuthTab::Login => "login",
                AuthTab::Signup => "signup",
                AuthTab::VerifyEmail(_) => "verify_email",
            },
            "auth_busy": self.auth_busy,
            "auth_error": self.auth_error,
            "sync_error": self.sync_error,
            "friends_error": self.friends_error,
            "has_user": self.user.is_some(),
            "view": if self.view_mode == 1 { "gallery" } else {
                match self.shell_view {
                    ShellView::Chat => "chat",
                    ShellView::Friends => "friends",
                    ShellView::Settings => "settings",
                }
            },
            "theme": self.state.preferences.theme.label(),
            "media_sidebar_open": self.media_sidebar_open,
            "call_connected": self.call_connected,
            "friend_requests": self.state.friend_requests.len(),
            "soundboard": {
                "clips": self.soundboard_clips.len(),
                "status": match &self.soundboard_status {
                    crate::SoundboardStatus::NotLoaded => "not_loaded".to_string(),
                    crate::SoundboardStatus::Loading => "loading".to_string(),
                    crate::SoundboardStatus::Loaded => "loaded".to_string(),
                    crate::SoundboardStatus::Failed(message) => format!("failed: {message}"),
                },
            },
            "media": {
                "ready": self.media_files.values().filter(|f| matches!(f, crate::MediaFile::Ready { .. })).count(),
                "loading": self.media_files.values().filter(|f| matches!(f, crate::MediaFile::Loading)).count(),
                "failed": self.media_files.values().filter(|f| matches!(f, crate::MediaFile::Failed)).count(),
            },
            "loading_older": self.loading_older,
            "voice": {
                "status": self.voice_state.status,
                "receiving": self.voice_state.receiving,
                "local_meter_status": self.voice_state.local_meter_status,
                "participants": self.voice_state.participants.len(),
                "remote_muted": self.voice_state.remote_muted,
                "muted": self.voice_state.muted,
                "levels": self.voice_state.levels.values().collect::<Vec<_>>(),
                "error": self.voice_state.error,
            },
            "screen": {
                "share": format!("{:?}", self.voice_state.screen.share),
                "watch": format!("{:?}", self.voice_state.screen.watch),
                "partner_sharing": self.voice_state.screen.partner_sharing,
                "sources": self.voice_state.screen.sources.len(),
                "preview_frame": self.share_frame.is_some(),
            },
            "typing": { "joined": self.typing.is_some(), "self": self.self_typing_until.is_some(), "partner": self.partner_typing_until.is_some() },
            "hud_open": self.hud.is_some(),
            "playing_clip": self.playing_sound_id.is_some(),
            "playing_video": self.playing_video.is_some(),
            "banner": self.banner.as_ref().map(|banner| banner.notice_id.is_some()),
            "decorations": self.decoration_catalog.len(),
            "network": {
                "delivery_links": crate::backend::DELIVERY_LINKS.load(Ordering::Relaxed),
                "media_downloads": crate::backend::MEDIA_DOWNLOADS.load(Ordering::Relaxed),
                "profile_art_downloads": crate::backend::PROFILE_ART_DOWNLOADS.load(Ordering::Relaxed),
                "downloaded_bytes": crate::backend::DOWNLOADED_BYTES.load(Ordering::Relaxed),
                "sounds_played": crate::audio::SOUNDS_PLAYED.load(Ordering::Relaxed),
            },
            "conversations": conversations,
            "inputs": inputs,
            "window": {
                "width": f32::from(bounds.size.width),
                "height": f32::from(bounds.size.height),
                "scale": window.scale_factor(),
                "active": window.is_window_active(),
            },
        })
    }

    fn dev_tool(
        &mut self,
        name: &str,
        args: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let string = |key: &str| args[key].as_str().ok_or(format!("{key} is required"));
        match name {
            "nitro_get_state" => {}
            "nitro_navigate" => {
                let view = string("view")?;
                self.view_mode = usize::from(view == "gallery");
                self.shell_view = match view {
                    "friends" => ShellView::Friends,
                    "settings" => ShellView::Settings,
                    _ => ShellView::Chat,
                };
            }
            "nitro_auth_tab" => {
                self.auth_tab = if string("tab")? == "signup" { AuthTab::Signup } else { AuthTab::Login };
            }
            "nitro_open_conversation" => {
                let index = args["index"].as_u64().ok_or("index is required")? as usize;
                if index >= self.state.conversations.len() {
                    return Err("No conversation at that index".into());
                }
                self.dev_block_writes = true;
                self.switch_partner(index, window, cx);
                self.dev_block_writes = false;
            }
            "nitro_set_media" => {
                self.media_sidebar_open = args["open"].as_bool().ok_or("open is required")?;
            }
            "nitro_sound_wheel" => match args["action"].as_str().unwrap_or("") {
                "open" => {
                    self.open_wheel(cx);
                    if let Some(wheel) = self.sound_wheel.as_mut() {
                        wheel.stay_open = true;
                    }
                }
                "point" => self.wheel_aim_by(args["dx"].as_f64().unwrap_or(0.) as f32, args["dy"].as_f64().unwrap_or(0.) as f32),
                "page" => self.wheel_turn_page(true),
                _ => self.sound_wheel = None,
            },
            "nitro_set_snow" => {
                self.set_snow(args["on"].as_bool().ok_or("on is required")?, cx);
            }
            "nitro_dropdown" => {
                use crate::state::HeaderDropdown::*;
                self.open_dropdown = match string("name")? {
                    "switcher" => Some(Switcher),
                    "soundboard" => {
                        self.load_soundboard(cx);
                        Some(Soundboard)
                    }
                    "microphone" => Some(Microphone),
                    "output" => Some(Output),
                    _ => None,
                };
            }
            "nitro_focus_input" => {
                let input = self.dev_input(string("field")?).ok_or("Unknown field")?;
                input.update(cx, |state, cx| state.focus(window, cx));
            }
            "nitro_set_input" => {
                let field = string("field")?;
                if field == "password" {
                    return Err("The password field can't be set through the MCP".into());
                }
                let text = string("text")?.to_string();
                let input = self.dev_input(field).ok_or("Unknown field")?;
                input.update(cx, |state, cx| state.set_value(text, window, cx));
            }
            "nitro_scroll_history" => {
                if string("to")? == "top" {
                    self.chat_scroll.set_offset(gpui::point(px(0.), px(0.)));
                } else {
                    self.chat_scroll.scroll_to_bottom();
                }
            }
            "nitro_set_theme" => {
                let name = string("name")?;
                self.state.preferences.theme = crate::state::AppTheme::ALL
                    .into_iter()
                    .find(|theme| theme.label() == name || theme.id() == name)
                    .ok_or("Unknown theme")?;
            }
            "nitro_show_banner" => {
                let severity = crate::components::BannerSeverity::parse(string("severity")?);
                self.show_banner(severity, string("message")?, None);
            }
            "nitro_voice" => {
                let allowed = self.dev_write_chat.as_ref().is_some_and(|chat| {
                    self.state
                        .conversations
                        .get(self.state.active_partner)
                        .is_some_and(|conversation| &conversation.username == chat)
                });
                if !allowed {
                    return Err("Voice is only allowed in the chat named by --dev-mcp-write-chat".into());
                }
                if string("action")? == "join" {
                    self.join_voice(cx);
                } else {
                    self.leave_voice(cx);
                }
            }
            "nitro_settings_tab" => {
                use crate::state::SettingsTab::*;
                self.shell_view = ShellView::Settings;
                self.view_mode = 0;
                self.settings_tab = match string("tab")? {
                    "appearance" => Appearance,
                    "chat" => Chat,
                    "voice" => Voice,
                    "keybinds" => Keybinds,
                    "soundboard" => Soundboard,
                    "about" => About,
                    _ => General,
                };
            }
            "nitro_set_voice_preview" => {
                self.call_connected = args["on"].as_bool().ok_or("on is required")?;
            }
            "nitro_resize" => {
                let width = args["width"].as_f64().ok_or("width is required")?;
                let height = args["height"].as_f64().ok_or("height is required")?;
                window.resize(size(px(width as f32), px(height as f32)));
            }
            "nitro_attach_file" => {
                self.attach_file(string("path")?.into(), cx);
            }
            "nitro_clipboard_set" => {
                cx.write_to_clipboard(ClipboardItem::new_string(string("text")?.to_string()));
            }
            _ => return Err(format!("Unknown tool {name}")),
        }
        cx.notify();
        Ok(self.dev_state(window, cx))
    }
}

// ---------------------------------------------------------------------------
// Window capture
// ---------------------------------------------------------------------------

async fn overlay_probe(view: &gpui::Entity<Preview>, cx: &mut AsyncApp, args: &Value) -> Result<Value, String> {
    if view.update(cx, |this, _| this.call_connected).unwrap_or(true) {
        return Err("Leave the real call before running a synthetic overlay test".into());
    }
    let scale = (args["scale"].as_f64().unwrap_or(0.7) as f32).clamp(0.5, 1.5);
    let levels = args["levels"].as_array().map(|values| values.iter().take(16)
        .filter_map(|value| value.as_f64()).map(|value| (value as f32).clamp(0., 1.)).collect::<Vec<_>>());
    let art = if args["marker"].as_bool() == Some(true) {
        let path = std::env::temp_dir().join("nitro-overlay-calibration.png");
        image::RgbaImage::from_pixel(512, 512, image::Rgba([220, 32, 32, 255]))
            .save(&path).map_err(|error| error.to_string())?;
        crate::components::AvatarImage { path: Some(path), decoration: None }
    } else if args["photo"].as_bool() == Some(true) {
        view.update(cx, |this, _| this.state.profile.photo.clone()).map_err(|_| "App closed")?
    } else { Default::default() };
    let steps: Vec<(String, f32)> = levels.map(|levels| levels.into_iter().enumerate()
        .map(|(i, level)| (format!("frame-{i}"), level)).collect())
        .unwrap_or_else(|| vec![("quiet".into(), 0.), ("loud".into(), 1.)]);
    // A test pattern, standing in for a live screen.
    let stream = (args["stream"].as_bool() == Some(true)).then(|| {
        let pixels = image::RgbaImage::from_fn(640, 360, |x, y| {
            image::Rgba([(x / 3) as u8, (y / 2) as u8, 150, 255])
        });
        crate::components::OverlayStream {
            owner: "overlay-test".into(),
            frame: std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)])),
        }
    });
    let mut overlay = None;
    let result = async {
        let mut frames = Vec::new();
        for (name, level) in steps {
            let person = crate::components::OverlayPerson {
                id: "overlay-test".into(), name: "Meter test".into(), initials: "M".into(),
                color: crate::components::MATERIAL_DARK, art: art.clone(), muted: false, level,
            };
            cx.update(|cx| crate::hud::sync(&mut overlay, Some(vec![person]), scale, false, stream.clone(), cx))
                .map_err(|_| "App closed".to_string())?;
            gpui::Timer::after(Duration::from_millis(450)).await;
            let hwnd = overlay.as_ref().ok_or("Overlay failed to open")?.update(cx, |_, window, _| {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                match HasWindowHandle::window_handle(window).ok()?.as_raw() {
                    RawWindowHandle::Win32(raw) => Some(raw.hwnd.get()),
                    _ => None,
                }
            }).map_err(|_| "Overlay closed".to_string())?.ok_or("No overlay handle")?;
            let mut frame = save_screenshot(hwnd, &format!("nitro-overlay-{scale}-{name}.png"))?;
            frame["level"] = json!(level);
            frames.push(frame);
        }
        Ok(json!({"scale":scale,"frames":frames,"microphone":false,"call":false}))
    }.await;
    let _ = cx.update(|cx| crate::hud::sync(&mut overlay, None, scale, false, None, cx));
    result
}

fn screenshot() -> Result<Value, String> {
    let hwnd = WINDOW_HANDLE.load(Ordering::Relaxed);
    save_screenshot(hwnd, "nitro-hero-window.png")
}

fn save_screenshot(hwnd: isize, name: &str) -> Result<Value, String> {
    if hwnd == 0 {
        return Err("Window handle isn't available".into());
    }
    let (width, height, mut pixels) = capture::window(hwnd).ok_or("Capture failed")?;
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    let path = std::env::temp_dir().join(name);
    image::save_buffer(&path, &pixels, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|err| err.to_string())?;
    Ok(json!({ "path": path, "width": width, "height": height }))
}

#[cfg(windows)]
mod capture {
    use std::ffi::c_void;

    pub fn post_mouse(hwnd: isize, x: i32, y: i32, message: u32, wparam: usize) {
        let lparam = ((y as u16 as isize) << 16) | (x as u16 as isize);
        unsafe {
            PostMessageW(hwnd, message, wparam, lparam);
        }
    }

    /// WM_MOUSEWHEEL takes screen coordinates, unlike the other mouse messages.
    pub fn post_wheel(hwnd: isize, x: i32, y: i32, notches: i32) {
        let mut origin = Point { x, y };
        unsafe {
            ClientToScreen(hwnd, &mut origin);
            let delta = (notches * 120) as i16 as u16 as usize;
            let lparam = ((origin.y as u16 as isize) << 16) | (origin.x as u16 as isize);
            PostMessageW(hwnd, 0x020A, delta << 16, lparam);
        }
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    struct BitmapInfo {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        image_size: u32,
        x_ppm: i32,
        y_ppm: i32,
        used: u32,
        important: u32,
        colors: [u32; 3],
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
        fn GetDC(hwnd: isize) -> *mut c_void;
        fn ReleaseDC(hwnd: isize, dc: *mut c_void) -> i32;
        fn PrintWindow(hwnd: isize, dc: *mut c_void, flags: u32) -> i32;
        fn PostMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> i32;
        fn ClientToScreen(hwnd: isize, point: *mut Point) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(dc: *mut c_void) -> *mut c_void;
        fn CreateCompatibleBitmap(dc: *mut c_void, width: i32, height: i32) -> *mut c_void;
        fn SelectObject(dc: *mut c_void, object: *mut c_void) -> *mut c_void;
        fn DeleteObject(object: *mut c_void) -> i32;
        fn DeleteDC(dc: *mut c_void) -> i32;
        fn GetDIBits(
            dc: *mut c_void,
            bitmap: *mut c_void,
            start: u32,
            lines: u32,
            bits: *mut c_void,
            info: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
    }

    pub fn window(hwnd: isize) -> Option<(u32, u32, Vec<u8>)> {
        const PW_RENDERFULLCONTENT: u32 = 2;
        unsafe {
            let mut rect = Rect::default();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return None;
            }
            let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
            if width <= 0 || height <= 0 {
                return None;
            }
            let screen = GetDC(hwnd);
            let memory = CreateCompatibleDC(screen);
            let bitmap = CreateCompatibleBitmap(screen, width, height);
            let previous = SelectObject(memory, bitmap);
            let printed = PrintWindow(hwnd, memory, PW_RENDERFULLCONTENT) != 0;
            let mut pixels = vec![0u8; width as usize * height as usize * 4];
            let mut info = BitmapInfo {
                size: 40,
                width,
                height: -height,
                planes: 1,
                bit_count: 32,
                compression: 0,
                image_size: 0,
                x_ppm: 0,
                y_ppm: 0,
                used: 0,
                important: 0,
                colors: [0; 3],
            };
            let copied = GetDIBits(
                memory,
                bitmap,
                0,
                height as u32,
                pixels.as_mut_ptr().cast(),
                &mut info,
                0,
            );
            SelectObject(memory, previous);
            DeleteObject(bitmap);
            DeleteDC(memory);
            ReleaseDC(hwnd, screen);
            (printed && copied != 0).then_some((width as u32, height as u32, pixels))
        }
    }
}

#[cfg(not(windows))]
mod capture {
    pub fn window(_: isize) -> Option<(u32, u32, Vec<u8>)> {
        None
    }
    pub fn post_mouse(_: isize, _: i32, _: i32, _: u32, _: usize) {}
    pub fn post_wheel(_: isize, _: i32, _: i32, _: i32) {}
}

// ---------------------------------------------------------------------------
// Stdio bridge for MCP clients
// ---------------------------------------------------------------------------

/// Forwards stdin JSON-RPC lines to the running app's server and prints the replies.
pub fn bridge() {
    let post = |line: &str| -> Option<String> {
        let text = std::fs::read_to_string(descriptor_path()).ok()?;
        let descriptor: Value = serde_json::from_str(&text).ok()?;
        let url = descriptor["url"].as_str()?;
        let token = descriptor["token"].as_str()?;
        ureq::post(url)
            .set("Authorization", &format!("Bearer {token}"))
            .set("Content-Type", "application/json")
            .send_string(line)
            .ok()?
            .into_string()
            .ok()
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let id = serde_json::from_str::<Value>(&line)
            .ok()
            .and_then(|value| value.get("id").cloned());
        let reply = post(&line).filter(|reply| !reply.is_empty());
        let output = match (reply, id) {
            (Some(reply), _) => reply,
            (None, Some(id)) => json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32000, "message": "Nitro Hero isn't running with --dev-mcp" },
            })
            .to_string(),
            (None, None) => continue,
        };
        let _ = writeln!(stdout, "{output}");
        let _ = stdout.flush();
    }
}
