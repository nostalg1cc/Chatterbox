use serde::{Deserialize, Serialize};
use crate::components::{AvatarImage, NameEffect};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthStatus {
    #[default]
    SignedOut,
    Onboarding,
    SignedIn,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthTab {
    Login,
    Signup,
    VerifyEmail(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellView {
    Chat,
    Friends,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingsTab {
    General,
    Appearance,
    Chat,
    Voice,
    Keybinds,
    Soundboard,
    About,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderDropdown {
    Switcher,
    Soundboard,
    Microphone,
    Output,
    Screen,
}

pub use crate::themes::AppTheme;

/// The colour roles components draw with.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub canvas: u32,
    pub stage: u32,
    pub surface: u32,
    pub subtle: u32,
    pub panel: u32,
    pub raised: u32,
    pub media: u32,
    pub accent: u32,
    pub accent_soft: u32,
    pub accent_line: u32,
    pub text: u32,
    pub muted: u32,
    pub selected: u32,
    pub hairline: u32,
}

static ACTIVE_THEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Set once per frame by the root view; components read it while painting.
pub fn set_active_theme(theme: AppTheme) {
    let index = AppTheme::ALL.iter().position(|t| *t == theme).unwrap_or(0) as u8;
    ACTIVE_THEME.store(index, std::sync::atomic::Ordering::Relaxed);
}

pub fn active_is_default() -> bool {
    ACTIVE_THEME.load(std::sync::atomic::Ordering::Relaxed) == 0
}

pub fn palette() -> Palette {
    let index = ACTIVE_THEME.load(std::sync::atomic::Ordering::Relaxed) as usize;
    AppTheme::ALL[index.min(AppTheme::ALL.len() - 1)].palette()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub initials: String,
    pub avatar_color: u32,
    pub photo: AvatarImage,
    pub name_color: u32,
    pub name_weight_value: u16,
    pub name_effect: Option<NameEffect>,
    pub name_font: String,
    pub name_weight: String,
    pub avatar_decoration: Option<String>,
    pub onboarding_completed: bool,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            username: String::new(),
            display_name: String::new(),
            email: String::new(),
            initials: String::new(),
            avatar_color: 0x0a84ff,
            photo: AvatarImage::default(),
            name_color: 0xf2f2f3,
            name_weight_value: 500,
            name_effect: None,
            name_font: "sans".into(),
            name_weight: "semibold".into(),
            avatar_decoration: None,
            onboarding_completed: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub input_device_id: String,
    pub output_device_id: String,
    pub input_volume: u8,
    pub output_volume: u8,
    pub partner_volume: u8,
    pub soundboard_volume: u8,
    pub pinned_sound_ids: Vec<String>,
    pub enter_to_send: bool,
    pub compact_messages: bool,
    pub show_media_previews: bool,
    pub interface_sounds: bool,
    #[serde(default)]
    pub dismissed_notice_id: Option<String>,
    #[serde(default = "enabled_by_default")]
    pub voice_hud_enabled: bool,
    #[serde(default = "default_hud_scale")]
    pub voice_hud_scale: u8,
    #[serde(default)]
    pub voice_hud_show_names: bool,
    /// Mute, deafen, leave and open soundboard, in `KEYBIND_ACTIONS` order.
    #[serde(default = "default_keybinds")]
    pub keybinds: [String; 4],
    #[serde(default = "default_interface_volume")]
    pub interface_sound_volume: u8,
    #[serde(default = "enabled_by_default")]
    pub glass_ui: bool,
    /// Where the window was, to open there next time.
    #[serde(default)]
    pub window: Option<SavedWindow>,
    #[serde(default = "enabled_by_default")]
    pub wheel_enabled: bool,
    #[serde(default = "default_wheel_key")]
    pub wheel_key: String,
    /// Open only after the key has been held, rather than at once.
    #[serde(default)]
    pub wheel_hold: bool,
    #[serde(default = "default_wheel_hold_tenths")]
    pub wheel_hold_tenths: u8,
    /// Settings from the old app are only ever brought over once.
    #[serde(default)]
    pub legacy_imported: bool,
    pub echo_cancellation: bool,
    pub noise_suppression: bool,
    pub theme: AppTheme,
}

fn enabled_by_default() -> bool {
    true
}

fn default_hud_scale() -> u8 {
    70
}

pub const KEYBIND_ACTIONS: [&str; 4] = ["Toggle mute", "Toggle deafen", "Leave call", "Open soundboard"];

fn default_keybinds() -> [String; 4] {
    ["ctrl-shift-m", "ctrl-shift-d", "ctrl-shift-l", "ctrl-shift-b"].map(String::from)
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedWindow {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

fn default_wheel_key() -> String {
    "alt-g".into()
}

fn default_wheel_hold_tenths() -> u8 {
    10
}

fn default_interface_volume() -> u8 {
    65
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            input_device_id: "default".into(),
            output_device_id: "default".into(),
            legacy_imported: false,
            input_volume: 100,
            output_volume: 100,
            partner_volume: 120,
            soundboard_volume: 80,
            pinned_sound_ids: vec!["airhorn".into(), "808-bass".into()],
            enter_to_send: true,
            compact_messages: false,
            show_media_previews: true,
            interface_sounds: true,
            dismissed_notice_id: None,
            voice_hud_enabled: true,
            voice_hud_scale: default_hud_scale(),
            voice_hud_show_names: false,
            keybinds: default_keybinds(),
            interface_sound_volume: default_interface_volume(),
            glass_ui: true,
            window: None,
            wheel_enabled: true,
            wheel_key: default_wheel_key(),
            wheel_hold: false,
            wheel_hold_tenths: default_wheel_hold_tenths(),
            echo_cancellation: true,
            noise_suppression: false,
            theme: AppTheme::DefaultDark,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReactionChip {
    pub emoji: String,
    pub count: usize,
    pub mine: bool,
    /// This user's reaction row, needed to remove it.
    pub mine_row: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChatAttachment {
    None,
    MediaGallery {
        title: String,
        subtitle: String,
        items: usize,
    },
    VideoClip {
        title: String,
        duration: String,
        resolution: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: usize,
    /// Written by the app, such as a call starting, rather than typed by the sender.
    pub system: bool,
    pub name_color: u32,
    pub name_weight: u16,
    pub name_effect: Option<NameEffect>,
    pub media_path: Option<String>,
    pub media_size: Option<(u32, u32)>,
    pub media_bytes: Option<u64>,
    pub db_id: String,
    pub created_at: String,
    pub pending: bool,
    pub sender: String,
    pub initials: String,
    pub avatar_color: u32,
    pub photo: AvatarImage,
    pub role_badge: Option<(String, u32)>,
    pub time: String,
    pub text: String,
    pub reply_to: Option<(String, String)>,
    pub attachment: ChatAttachment,
    pub reactions: Vec<ReactionChip>,
    pub is_self: bool,
    pub pinned: bool,
    pub edited: bool,
    pub deleted: bool,
    pub marker_before: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: String,
    pub media_path: Option<String>,
    pub kind: String, // "photo" | "clip" | "gallery"
    pub title: String,
    pub meta: String,
    pub sender: String,
    pub pinned: bool,
    pub starred: bool,
    pub accent: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartnerConversation {
    pub id: String,
    pub partner_id: String,
    pub name_color: u32,
    pub name_weight: u16,
    pub name_effect: Option<NameEffect>,
    pub username: String,
    pub display_name: String,
    pub initials: String,
    pub color: u32,
    pub photo: AvatarImage,
    pub presence: String, // "online" | "away" | "offline"
    pub status_text: String,
    pub unread_count: usize,
    /// Last message summary from the server, for lists before messages load.
    pub preview: String,
    pub voice_started_at: Option<String>,
    /// Who's really in this conversation's voice room, in join order.
    pub voice_participants: Vec<String>,
    pub messages: Vec<ChatMessage>,
    pub shared_media: Vec<MediaItem>,
    pub loaded: bool,
    pub has_more: bool,
}

impl PartnerConversation {
    /// Shown when there's no conversation to open yet.
    pub fn empty() -> Self {
        Self {
            id: String::new(),
            partner_id: String::new(),
            username: String::new(),
            display_name: "No conversation".into(),
            initials: "–".into(),
            color: 0x282828,
            photo: AvatarImage::default(),
            name_color: 0xf5f5f7,
            name_weight: 500,
            name_effect: None,
            presence: "unknown".into(),
            status_text: "Add a friend to start".into(),
            unread_count: 0,
            preview: String::new(),
            voice_started_at: None,
            voice_participants: vec![],
            messages: vec![],
            shared_media: vec![],
            loaded: true,
            has_more: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FriendRequest {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub initials: String,
    pub color: u32,
    pub incoming: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundClip {
    pub id: String,
    pub name: String,
    pub duration_label: String,
    pub owner_initials: String,
    pub owner_name: String,
    pub is_mine: bool,
    pub duration_ms: u64,
    pub size_bytes: u64,
    pub owner_color: u32,
    pub owner_art: AvatarImage,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PersistedAppState {
    pub preferences: Preferences,
    #[serde(skip)]
    pub auth_status: AuthStatus,
    #[serde(skip)]
    pub profile: UserProfile,
    #[serde(skip)]
    pub active_partner: usize,
    #[serde(skip)]
    pub conversations: Vec<PartnerConversation>,
    #[serde(skip)]
    pub friend_requests: Vec<FriendRequest>,
}

fn state_file_path() -> std::path::PathBuf {
    crate::backend::data_dir().join("preferences.json")
}

pub fn load_state() -> PersistedAppState {
    let path = state_file_path();
    load_state_at(&path)
}

fn load_state_at(path: &std::path::Path) -> PersistedAppState {
    if let Ok(bytes) = std::fs::read(&path)
        && let Ok(parsed) = serde_json::from_slice::<PersistedAppState>(&bytes)
    {
        return parsed;
    }
    std::fs::read(path.with_extension("json.bak")).ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save_state(state: &PersistedAppState) {
    let path = state_file_path();
    if let Err(error) = save_state_at(state, &path) {
        eprintln!("Preferences could not be saved: {error}");
    }
}

fn save_state_at(state: &PersistedAppState, path: &std::path::Path) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(state)?;
    let temp = path.with_extension("json.tmp");
    // Preserve only a valid previous state; don't replace the backup with corruption.
    if std::fs::read(path).ok().is_some_and(|bytes|
        serde_json::from_slice::<PersistedAppState>(&bytes).is_ok()) {
        std::fs::copy(path, path.with_extension("json.bak"))?;
    }
    use std::io::Write;
    let mut file = std::fs::File::create(&temp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temp, path)
}

#[cfg(test)]
mod preference_tests {
    use super::*;

    #[test]
    fn device_routes_survive_save_reload_and_corrupt_primary_recovery() {
        let directory = std::env::temp_dir().join(format!("hero-prefs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("preferences.json");
        let mut state = PersistedAppState::default();
        state.preferences.input_device_id = "input-endpoint-guid".into();
        state.preferences.output_device_id = "output-endpoint-guid".into();
        state.preferences.theme = AppTheme::BabyPink;
        save_state_at(&state, &path).unwrap();
        save_state_at(&state, &path).unwrap(); // also exercises replacement on Windows
        for corrupt in [false, true] {
            if corrupt { std::fs::write(&path, b"{truncated").unwrap(); }
            let restored = load_state_at(&path);
            assert_eq!(restored.preferences.input_device_id, "input-endpoint-guid");
            assert_eq!(restored.preferences.output_device_id, "output-endpoint-guid");
            assert_eq!(restored.preferences.theme, AppTheme::BabyPink);
        }
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(path.with_extension("json.bak")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn missing_fields_do_not_discard_saved_devices() {
        let state: PersistedAppState = serde_json::from_str(r#"{"preferences":{"input_device_id":"saved-input","output_device_id":"saved-output","theme":"deepBlue"}}"#).unwrap();
        assert_eq!(state.preferences.input_device_id, "saved-input");
        assert_eq!(state.preferences.output_device_id, "saved-output");
        assert_eq!(state.preferences.theme, AppTheme::DeepBlue);
        assert_eq!(state.preferences.output_volume, Preferences::default().output_volume);
    }
}

pub fn is_valid_username(username: &str) -> bool {
    let len = username.len();
    (3..=20).contains(&len)
        && username
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub fn initials_for(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "U".into();
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() >= 2 {
        let a = parts[0].chars().next().unwrap_or('U');
        let b = parts[1].chars().next().unwrap_or('U');
        format!("{}{}", a, b).to_uppercase()
    } else {
        trimmed.chars().take(2).collect::<String>().to_uppercase()
    }
}

#[cfg(test)]
mod contrast {
    use super::*;

    fn luminance(hex: u32) -> f64 {
        let channel = |shift: u32| {
            let value = ((hex >> shift) & 0xff) as f64 / 255.;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    fn ratio(a: u32, b: u32) -> f64 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    const ACCENT_LABEL: u32 = 0xffffff;

    /// WCAG AA: 4.5 for text, 3 for indicators. Default Dark's accent is Nitro's existing
    /// colour and intentionally unchanged, so its label pair is reported but not enforced.
    #[test]
    fn themes_meet_aa() {
        let mut failures = vec![];
        for theme in AppTheme::ALL {
            let p = theme.palette();
            let surfaces = [
                ("canvas", p.canvas),
                ("stage", p.stage),
                ("surface", p.surface),
                ("subtle", p.subtle),
                ("panel", p.panel),
                ("raised", p.raised),
                ("selected", p.selected),
            ];
            println!("{}", theme.label());
            for (name, surface) in surfaces {
                let body = ratio(p.text, surface);
                let muted = ratio(p.muted, surface);
                let accent = ratio(p.accent, surface);
                println!("  {name:8} body {body:5.2}  muted {muted:5.2}  accent {accent:5.2}");
                if body < 4.5 || muted < 4.5 {
                    failures.push(format!("{} text on {name}", theme.label()));
                }
            }
            let label = ratio(ACCENT_LABEL, p.accent);
            let indicator = ratio(p.accent_line, p.stage);
            let soft = ratio(p.accent_soft, p.surface);
            let on_selected = ratio(p.accent_soft, p.selected);
            println!("  soft on selected {on_selected:5.2}");
            if on_selected < 4.5 {
                failures.push(format!("{} soft accent on selected {on_selected:.2}", theme.label()));
            }
            println!("  white on accent {label:5.2}  accent on stage {indicator:5.2}  soft on surface {soft:5.2}");
            // Only Default retains its existing iOS-blue/white label exception.
            if theme != AppTheme::DefaultDark && label < 4.5 {
                failures.push(format!("{} white on accent {label:.2}", theme.label()));
            }
            if theme != AppTheme::DefaultDark && p.accent == 0x0a84ff {
                failures.push(format!("{} incorrectly uses the Default accent", theme.label()));
            }
            // Status colours keep their meaning on every surface they appear on.
            for (name, status) in [("green", 0x00d084), ("amber", 0xf5c344)] {
                for (surface_name, surface) in [("surface", p.surface), ("stage", p.stage), ("panel", p.panel)] {
                    let contrast = ratio(status, surface);
                    if contrast < 3. {
                        failures.push(format!("{} {name} on {surface_name} {contrast:.2}", theme.label()));
                    }
                }
            }
            let tab_separation = ratio(p.raised, p.subtle);
            println!("  selected tab / track {tab_separation:.2}");
            if tab_separation < 1.6 {
                failures.push(format!("{} selected tab blends into track {tab_separation:.2}", theme.label()));
            }
            if indicator < 3. {
                failures.push(format!("{} accent on stage {indicator:.2}", theme.label()));
            }
            if soft < 4.5 {
                failures.push(format!("{} soft accent on surface {soft:.2}", theme.label()));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
