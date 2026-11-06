//! Blocking Supabase client for Nitro Hero. Call only from background tasks.
//!
//! The session lives in memory; only the refresh token is written to disk, encrypted
//! for the current Windows user with DPAPI. Row Level Security is enforced by sending
//! the user's own access token on every request.
use crate::components::{AvatarImage, NameEffect};
use crate::state::{
    ChatAttachment, ChatMessage, FriendRequest, MediaItem, PartnerConversation, ReactionChip,
    SoundClip, UserProfile, initials_for,
};
use chrono::{DateTime, Local};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const PAGE_SIZE: usize = 50;

#[derive(Debug, Clone)]
pub enum ApiError {
    /// The server answered and said no.
    Rejected { status: u16, message: String },
    /// The server could not be reached. Never treat this as signed in.
    Network(#[allow(dead_code)] String),
    /// There is no usable session any more.
    SignedOut,
    /// `.env` has no Supabase settings.
    NotConfigured,
}

impl ApiError {
    pub fn message(&self) -> String {
        match self {
            ApiError::Rejected { message, .. } => message.clone(),
            ApiError::Network(_) => "Can't reach Nitro. Check your connection and try again.".into(),
            ApiError::SignedOut => "Your session ended. Log in again.".into(),
            ApiError::NotConfigured => "Supabase isn't configured (missing .env).".into(),
        }
    }
}

type ApiResult<T> = Result<T, ApiError>;

struct Config {
    url: String,
    key: String,
}

fn config() -> ApiResult<&'static Config> {
    static CONFIG: OnceLock<Option<Config>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            if let (Some(url), Some(key)) = (option_env!("NITRO_SUPABASE_URL"), option_env!("NITRO_SUPABASE_KEY")) {
                return Some(Config { url: url.trim_end_matches('/').to_string(), key: key.to_string() });
            }
            let manifest = env!("CARGO_MANIFEST_DIR");
            for candidate in [
                format!("{manifest}/../.env"),
                format!("{manifest}/.env"),
                ".env".to_string(),
                "../.env".to_string(),
            ] {
                let Ok(text) = std::fs::read_to_string(candidate) else {
                    continue;
                };
                let value = |name: &str| {
                    text.lines().find_map(|line| {
                        line.trim()
                            .strip_prefix(name)
                            .and_then(|rest| rest.strip_prefix('='))
                            .map(|rest| rest.trim().trim_matches('"').to_string())
                            .filter(|rest| !rest.is_empty())
                    })
                };
                if let (Some(url), Some(key)) =
                    (value("VITE_SUPABASE_URL"), value("VITE_SUPABASE_ANON_KEY"))
                {
                    return Some(Config {
                        url: url.trim_end_matches('/').to_string(),
                        key,
                    });
                }
            }
            None
        })
        .as_ref()
        .ok_or(ApiError::NotConfigured)
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(6))
            .timeout(Duration::from_secs(20))
            .build()
    })
}

fn server_message(body: &str) -> String {
    let parsed: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    ["msg", "error_description", "message", "error"]
        .iter()
        .find_map(|key| parsed.get(*key).and_then(Value::as_str))
        .unwrap_or("The server rejected that request.")
        .to_string()
}

fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<String>,
) -> ApiResult<String> {
    let mut call = agent().request(method, url);
    for (name, value) in headers {
        call = call.set(name, value);
    }
    let response = match body {
        Some(body) => call.set("Content-Type", "application/json").send_string(&body),
        None => call.call(),
    };
    match response {
        Ok(response) => response
            .into_string()
            .map_err(|err| ApiError::Network(err.to_string())),
        Err(ureq::Error::Status(status, response)) => {
            let text = response.into_string().unwrap_or_default();
            Err(ApiError::Rejected {
                status,
                message: server_message(&text),
            })
        }
        Err(err) => Err(ApiError::Network(err.kind().to_string())),
    }
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}

/// What the UI is allowed to know about a session. Never carries tokens.
#[derive(Clone, Debug)]
pub struct SignedInUser {
    pub id: String,
    pub email: String,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
static REFRESH: Mutex<()> = Mutex::new(());

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join("Nitro Hero");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn session_file() -> PathBuf {
    data_dir().join("session.bin")
}

fn store_refresh_token(token: &str) {
    if let Some(sealed) = dpapi::protect(token.as_bytes()) {
        let path = session_file();
        let temp = path.with_extension("tmp");
        if std::fs::write(&temp, sealed).is_ok() {
            let _ = std::fs::rename(temp, path);
        }
    }
}

fn clear_session() {
    if let Ok(mut session) = SESSION.lock() {
        *session = None;
    }
    let _ = std::fs::remove_file(session_file());
}

fn adopt_session(json: &Value) -> Option<SignedInUser> {
    let access_token = json["access_token"].as_str()?.to_string();
    let refresh_token = json["refresh_token"].as_str()?.to_string();
    let user_id = json["user"]["id"].as_str()?.to_string();
    let email = json["user"]["email"].as_str().unwrap_or_default().to_string();
    let expires_at = json["expires_at"]
        .as_u64()
        .unwrap_or_else(|| now_secs() + json["expires_in"].as_u64().unwrap_or(3600));
    store_refresh_token(&refresh_token);
    if let Ok(mut session) = SESSION.lock() {
        *session = Some(Session {
            access_token,
            refresh_token,
            expires_at,
        });
    }
    Some(SignedInUser { id: user_id, email })
}

fn auth_post(path: &str, payload: Value) -> ApiResult<Value> {
    let config = config()?;
    let text = request(
        "POST",
        &format!("{}/auth/v1/{path}", config.url),
        &[("apikey", &config.key)],
        Some(payload.to_string()),
    )?;
    serde_json::from_str(&text).map_err(|_| ApiError::Network("bad auth response".into()))
}

pub fn sign_in(email: &str, password: &str) -> ApiResult<SignedInUser> {
    let json = auth_post(
        "token?grant_type=password",
        json!({ "email": email.trim(), "password": password }),
    )
    .map_err(|err| match err {
        ApiError::Rejected { status, message } if message.contains("Invalid login credentials") => {
            ApiError::Rejected {
                status,
                message: "Wrong email or password.".into(),
            }
        }
        other => other,
    })?;
    adopt_session(&json).ok_or(ApiError::Network("incomplete auth response".into()))
}

/// `Ok(None)` means the account exists and is waiting for email confirmation.
pub fn sign_up(
    email: &str,
    password: &str,
    username: &str,
    display_name: &str,
) -> ApiResult<Option<SignedInUser>> {
    let json = auth_post(
        "signup",
        json!({
            "email": email.trim(),
            "password": password,
            "data": { "username": username, "display_name": display_name },
        }),
    )
    .map_err(|err| match err {
        ApiError::Rejected { status, message } if message.contains("Database error") => {
            ApiError::Rejected {
                status,
                message: "That username or email is already taken.".into(),
            }
        }
        other => other,
    })?;
    Ok(adopt_session(&json))
}

fn refresh_with(token: &str) -> ApiResult<SignedInUser> {
    match auth_post(
        "token?grant_type=refresh_token",
        json!({ "refresh_token": token }),
    ) {
        Ok(json) => adopt_session(&json).ok_or(ApiError::Network("incomplete auth response".into())),
        Err(ApiError::Rejected { .. }) => {
            clear_session();
            Err(ApiError::SignedOut)
        }
        Err(other) => Err(other),
    }
}

/// True when a saved session exists, so the UI can show a restoring state.
pub fn has_saved_session() -> bool {
    session_file().exists()
}

fn legacy_session_marker() -> PathBuf {
    data_dir().join("legacy-session-tried")
}

/// True if starting up should try to restore a sign-in: this app's own, or once, the old app's.
pub fn may_restore_session() -> bool {
    has_saved_session() || (!legacy_session_marker().exists() && crate::migrate::has_legacy_storage())
}

/// Right after updating from the old app, carry on as the same person. A token the server
/// turns down, or none at all, means a normal sign-in from then on. A network failure tries
/// again next launch, because the token is still good.
fn restore_legacy_session() -> ApiResult<Option<SignedInUser>> {
    let settled = || {
        let _ = std::fs::write(legacy_session_marker(), b"");
    };
    let Some(token) = crate::migrate::legacy_refresh_token() else {
        settled();
        return Ok(None);
    };
    match refresh_with(&token) {
        Ok(user) => {
            settled();
            Ok(Some(user))
        }
        Err(ApiError::SignedOut) => {
            settled();
            Ok(None)
        }
        Err(_) => Ok(None),
    }
}

/// `Ok(None)` when nothing is saved. A network failure keeps the saved token.
pub fn restore_session() -> ApiResult<Option<SignedInUser>> {
    let Ok(sealed) = std::fs::read(session_file()) else {
        if legacy_session_marker().exists() {
            return Ok(None);
        }
        return restore_legacy_session();
    };
    let Some(token) = dpapi::unprotect(&sealed).and_then(|raw| String::from_utf8(raw).ok()) else {
        clear_session();
        return Ok(None);
    };
    refresh_with(&token).map(Some)
}

/// What the voice worker needs to reach Supabase as this user. It stays in memory, and the
/// token is replaced as it refreshes.
pub fn voice_credentials() -> ApiResult<(String, String, String)> {
    let config = config()?;
    Ok((config.url.clone(), config.key.clone(), access_token(false)?))
}

pub fn sign_out() {
    let token = SESSION
        .lock()
        .ok()
        .and_then(|session| session.as_ref().map(|s| s.access_token.clone()));
    if let (Some(token), Ok(config)) = (token, config()) {
        let _ = request(
            "POST",
            &format!("{}/auth/v1/logout", config.url),
            &[
                ("apikey", &config.key),
                ("Authorization", &format!("Bearer {token}")),
            ],
            Some("{}".into()),
        );
    }
    clear_session();
}

fn access_token(force_refresh: bool) -> ApiResult<String> {
    let _guard = REFRESH.lock().map_err(|_| ApiError::SignedOut)?;
    let current = SESSION
        .lock()
        .map_err(|_| ApiError::SignedOut)?
        .clone()
        .ok_or(ApiError::SignedOut)?;
    if !force_refresh && current.expires_at > now_secs() + 60 {
        return Ok(current.access_token);
    }
    refresh_with(&current.refresh_token)?;
    SESSION
        .lock()
        .map_err(|_| ApiError::SignedOut)?
        .as_ref()
        .map(|session| session.access_token.clone())
        .ok_or(ApiError::SignedOut)
}

fn rest(method: &str, path: &str, body: Option<Value>, prefer: &str) -> ApiResult<Value> {
    let config = config()?;
    authed(method, &format!("{}/rest/v1/{path}", config.url), body, prefer)
}

fn function(name: &str, body: Value) -> ApiResult<Value> {
    let config = config()?;
    authed("POST", &format!("{}/functions/v1/{name}", config.url), Some(body), "")
}

fn authed(method: &str, url: &str, body: Option<Value>, prefer: &str) -> ApiResult<Value> {
    let config = config()?;
    let body = body.map(|value| value.to_string());
    let mut token = access_token(false)?;
    for attempt in 0..2 {
        let bearer = format!("Bearer {token}");
        let mut headers = vec![("apikey", config.key.as_str()), ("Authorization", bearer.as_str())];
        if !prefer.is_empty() {
            headers.push(("Prefer", prefer));
        }
        match request(method, url, &headers, body.clone()) {
            Ok(text) if text.trim().is_empty() => return Ok(Value::Null),
            Ok(text) => {
                return serde_json::from_str(&text)
                    .map_err(|_| ApiError::Network("bad response".into()));
            }
            Err(ApiError::Rejected { status: 401, .. }) if attempt == 0 => {
                token = access_token(true)?;
            }
            Err(err) => return Err(err),
        }
    }
    Err(ApiError::SignedOut)
}

// ---------------------------------------------------------------------------
// Read model
// ---------------------------------------------------------------------------

const AVATAR_PALETTE: [u32; 6] = [0x0a84ff, 0x00b339, 0x8b5cf6, 0xec4899, 0xf59e0b, 0x06b6d4];

fn color_for(id: &str) -> u32 {
    let sum = id.bytes().fold(0usize, |acc, byte| acc.wrapping_add(byte as usize));
    AVATAR_PALETTE[sum % AVATAR_PALETTE.len()]
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

fn safe_name(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '_' })
        .collect()
}

const MAX_DOWNLOAD_BYTES: u64 = 16 * 1024 * 1024;

/// Crop once off the UI thread. GPUI's Cover rounds the expanded image bounds,
/// not the visible crop, so sidebar thumbnails must already have the card ratio.
pub fn sidebar_thumbnail(original: &std::path::Path) -> Option<PathBuf> {
    let mut cache_name = original.as_os_str().to_os_string();
    cache_name.push(".sidebar-v1.png");
    let file = PathBuf::from(cache_name);
    if file.exists() {
        return Some(file);
    }
    // Media cache keys are asset IDs, often without a file extension.
    let source = image::ImageReader::open(original).ok()?.with_guessed_format().ok()?.decode().ok()?;
    let cropped = source.resize_to_fill(555, 352, image::imageops::FilterType::Triangle);
    let temp = file.with_extension("part");
    cropped.save_with_format(&temp, image::ImageFormat::Png).ok()?;
    std::fs::rename(temp, &file).ok()?;
    Some(file)
}

#[cfg(test)]
mod thumbnail_tests {
    use super::*;

    #[test]
    fn extensionless_media_is_cropped_and_reuses_its_cached_thumbnail() {
        let original = std::env::temp_dir().join(format!("hero-thumbnail-{}", uuid::Uuid::new_v4()));
        image::RgbaImage::from_pixel(40, 10, image::Rgba([40, 80, 120, 255]))
            .save_with_format(&original, image::ImageFormat::Png).unwrap();
        let thumbnail = sidebar_thumbnail(&original).unwrap();
        assert_eq!(image::image_dimensions(&thumbnail).unwrap(), (555, 352));
        let modified = std::fs::metadata(&thumbnail).unwrap().modified().unwrap();
        assert_eq!(sidebar_thumbnail(&original), Some(thumbnail.clone()));
        assert_eq!(std::fs::metadata(&thumbnail).unwrap().modified().unwrap(), modified);
        std::fs::remove_file(original).unwrap();
        std::fs::remove_file(thumbnail).unwrap();
    }
}

/// Bandwidth-relevant calls this session, so tests can prove nothing downloads repeatedly.
pub static DELIVERY_LINKS: AtomicUsize = AtomicUsize::new(0);
pub static MEDIA_DOWNLOADS: AtomicUsize = AtomicUsize::new(0);
pub static PROFILE_ART_DOWNLOADS: AtomicUsize = AtomicUsize::new(0);
pub static DOWNLOADED_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Writes a download to the cache folder, so each file is fetched at most once. A file over
/// the limit is rejected rather than cached truncated.
fn download_to(url: &str, file: &std::path::Path) -> Option<()> {
    let response = agent()
        .get(url)
        .timeout(Duration::from_secs(180))
        .call()
        .ok()?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(
        &mut std::io::Read::take(response.into_reader(), MAX_DOWNLOAD_BYTES + 1),
        &mut bytes,
    )
    .ok()?;
    DOWNLOADED_BYTES.fetch_add(bytes.len(), Ordering::Relaxed);
    if bytes.is_empty() || bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        return None;
    }
    let temp = file.with_extension("part");
    std::fs::write(&temp, bytes).ok()?;
    std::fs::rename(temp, file).ok()
}

/// Avatars and decorations are re-checked with each snapshot, so a failed one is only
/// tried once per session instead of once a minute.
fn download_profile_art(url: &str, file: &std::path::Path) -> Option<()> {
    static FAILED: Mutex<Option<HashSet<String>>> = Mutex::new(None);
    let already_failed = FAILED
        .lock()
        .ok()
        .is_some_and(|failed| failed.as_ref().is_some_and(|urls| urls.contains(url)));
    if already_failed {
        return None;
    }
    PROFILE_ART_DOWNLOADS.fetch_add(1, Ordering::Relaxed);
    let result = download_to(url, file);
    if result.is_none()
        && let Ok(mut failed) = FAILED.lock()
    {
        failed.get_or_insert_with(HashSet::new).insert(url.to_string());
    }
    result
}

fn cache_file(folder: &str, name: &str) -> PathBuf {
    let dir = data_dir().join(folder);
    let _ = std::fs::create_dir_all(&dir);
    dir.join(safe_name(name))
}

fn cached_avatar(profile: &Value) -> Option<PathBuf> {
    let path = profile["avatar_path"].as_str()?;
    let config = config().ok()?;
    let file = cache_file("avatars", path);
    if file.exists() {
        return Some(file);
    }
    download_profile_art(
        &format!("{}/storage/v1/object/public/avatars/{path}", config.url),
        &file,
    )?;
    Some(file)
}

/// Full APNG for equipped decorations; the catalogue keeps using small stills.
fn cached_decoration(profile: &Value) -> Option<PathBuf> {
    let id = profile["avatar_decoration"].as_str()?;
    if id.len() != 3 || !id.bytes().all(|byte| byte.is_ascii_digit()) { return None; }
    let file = cache_file("decorations", &format!("{id}-animated.png"));
    if file.exists() || download_profile_art(
        &format!("https://res.cloudinary.com/lnkoms9m/image/upload/dislight/avatar-decorations/{id}.png"),
        &file,
    ).is_some() {
        Some(file)
    } else {
        decoration_file(id)
    }
}

/// A decoration's still frame, downloaded once and cached.
pub fn decoration_file(id: &str) -> Option<PathBuf> {
    let legacy = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../public/decorations")
        .join(format!("{id}-still.png"));
    if legacy.exists() {
        return Some(legacy);
    }
    if id.len() != 3 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let file = cache_file("decorations", &format!("{id}.png"));
    if file.exists() {
        return Some(file);
    }
    download_profile_art(
        &format!("https://res.cloudinary.com/lnkoms9m/image/upload/c_limit,w_96/pg_1/f_png/dislight/avatar-decorations/{id}.png"),
        &file,
    )?;
    Some(file)
}

fn avatar_art(profile: &Value) -> AvatarImage {
    AvatarImage {
        path: cached_avatar(profile),
        decoration: cached_decoration(profile),
    }
}

fn name_effect(profile: &Value) -> Option<NameEffect> {
    NameEffect::parse(profile["name_decoration"].as_str()?)
}

/// Nitro's name colours: database value, display colour and label.
pub const NAME_COLORS: [(&str, u32, &str); 10] = [
    ("default", 0xf5f5f7, "Default"),
    ("slate", 0xcbd5e1, "Slate"),
    ("red", 0xf87171, "Red"),
    ("orange", 0xfb923c, "Orange"),
    ("amber", 0xfbbf24, "Amber"),
    ("green", 0x34d399, "Green"),
    ("cyan", 0x22d3ee, "Cyan"),
    ("blue", 0x60a5fa, "Blue"),
    ("violet", 0xa78bfa, "Violet"),
    ("pink", 0xf472b6, "Pink"),
];

fn name_color(profile: &Value) -> u32 {
    let key = profile["name_color"].as_str().unwrap_or("default");
    NAME_COLORS
        .iter()
        .find(|(value, _, _)| *value == key)
        .map(|(_, color, _)| *color)
        .unwrap_or(0xf5f5f7)
}

pub const NAME_WEIGHTS: [(&str, u16, &str); 4] = [
    ("regular", 400, "Regular"),
    ("medium", 500, "Medium"),
    ("bold", 700, "Bold"),
    ("black", 900, "Black"),
];

fn name_weight(profile: &Value) -> u16 {
    match profile["name_weight"].as_str().unwrap_or("medium") {
        "regular" => 400,
        "bold" => 700,
        "black" => 900,
        _ => 500,
    }
}

fn display_of(profile: &Value) -> String {
    let display = text(profile, "display_name");
    if display.is_empty() { text(profile, "username") } else { display }
}

fn profiles_by_id(ids: &HashSet<String>) -> ApiResult<HashMap<String, Value>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let list = ids.iter().cloned().collect::<Vec<_>>().join(",");
    let rows = rest("GET", &format!("profiles?select=*&id=in.({list})"), None, "")?;
    Ok(rows
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| (text(row, "id"), row.clone()))
        .collect())
}

pub struct Snapshot {
    pub profile: UserProfile,
    pub onboarded: bool,
    pub conversations: Vec<PartnerConversation>,
    pub friend_requests: Vec<FriendRequest>,
    /// The newest pushed notice: id, severity and message.
    pub notice: Option<(String, String, String)>,
}

pub fn load_snapshot(user: &SignedInUser) -> ApiResult<Snapshot> {
    let conversations = rest("GET", "conversations?select=*&order=last_message_at.desc", None, "")?;
    let overview = rest("POST", "rpc/conversation_overview", Some(json!({})), "")?;
    let friendships = rest("GET", "friendships?select=*&order=created_at.desc", None, "")?;
    // Optional: a failure here must not stop the app loading.
    let notice = rest(
        "GET",
        "app_notices?select=id,severity,message&order=created_at.desc&limit=1",
        None,
        "",
    )
    .ok()
    .and_then(|rows| rows.as_array().and_then(|rows| rows.first().cloned()))
    .map(|row| (text(&row, "id"), text(&row, "severity"), text(&row, "message")));
    let conversations = conversations.as_array().cloned().unwrap_or_default();
    let friendships = friendships.as_array().cloned().unwrap_or_default();

    let counterpart = |row: &Value| {
        let first = text(row, "user1_id");
        if first == user.id { text(row, "user2_id") } else { first }
    };
    let mut ids: HashSet<String> = conversations.iter().map(counterpart).collect();
    ids.insert(user.id.clone());
    for row in &friendships {
        ids.insert(text(row, "requester_id"));
        ids.insert(text(row, "addressee_id"));
    }
    let profiles = profiles_by_id(&ids)?;

    let unread: HashMap<String, usize> = overview
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            let count = row["unread_count"]
                .as_u64()
                .or_else(|| row["unread_count"].as_str().and_then(|s| s.parse().ok()))
                .unwrap_or(0);
            (text(row, "conversation_id"), count as usize)
        })
        .collect();

    let previews: HashMap<String, String> = overview
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            let content = text(row, "last_message_content");
            let preview = if row["last_message_deleted"].as_bool().unwrap_or(false) {
                "Message deleted".to_string()
            } else if !content.is_empty() {
                content
            } else if row["last_message_id"].is_null() {
                "No messages yet".to_string()
            } else {
                "Attachment".to_string()
            };
            (text(row, "conversation_id"), preview)
        })
        .collect();

    let mine = profiles.get(&user.id).cloned().unwrap_or(Value::Null);
    let my_display = display_of(&mine);
    let profile = UserProfile {
        id: user.id.clone(),
        username: text(&mine, "username"),
        display_name: my_display.clone(),
        email: user.email.clone(),
        initials: initials_for(&my_display),
        avatar_color: color_for(&user.id),
        photo: avatar_art(&mine),
        name_color: name_color(&mine),
        name_weight_value: name_weight(&mine),
        name_effect: name_effect(&mine),
        name_font: text(&mine, "name_font"),
        name_weight: text(&mine, "name_weight"),
        avatar_decoration: mine["avatar_decoration"].as_str().map(str::to_string),
        onboarding_completed: !mine["onboarding_completed_at"].is_null(),
    };
    let onboarded = profile.onboarding_completed;

    let conversations = conversations
        .iter()
        .map(|row| {
            let partner_id = counterpart(row);
            let partner = profiles.get(&partner_id).cloned().unwrap_or(Value::Null);
            let display = display_of(&partner);
            let id = text(row, "id");
            PartnerConversation {
                unread_count: unread.get(&id).copied().unwrap_or(0),
                voice_started_at: None,
                voice_participants: vec![],
                preview: previews
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| "No messages yet".into()),
                id,
                partner_id: partner_id.clone(),
                username: text(&partner, "username"),
                initials: initials_for(&display),
                display_name: display,
                color: color_for(&partner_id),
                photo: avatar_art(&partner),
                name_color: name_color(&partner),
                name_weight: name_weight(&partner),
                name_effect: name_effect(&partner),
                presence: "unknown".into(),
                status_text: format!("@{}", text(&partner, "username")),
                messages: vec![],
                shared_media: vec![],
                loaded: false,
                has_more: false,
            }
        })
        .collect();

    let friend_requests = friendships
        .iter()
        .filter(|row| row["status"].as_str() == Some("pending"))
        .map(|row| {
            let requester = text(row, "requester_id");
            let incoming = requester != user.id;
            let other = if incoming { requester } else { text(row, "addressee_id") };
            let partner = profiles.get(&other).cloned().unwrap_or(Value::Null);
            let display = display_of(&partner);
            FriendRequest {
                id: text(row, "id"),
                username: text(&partner, "username"),
                initials: initials_for(&display),
                display_name: display,
                color: color_for(&other),
                incoming,
            }
        })
        .collect();

    Ok(Snapshot {
        profile,
        onboarded,
        conversations,
        friend_requests,
        notice,
    })
}

/// Who appears on a conversation's message rows.
#[derive(Clone)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub initials: String,
    pub color: u32,
    pub photo: AvatarImage,
    pub name_color: u32,
    pub name_weight: u16,
    pub name_effect: Option<NameEffect>,
}

#[derive(Clone)]
pub struct Participants {
    pub me: Person,
    pub partner: Person,
}

static NEXT_ROW_ID: AtomicUsize = AtomicUsize::new(1);

pub fn next_row_id() -> usize {
    NEXT_ROW_ID.fetch_add(1, Ordering::Relaxed)
}

fn local_time(stamp: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(stamp)
        .ok()
        .map(|time| time.with_timezone(&Local))
}

pub fn clock_label(stamp: &str) -> String {
    local_time(stamp)
        .map(|time| time.format("%H:%M").to_string())
        .unwrap_or_default()
}

pub fn now_stamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn duration_label(seconds: f64) -> String {
    let total = seconds.round() as u64;
    if total >= 3600 {
        format!("{}:{:02}:{:02}", total / 3600, total / 60 % 60, total % 60)
    } else {
        format!("{}:{:02}", total / 60, total % 60)
    }
}

fn attachment_of(row: &Value) -> ChatAttachment {
    if !row["media_deleted_at"].is_null() {
        return ChatAttachment::None;
    }
    let resolution = match (row["media_width"].as_u64(), row["media_height"].as_u64()) {
        (Some(w), Some(h)) => format!("{w}×{h}"),
        _ => String::new(),
    };
    match row["media_kind"].as_str() {
        Some("image") => ChatAttachment::MediaGallery {
            title: "Image".into(),
            subtitle: resolution,
            items: 1,
        },
        Some("video") => ChatAttachment::VideoClip {
            title: "Video".into(),
            duration: duration_label(row["media_duration_seconds"].as_f64().unwrap_or(0.)),
            resolution,
        },
        _ => ChatAttachment::None,
    }
}

fn message_text(row: &Value, sender: &str) -> String {
    match row["message_kind"].as_str() {
        Some("voice_started") => format!("{sender} started a voice call"),
        Some("voice_ended") => match row["voice_duration_seconds"].as_u64() {
            Some(seconds) => format!("Voice call ended · {}", duration_label(seconds as f64)),
            None => "Voice call ended".into(),
        },
        _ => text(row, "content"),
    }
}

fn map_rows(rows: &[Value], who: &Participants) -> ApiResult<Vec<ChatMessage>> {
    if rows.is_empty() {
        return Ok(vec![]);
    }
    let ids: Vec<String> = rows.iter().map(|row| text(row, "id")).collect();
    let reactions = rest(
        "GET",
        &format!("reactions?select=*&message_id=in.({})", ids.join(",")),
        None,
        "",
    )?;
    let reply_ids: Vec<String> = rows
        .iter()
        .filter_map(|row| row["reply_to_message_id"].as_str().map(str::to_string))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let replies: HashMap<String, Value> = if reply_ids.is_empty() {
        HashMap::new()
    } else {
        rest(
            "GET",
            &format!(
                "messages?select=id,sender_id,content,media_kind,deleted_at&id=in.({})",
                reply_ids.join(",")
            ),
            None,
            "",
        )?
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| (text(row, "id"), row.clone()))
        .collect()
    };

    let mut by_message: HashMap<String, Vec<ReactionChip>> = HashMap::new();
    for row in reactions.as_array().into_iter().flatten() {
        let chips = by_message.entry(text(row, "message_id")).or_default();
        let emoji = text(row, "emoji");
        let mine_row = (text(row, "user_id") == who.me.id).then(|| text(row, "id"));
        match chips.iter_mut().find(|chip| chip.emoji == emoji) {
            Some(chip) => {
                chip.count += 1;
                chip.mine = chip.mine || mine_row.is_some();
                if mine_row.is_some() {
                    chip.mine_row = mine_row;
                }
            }
            None => chips.push(ReactionChip {
                emoji,
                count: 1,
                mine: mine_row.is_some(),
                mine_row,
            }),
        }
    }

    Ok(rows
        .iter()
        .map(|row| {
            let id = text(row, "id");
            let is_self = text(row, "sender_id") == who.me.id;
            let person = if is_self { &who.me } else { &who.partner };
            let created_at = text(row, "created_at");
            let reply_to = row["reply_to_message_id"].as_str().and_then(|target| {
                let reply = replies.get(target)?;
                let author = if text(reply, "sender_id") == who.me.id {
                    who.me.name.clone()
                } else {
                    who.partner.name.clone()
                };
                let excerpt = if !reply["deleted_at"].is_null() {
                    "Message deleted".to_string()
                } else if text(reply, "content").is_empty() {
                    "Attachment".to_string()
                } else {
                    text(reply, "content").chars().take(80).collect()
                };
                Some((author, excerpt))
            });
            ChatMessage {
                id: next_row_id(),
                reactions: by_message.remove(&id).unwrap_or_default(),
                db_id: id,
                pending: false,
                sender: person.name.clone(),
                initials: person.initials.clone(),
                avatar_color: person.color,
                photo: person.photo.clone(),
                name_color: person.name_color,
                name_weight: person.name_weight,
                name_effect: person.name_effect,
                media_bytes: row["media_size_bytes"].as_u64(),
                media_path: row["media_path"].as_str().map(str::to_string).filter(|_| row["media_deleted_at"].is_null()),
                media_size: match (row["media_width"].as_u64(), row["media_height"].as_u64()) {
                    (Some(w), Some(h)) => Some((w as u32, h as u32)),
                    _ => None,
                },
                role_badge: None,
                time: clock_label(&created_at),
                created_at,
                system: matches!(
                    row["message_kind"].as_str(),
                    Some("voice_started" | "voice_ended")
                ),
                text: message_text(row, &person.name),
                reply_to,
                attachment: attachment_of(row),
                is_self,
                pinned: false,
                edited: !row["edited_at"].is_null(),
                deleted: !row["deleted_at"].is_null(),
                marker_before: None,
            }
        })
        .collect())
}

/// The newest page of a conversation, oldest first.
pub fn load_latest(conversation_id: &str, who: &Participants) -> ApiResult<(Vec<ChatMessage>, bool)> {
    let rows = rest(
        "GET",
        &format!(
            "messages?select=*&conversation_id=eq.{conversation_id}&order=created_at.desc,id.desc&limit={PAGE_SIZE}"
        ),
        None,
        "",
    )?;
    let mut rows = rows.as_array().cloned().unwrap_or_default();
    let has_more = rows.len() == PAGE_SIZE;
    rows.reverse();
    Ok((map_rows(&rows, who)?, has_more))
}

/// Messages newer than `after`, oldest first. Cheap enough to poll.
pub fn load_newer(conversation_id: &str, after: &str, who: &Participants) -> ApiResult<Vec<ChatMessage>> {
    let after = after.replace('+', "%2B");
    let rows = rest(
        "GET",
        &format!(
            "messages?select=*&conversation_id=eq.{conversation_id}&created_at=gt.{after}&order=created_at.asc,id.asc&limit={PAGE_SIZE}"
        ),
        None,
        "",
    )?;
    map_rows(rows.as_array().map(Vec::as_slice).unwrap_or(&[]), who)
}

/// The page just before `before`, oldest first, and whether more exist.
pub fn load_older(
    conversation_id: &str,
    before: &str,
    who: &Participants,
) -> ApiResult<(Vec<ChatMessage>, bool)> {
    let before = before.replace('+', "%2B");
    let rows = rest(
        "GET",
        &format!(
            "messages?select=*&conversation_id=eq.{conversation_id}&created_at=lt.{before}&order=created_at.desc,id.desc&limit={PAGE_SIZE}"
        ),
        None,
        "",
    )?;
    let mut rows = rows.as_array().cloned().unwrap_or_default();
    let has_more = rows.len() == PAGE_SIZE;
    rows.reverse();
    Ok((map_rows(&rows, who)?, has_more))
}

/// Returns the cached file for a chat image, downloading it the first time only.
pub fn fetch_chat_image(path: &str) -> ApiResult<PathBuf> {
    let file = cache_file("media", path);
    if file.exists() {
        return Ok(file);
    }
    let config = config()?;
    DELIVERY_LINKS.fetch_add(1, Ordering::Relaxed);
    let url = if path.starts_with("cloudinary:") {
        let reply = function("purge-chat-media", json!({ "mode": "delivery", "path": path }))?;
        text(&reply, "url")
    } else {
        let reply = rest_storage_sign(path)?;
        format!("{}/storage/v1{}", config.url, text(&reply, "signedURL"))
    };
    if url.is_empty() {
        return Err(ApiError::Network("no media url".into()));
    }
    MEDIA_DOWNLOADS.fetch_add(1, Ordering::Relaxed);
    download_to(&url, &file).ok_or(ApiError::Network("download failed".into()))?;
    Ok(file)
}

fn rest_storage_sign(path: &str) -> ApiResult<Value> {
    let config = config()?;
    authed(
        "POST",
        &format!("{}/storage/v1/object/sign/chat-media/{path}", config.url),
        Some(json!({ "expiresIn": 300 })),
        "",
    )
}

/// Decoration ids from the published catalogue, or the main app's bundled fallback.
pub fn decoration_catalog() -> Vec<String> {
    let listed = agent()
        .get("https://res.cloudinary.com/lnkoms9m/image/list/dislight-avatar-decoration.json")
        .call()
        .ok()
        .and_then(|response| response.into_string().ok())
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let mut ids: Vec<String> = listed
        .as_ref()
        .and_then(|value| value["resources"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|row| row["public_id"].as_str()?.rsplit('/').next().map(str::to_string))
        .filter(|id| id.len() == 3 && id.bytes().all(|byte| byte.is_ascii_digit()))
        .collect();
    ids.sort();
    if ids.is_empty() {
        ids = (1..=10).map(|number| format!("{number:03}")).collect();
    }
    ids
}

pub fn update_profile(user_id: &str, fields: Value) -> ApiResult<()> {
    rest(
        "PATCH",
        &format!("profiles?id=eq.{user_id}"),
        Some(fields),
        "return=minimal",
    )
    .map(|_| ())
}

/// Who's in a conversation's voice room: when it started, and users in join order.
pub fn load_voice(conversation_id: &str) -> ApiResult<(Option<String>, Vec<String>)> {
    let rooms = rest(
        "GET",
        &format!("voice_rooms?select=started_at&conversation_id=eq.{conversation_id}"),
        None,
        "",
    )?;
    let people = rest(
        "GET",
        &format!(
            "voice_participants?select=user_id&conversation_id=eq.{conversation_id}&order=joined_at.asc"
        ),
        None,
        "",
    )?;
    let started = rooms
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(|row| row["started_at"].as_str())
        .map(str::to_string);
    let mut users: Vec<String> = vec![];
    for row in people.as_array().into_iter().flatten() {
        let user = text(row, "user_id");
        if !users.contains(&user) {
            users.push(user);
        }
    }
    Ok((started, users))
}

pub fn seconds_since(stamp: &str) -> Option<u64> {
    let then = DateTime::parse_from_rfc3339(stamp).ok()?;
    Some((chrono::Utc::now().timestamp() - then.timestamp()).max(0) as u64)
}

/// Returns the cached sound file, downloading it the first time only.
pub fn fetch_sound(sound_id: &str, conversation: Option<&str>) -> ApiResult<PathBuf> {
    let file = cache_file("sounds", &format!("{sound_id}.webm"));
    if file.exists() {
        return Ok(file);
    }
    let url = sound_link(sound_id, conversation)?;
    MEDIA_DOWNLOADS.fetch_add(1, Ordering::Relaxed);
    download_to(&url, &file).ok_or(ApiError::Network("download failed".into()))?;
    Ok(file)
}

/// A short-lived link for the partner, who may not otherwise be able to read this sound.
pub fn sound_link(sound_id: &str, conversation: Option<&str>) -> ApiResult<String> {
    // In a call this also covers the partner's sounds. "preview" only covers your own.
    let body = match conversation {
        Some(conversation) => json!({ "mode": "play", "soundId": sound_id, "conversationId": conversation }),
        None => json!({ "mode": "preview", "soundId": sound_id }),
    };
    let reply = function("soundboard-storage", body)?;
    let url = text(&reply, "signedUrl");
    if url.is_empty() {
        return Err(ApiError::Network("no sound url".into()));
    }
    Ok(url)
}

/// A sound the partner played. Only links to this project's soundboard storage are followed.
pub fn fetch_shared_sound(sound_id: &str, url: &str) -> ApiResult<PathBuf> {
    let project = config().map_err(|_| ApiError::NotConfigured)?.url.clone();
    let trusted = url.strip_prefix(project.as_str())
        .is_some_and(|path| path.starts_with("/storage/v1/object/sign/soundboard/"));
    if !trusted {
        return Err(ApiError::Network("untrusted sound link".into()));
    }
    let file = cache_file("sounds", &format!("{sound_id}.webm"));
    if file.exists() {
        return Ok(file);
    }
    MEDIA_DOWNLOADS.fetch_add(1, Ordering::Relaxed);
    download_to(url, &file).ok_or(ApiError::Network("download failed".into()))?;
    Ok(file)
}

/// Reserves storage, uploads, then confirms. The reservation is released if any step fails.
pub fn upload_sound(name: &str, webm: Vec<u8>, duration_ms: u32) -> ApiResult<()> {
    let sound_id = uuid::Uuid::new_v4().to_string();
    let reservation = function(
        "soundboard-storage",
        json!({ "mode": "reserve", "soundId": sound_id, "name": name, "sizeBytes": webm.len(), "durationMs": duration_ms }),
    )?;
    let (path, token) = (text(&reservation, "path"), text(&reservation, "token"));
    let finished = (|| {
        if path.is_empty() || token.is_empty() {
            return Err(ApiError::Network("no upload link".into()));
        }
        let config = config()?;
        let url = format!("{}/storage/v1/object/upload/sign/soundboard/{path}?token={token}", config.url);
        agent()
            .put(&url)
            .set("apikey", &config.key)
            .set("Content-Type", "audio/webm")
            .set("Cache-Control", "max-age=31536000")
            .send_bytes(&webm)
            .map_err(|_| ApiError::Network("The sound couldn't be uploaded.".into()))?;
        function(
            "soundboard-storage",
            json!({ "mode": "finalize", "soundId": sound_id, "name": name, "sizeBytes": webm.len(), "durationMs": duration_ms }),
        )
    })();
    if let Err(error) = finished {
        let _ = function("soundboard-storage", json!({ "mode": "discard", "soundId": sound_id }));
        return Err(error);
    }
    Ok(())
}

pub fn delete_sound(sound_id: &str) -> ApiResult<()> {
    function("soundboard-storage", json!({ "mode": "delete", "soundId": sound_id })).map(|_| ())
}

pub fn rename_sound(sound_id: &str, name: &str) -> ApiResult<()> {
    function("soundboard-storage", json!({ "mode": "rename", "soundId": sound_id, "name": name }))
        .map(|_| ())
}

/// The sounds this account can see. The shared list needs an active voice room.
pub fn load_sounds(
    my_id: &str,
    owners: &HashMap<String, (String, u32, AvatarImage)>,
    conversation: Option<&str>,
) -> ApiResult<Vec<SoundClip>> {
    let body = match conversation {
        Some(conversation) => json!({ "mode": "list", "conversationId": conversation }),
        None => json!({ "mode": "list" }),
    };
    let reply = function("soundboard-storage", body)?;
    Ok(reply["sounds"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            let owner = text(row, "owner_id");
            let (owner_name, owner_color, owner_art) = owners
                .get(&owner)
                .cloned()
                .unwrap_or_else(|| ("Someone".to_string(), color_for(&owner), AvatarImage::default()));
            SoundClip {
                id: text(row, "id"),
                name: text(row, "name"),
                duration_label: format!(
                    "{:.1}s",
                    row["duration_ms"].as_f64().unwrap_or(0.) / 1000.
                ),
                owner_initials: initials_for(&owner_name),
                owner_name,
                is_mine: owner == my_id,
                duration_ms: row["duration_ms"].as_u64().unwrap_or(0),
                size_bytes: row["size_bytes"].as_u64().unwrap_or(0),
                owner_color,
                owner_art,
            }
        })
        .collect())
}

/// Merges a server page into what's on screen: keeps row ids stable and unsent
/// messages in place, then recomputes date markers.
pub fn merge_messages(existing: &mut Vec<ChatMessage>, incoming: Vec<ChatMessage>) {
    for mut message in incoming {
        match existing.iter_mut().find(|row| row.db_id == message.db_id) {
            Some(row) => {
                message.id = row.id;
                *row = message;
            }
            None => existing.push(message),
        }
    }
    existing.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.db_id.cmp(&b.db_id)));
    let mut last_day = None;
    for row in existing.iter_mut() {
        let day = local_time(&row.created_at).map(|time| time.date_naive());
        row.marker_before = match (day, last_day) {
            (Some(current), previous) if previous != Some(current) => {
                local_time(&row.created_at).map(|time| time.format("%a, %b %-d").to_string())
            }
            _ => None,
        };
        if day.is_some() {
            last_day = day;
        }
    }
}

/// Identifies a file by kind, byte size and dimensions, which is what the database stores.
/// Copies of the same upload match exactly; distinct files practically never do.
pub fn media_key(message: &ChatMessage) -> Option<(bool, u64, u32, u32)> {
    let is_video = match message.attachment {
        ChatAttachment::None => return None,
        ChatAttachment::MediaGallery { .. } => false,
        ChatAttachment::VideoClip { .. } => true,
    };
    let (width, height) = message.media_size?;
    Some((is_video, message.media_bytes?, width, height))
}

/// How long a sender can pause between items and stay in the same burst.
const BURST_GAP_SECONDS: i64 = 120;

fn can_join_burst(message: &ChatMessage) -> bool {
    !matches!(message.attachment, ChatAttachment::None)
        && !message.system
        && message.text.is_empty()
        && !message.deleted
        && !message.pending
        && message.reply_to.is_none()
}

/// Splits messages into runs to render as one card: consecutive captionless media from
/// the same sender, each within `BURST_GAP_SECONDS` of the last. Returns (start, length).
pub fn burst_groups(messages: &[ChatMessage]) -> Vec<(usize, usize)> {
    let mut groups: Vec<(usize, usize)> = vec![];
    for (index, message) in messages.iter().enumerate() {
        let joins = groups.last().is_some_and(|(start, length)| {
            let previous = &messages[start + length - 1];
            can_join_burst(previous)
                && can_join_burst(message)
                && previous.sender == message.sender
                && message.marker_before.is_none()
                && matches!(
                    (local_time(&previous.created_at), local_time(&message.created_at)),
                    (Some(a), Some(b)) if (b - a).num_seconds() <= BURST_GAP_SECONDS
                )
        });
        if joins {
            if let Some(group) = groups.last_mut() {
                group.1 += 1;
            }
        } else {
            groups.push((index, 1));
        }
    }
    groups
}

/// The sidebar's media list, derived from loaded messages. Metadata only for now.
pub fn media_items(messages: &[ChatMessage]) -> Vec<MediaItem> {
    let mut seen = HashSet::new();
    messages
        .iter()
        .rev()
        .filter(|message| !message.deleted)
        .filter(|message| media_key(message).is_none_or(|key| seen.insert(key)))
        .filter_map(|message| {
            let (kind, title, meta, accent) = match &message.attachment {
                ChatAttachment::None => return None,
                ChatAttachment::MediaGallery { title, subtitle, .. } => {
                    ("photo", title.clone(), subtitle.clone(), 0x0a84ff)
                }
                ChatAttachment::VideoClip { title, duration, resolution } => {
                    ("clip", title.clone(), format!("{duration} · {resolution}"), 0x00b339)
                }
            };
            Some(MediaItem {
                id: message.db_id.clone(),
                media_path: message.media_path.clone(),
                kind: kind.into(),
                title,
                meta,
                sender: message.sender.clone(),
                pinned: false,
                starred: false,
                accent,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

pub fn send_message(
    id: &str,
    conversation_id: &str,
    sender_id: &str,
    content: &str,
    reply_to: Option<&str>,
) -> ApiResult<()> {
    rest(
        "POST",
        "messages",
        Some(json!({
            "id": id,
            "conversation_id": conversation_id,
            "sender_id": sender_id,
            "content": content,
            "reply_to_message_id": reply_to,
        })),
        "return=minimal",
    )
    .map(|_| ())
}

/// Reserves space, uploads the original, and sends the message, as main Nitro does. The
/// reservation is given back if any step fails.
pub fn send_image_message(
    id: &str,
    conversation_id: &str,
    sender_id: &str,
    content: &str,
    reply_to: Option<&str>,
    media: &crate::chat_media::PendingUpload,
) -> ApiResult<()> {
    let reservation = function(
        "purge-chat-media",
        json!({
            "mode": "reserve",
            "provider": "cloudinary",
            "conversationId": conversation_id,
            "messageId": id,
            "kind": "image",
            "mimeType": "image/webp",
            "uploadMimeType": media.upload_mime,
            "sizeBytes": media.bytes.len(),
        }),
    )?;
    let path = text(&reservation, "path");
    if path.is_empty() || reservation["provider"] != "cloudinary" {
        return Err(ApiError::Network("The upload couldn't start.".into()));
    }
    let sent = upload_to_cloudinary(&reservation, media).and_then(|()| {
        rest(
            "POST",
            "messages",
            Some(json!({
                "id": id,
                "conversation_id": conversation_id,
                "sender_id": sender_id,
                "content": content,
                "reply_to_message_id": reply_to,
                "media_kind": "image",
                "media_path": path,
                "media_mime_type": "image/webp",
                "media_size_bytes": media.bytes.len(),
                "media_width": media.width,
                "media_height": media.height,
            })),
            "return=minimal",
        )
        .map(|_| ())
    });
    let mode = if sent.is_ok() { "finalize" } else { "discard" };
    let _ = function("purge-chat-media", json!({ "mode": mode, "path": path }));
    sent
}

fn upload_to_cloudinary(reservation: &Value, media: &crate::chat_media::PendingUpload) -> ApiResult<()> {
    let boundary = format!("nitro{}", uuid::Uuid::new_v4().simple());
    let mut body = Vec::with_capacity(media.bytes.len() + 1024);
    let fields = reservation["fields"].as_object().ok_or(ApiError::Network("The upload couldn't start.".into()))?;
    for (name, value) in fields {
        let value = value.as_str().map(String::from).unwrap_or_else(|| value.to_string());
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"upload\"\r\nContent-Type: {}\r\n\r\n",
            media.upload_mime
        )
        .as_bytes(),
    );
    body.extend_from_slice(&media.bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let url = text(reservation, "uploadUrl");
    if !url.starts_with("https://api.cloudinary.com/") {
        return Err(ApiError::Network("The upload couldn't start.".into()));
    }
    let response = ureq::post(&url)
        .timeout(Duration::from_secs(120))
        .set("Content-Type", &format!("multipart/form-data; boundary={boundary}"))
        .send_bytes(&body)
        .map_err(|_| ApiError::Network("The image couldn't be uploaded.".into()))?;
    let result: Value = serde_json::from_reader(response.into_reader())
        .map_err(|_| ApiError::Network("The image couldn't be uploaded.".into()))?;
    if result["public_id"] != reservation["publicId"] {
        return Err(ApiError::Network("The image couldn't be uploaded.".into()));
    }
    Ok(())
}

pub fn mark_read(conversation_id: &str, user_id: &str) -> ApiResult<()> {
    rest(
        "POST",
        "conversation_reads?on_conflict=conversation_id,user_id",
        Some(json!({
            "conversation_id": conversation_id,
            "user_id": user_id,
            "last_read_at": now_stamp(),
        })),
        "resolution=merge-duplicates,return=minimal",
    )
    .map(|_| ())
}

pub fn edit_message(message_id: &str, content: &str) -> ApiResult<()> {
    rest(
        "PATCH",
        &format!("messages?id=eq.{message_id}"),
        Some(json!({ "content": content, "edited_at": now_stamp() })),
        "return=minimal",
    )
    .map(|_| ())
}

pub fn delete_message(message_id: &str) -> ApiResult<()> {
    rest(
        "PATCH",
        &format!("messages?id=eq.{message_id}"),
        Some(json!({ "deleted_at": now_stamp() })),
        "return=minimal",
    )
    .map(|_| ())
}

pub fn add_reaction(message_id: &str, user_id: &str, emoji: &str) -> ApiResult<()> {
    rest(
        "POST",
        "reactions",
        Some(json!({ "message_id": message_id, "user_id": user_id, "emoji": emoji })),
        "return=minimal",
    )
    .map(|_| ())
}

pub fn remove_reaction(reaction_id: &str) -> ApiResult<()> {
    rest("DELETE", &format!("reactions?id=eq.{reaction_id}"), None, "return=minimal").map(|_| ())
}

pub fn send_friend_request(username: &str, my_id: &str, my_username: &str) -> ApiResult<()> {
    let username = username.trim().to_lowercase();
    let rejected = |message: &str| ApiError::Rejected {
        status: 400,
        message: message.into(),
    };
    if !crate::state::is_valid_username(&username) {
        return Err(rejected("Usernames are 3–20 characters: a–z, 0–9, underscores."));
    }
    if username == my_username {
        return Err(rejected("That's your own username."));
    }
    let found = rest("GET", &format!("profiles?select=id&username=eq.{username}"), None, "")?;
    let Some(target) = found.as_array().and_then(|rows| rows.first()) else {
        return Err(rejected(&format!("No one goes by @{username}.")));
    };
    rest(
        "POST",
        "friendships",
        Some(json!({ "requester_id": my_id, "addressee_id": text(target, "id") })),
        "return=minimal",
    )
    .map(|_| ())
    .map_err(|err| match err {
        ApiError::Rejected { status: 409, .. } => rejected("A request between you already exists."),
        other => other,
    })
}

pub fn accept_friend_request(id: &str) -> ApiResult<()> {
    rest(
        "PATCH",
        &format!("friendships?id=eq.{id}"),
        Some(json!({ "status": "accepted" })),
        "return=minimal",
    )
    .map(|_| ())
}

pub fn delete_friendship(id: &str) -> ApiResult<()> {
    rest("DELETE", &format!("friendships?id=eq.{id}"), None, "return=minimal").map(|_| ())
}

pub fn complete_onboarding(user_id: &str, display_name: &str) -> ApiResult<()> {
    rest(
        "PATCH",
        &format!("profiles?id=eq.{user_id}"),
        Some(json!({ "display_name": display_name, "onboarding_completed_at": now_stamp() })),
        "return=minimal",
    )
    .map(|_| ())
}

// ---------------------------------------------------------------------------
// DPAPI (current Windows user only)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod dpapi {
    use std::{ffi::c_void, ptr, slice};

    #[repr(C)]
    struct Blob {
        len: u32,
        data: *mut u8,
    }

    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            input: *const Blob,
            description: *const u16,
            entropy: *const Blob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
        fn CryptUnprotectData(
            input: *const Blob,
            description: *mut *mut u16,
            entropy: *const Blob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    fn take(blob: Blob) -> Vec<u8> {
        unsafe {
            let bytes = slice::from_raw_parts(blob.data, blob.len as usize).to_vec();
            LocalFree(blob.data.cast());
            bytes
        }
    }

    pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
        let input = Blob { len: plain.len() as u32, data: plain.as_ptr() as *mut u8 };
        let mut output = Blob { len: 0, data: ptr::null_mut() };
        let ok = unsafe {
            CryptProtectData(&input, ptr::null(), ptr::null(), ptr::null_mut(), ptr::null_mut(), 1, &mut output)
        };
        (ok != 0).then(|| take(output))
    }

    pub fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
        let input = Blob { len: sealed.len() as u32, data: sealed.as_ptr() as *mut u8 };
        let mut output = Blob { len: 0, data: ptr::null_mut() };
        let ok = unsafe {
            CryptUnprotectData(&input, ptr::null_mut(), ptr::null(), ptr::null_mut(), ptr::null_mut(), 1, &mut output)
        };
        (ok != 0).then(|| take(output))
    }
}

#[cfg(not(windows))]
mod dpapi {
    pub fn protect(_: &[u8]) -> Option<Vec<u8>> {
        None
    }
    pub fn unprotect(_: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod burst_tests {
    use super::*;

    fn image(sender: &str, at: &str, text: &str, bytes: u64) -> ChatMessage {
        ChatMessage {
            id: next_row_id(),
            system: false,
            name_color: 0,
            name_weight: 500,
            name_effect: None,
            media_path: Some("p".into()),
            media_size: Some((10, 10)),
            media_bytes: Some(bytes),
            db_id: at.into(),
            created_at: at.into(),
            pending: false,
            sender: sender.into(),
            initials: String::new(),
            avatar_color: 0,
            photo: Default::default(),
            role_badge: None,
            time: String::new(),
            text: text.into(),
            reply_to: None,
            attachment: ChatAttachment::MediaGallery {
                title: String::new(),
                subtitle: String::new(),
                items: 1,
            },
            reactions: vec![],
            is_self: false,
            pinned: false,
            edited: false,
            deleted: false,
            marker_before: None,
        }
    }

    const T0: &str = "2026-10-25T14:42:00+00:00";
    const T1: &str = "2026-10-25T14:43:30+00:00";
    const T2: &str = "2026-10-25T14:50:00+00:00";

    #[test]
    fn groups_quick_captionless_media_from_one_sender() {
        let rows = [image("a", T0, "", 1), image("a", T1, "", 1)];
        assert_eq!(burst_groups(&rows), vec![(0, 2)]);
    }

    #[test]
    fn splits_on_gap_sender_and_caption() {
        let gap = [image("a", T0, "", 1), image("a", T2, "", 1)];
        assert_eq!(burst_groups(&gap), vec![(0, 1), (1, 1)]);
        let sender = [image("a", T0, "", 1), image("b", T1, "", 1)];
        assert_eq!(burst_groups(&sender), vec![(0, 1), (1, 1)]);
        let caption = [image("a", T0, "", 1), image("a", T1, "look", 1)];
        assert_eq!(burst_groups(&caption), vec![(0, 1), (1, 1)]);
    }

    #[test]
    fn sidebar_lists_identical_files_once() {
        let rows = [image("a", T0, "", 7), image("a", T1, "", 7), image("a", T2, "", 8)];
        assert_eq!(media_items(&rows).len(), 2);
    }
}

#[cfg(test)]
mod legacy_session_tests {
    use super::*;

    /// Exchanges this machine's real old-app token, as the first launch does. Prints no token.
    #[test]
    #[ignore]
    fn the_old_apps_token_signs_in() {
        let token = crate::migrate::legacy_refresh_token().expect("no token found");
        println!("signed in: {}", refresh_with(&token).is_ok());
    }
}
