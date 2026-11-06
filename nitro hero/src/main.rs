#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio_devices;
mod audio;
mod auth_views;
mod backend;
mod chat;
mod clipboard;
mod components;
#[cfg(debug_assertions)]
mod dev_mcp;
mod hud;
mod state;
mod themes;
mod sync;
mod app_icon;
mod chat_media;
mod sound_file;
mod migrate;
mod updater;
mod hotkeys;
mod realtime;
mod screen_share;
mod screen_view;
mod voice;

use std::{
    borrow::Cow,
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

use components::{
    rgb,
    MATERIAL_DARK, MATERIAL_GREEN, MATERIAL_RED, MATERIAL_SUBTLE, StrokeKind, alert, avatar,
    avatar_group, badge, badge_anchor, button_group, button_group_segment, label,
    material_interactive, material_surface, section, slider, slider_field, split_toggle, tabs,
    ACCENT, SliderHandle,
};
use gpui::{
    App, AppContext, Application, AssetSource, Bounds, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, Pixels, Render, SharedString, StatefulInteractiveElement, Styled, TitlebarOptions, Window,
    StyledImage, WindowBounds, WindowOptions, canvas, div, img, px, rgba, size, svg,
};
use gpui_component::{
    Root, Theme, ThemeMode,
    input::{Copy as InputCopy, Cut, Input, InputEvent, InputState, Paste},
};

struct Icons;
impl AssetSource for Icons {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(match path {
            "ChevronDown" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/ChevronDown.svg"
            ))),
            "ChevronLeft" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/ChevronLeft.svg"
            ))),
            "ChevronRight" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/ChevronRight.svg"
            ))),
            "CodeFork" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/CodeFork.svg"
            ))),
            "Ellipsis" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/Ellipsis.svg"
            ))),
            "Picture" => Some(Cow::Borrowed(include_bytes!("../assets/icons/Picture.svg"))),
            "Pin" => Some(Cow::Borrowed(include_bytes!("../assets/icons/Pin.svg"))),
            "QrCode" => Some(Cow::Borrowed(include_bytes!("../assets/icons/QrCode.svg"))),
            "Star" => Some(Cow::Borrowed(include_bytes!("../assets/icons/Star.svg"))),
            "TextAlignCenter" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/TextAlignCenter.svg"
            ))),
            "TextAlignJustify" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/TextAlignJustify.svg"
            ))),
            "TextAlignLeft" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/TextAlignLeft.svg"
            ))),
            "TextAlignRight" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/TextAlignRight.svg"
            ))),
            "ThumbsDown" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/ThumbsDown.svg"
            ))),
            "Sparkle" => Some(Cow::Borrowed(include_bytes!("../assets/icons/Sparkle.svg"))),
            "SmilePlus" => Some(Cow::Borrowed(include_bytes!("../assets/icons/SmilePlus.svg"))),
            "Pencil" => Some(Cow::Borrowed(include_bytes!("../assets/icons/Pencil.svg"))),
            "Trash" => Some(Cow::Borrowed(include_bytes!("../assets/icons/Trash.svg"))),
            "ThumbsUp" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/ThumbsUp.svg"
            ))),
            "Video" => Some(Cow::Borrowed(include_bytes!("../assets/icons/Video.svg"))),
            "HeroInfoIcon" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/HeroInfoIcon.svg"
            ))),
            "HeroDangerIcon" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/HeroDangerIcon.svg"
            ))),
            "HeroSuccessIcon" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/HeroSuccessIcon.svg"
            ))),
            "HeroWarningIcon" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/HeroWarningIcon.svg"
            ))),
            "HeroSpinner" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/HeroSpinner.svg"
            ))),
            "Bold" => Some(Cow::Borrowed(include_bytes!("../assets/icons/Bold.svg"))),
            "Italic" => Some(Cow::Borrowed(include_bytes!("../assets/icons/Italic.svg"))),
            "Underline" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/Underline.svg"
            ))),
            "Strikethrough" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/Strikethrough.svg"
            ))),
            "WindowClose" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/WindowClose.svg"
            ))),
            "WindowMinimize" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/WindowMinimize.svg"
            ))),
            "WindowMaximize" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/WindowMaximize.svg"
            ))),
            "CallPhone" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/CallPhone.svg"
            ))),
            "CallAudioLines" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/CallAudioLines.svg"
            ))),
            "CallScreenShare" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/CallScreenShare.svg"
            ))),
            "CallScreenShareOff" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/CallScreenShareOff.svg"
            ))),
            "CallMic" => Some(Cow::Borrowed(include_bytes!("../assets/icons/CallMic.svg"))),
            "CallMicOff" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/CallMicOff.svg"
            ))),
            "CallHeadphones" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/CallHeadphones.svg"
            ))),
            "CallHeadphoneOff" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/CallHeadphoneOff.svg"
            ))),
            "CallChevronDown" => Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/CallChevronDown.svg"
            ))),
            "Paperclip" => Some(Cow::Borrowed(
                b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48\"/></svg>",
            )),
            "Send" => Some(Cow::Borrowed(
                b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z\"/><path d=\"m21.854 2.147-10.94 10.939\"/></svg>",
            )),
            "Reply" => Some(Cow::Borrowed(
                b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><polyline points=\"9 17 4 12 9 7\"/><path d=\"M20 18v-2a4 4 0 0 0-4-4H4\"/></svg>",
            )),
            "SidebarRight" => Some(Cow::Borrowed(
                b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><rect width=\"18\" height=\"18\" x=\"3\" y=\"3\" rx=\"2\"/><path d=\"M15 3v18\"/></svg>",
            )),
            "Play" => Some(Cow::Borrowed(
                b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"currentColor\"><polygon points=\"6 3 20 12 6 21 6 3\"/></svg>",
            )),
            "ArrowDown" => Some(Cow::Borrowed(
                b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12 5v14\"/><path d=\"m19 12-7 7-7-7\"/></svg>",
            )),
            "Settings" => Some(Cow::Borrowed(
                b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z\"/><circle cx=\"12\" cy=\"12\" r=\"3\"/></svg>",
            )),
            _ => None,
        })
    }
    fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}


/// The banner on screen. There's one slot; a new banner replaces the old one, as in Nitro.
/// An audio file chosen for the soundboard, before it uses any storage.
pub struct SoundDraft {
    pub file_name: String,
    pub samples: std::sync::Arc<Vec<f32>>,
    pub duration_ms: u32,
    pub start_ms: u32,
    pub end_ms: u32,
}

impl SoundDraft {
    /// How much one slider step is worth. Sliders count in u16, so long files need coarser steps.
    pub fn trim_unit(&self) -> u32 {
        (self.duration_ms / 60_000 + 1) * 10
    }
}

#[derive(Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(updater::Update),
    Downloading(String),
    Failed(String),
}

pub struct ActiveBanner {
    pub severity: components::BannerSeverity,
    pub message: String,
    /// Pushed notices stay until dismissed, and remember it. Others leave after 5 seconds.
    pub notice_id: Option<String>,
    pub shown_at: Instant,
    pub closing_at: Option<Instant>,
}

const BANNER_SLIDE: Duration = Duration::from_millis(320);
const BANNER_AUTO_DISMISS: Duration = Duration::from_secs(5);

pub enum MediaFile {
    Loading,
    Ready { original: std::path::PathBuf, sidebar: Option<std::path::PathBuf> },
    Failed,
}

pub enum SoundboardStatus {
    NotLoaded,
    Loading,
    Loaded,
    Failed(String),
}

pub struct Preview {
    pub view_mode: usize,
    pub state: state::PersistedAppState,
    pub shell_view: state::ShellView,
    pub settings_tab: state::SettingsTab,
    pub auth_tab: state::AuthTab,
    pub auth_error: Option<String>,
    pub auth_email_input: Entity<InputState>,
    pub auth_password_input: Entity<InputState>,
    pub auth_username_input: Entity<InputState>,
    pub auth_display_name_input: Entity<InputState>,
    pub onboarding_step: usize,
    pub onboarding_name_input: Entity<InputState>,
    pub add_friend_input: Entity<InputState>,
    pub call_connected: bool,
    pub call_started: Instant,
    pub call_muted: bool,
    pub call_deafened: bool,
    pub call_streaming: bool,
    pub call_voice_hovered: bool,
    pub call_stream_hovered: bool,
    pub call_notice: Option<&'static str>,
    pub chat_input: Entity<InputState>,
    pub chat_replying_to: Option<(String, String)>,
    pub media_sidebar_open: bool,
    pub media_tabs: components::TabsState,
    pub open_dropdown: Option<state::HeaderDropdown>,
    pub audio_devices: audio_devices::SystemAudioDevices,
    pub soundboard_clips: Vec<state::SoundClip>,
    pub soundboard_filter: usize,
    pub playing_sound_id: Option<String>,
    input: Entity<InputState>,
    dropdown_bounds: Rc<Cell<Bounds<Pixels>>>,
    dropdown_open: bool,
    dropdown_focus: FocusHandle,
    dropdown_index: usize,
    dropdown_result: Option<&'static str>,
    pub toggles: [bool; 4],
    gallery_tabs: components::TabsState,
    pub auth_tabs: components::TabsState,
    alert_visible: bool,
    alert_feedback: Option<&'static str>,
    button_feedback: String,
    pub slider_value: [u8; 2],
    /// Slots 0–1 are the playground; 2–5 are soundboard, input, partner and output volume.
    /// 6 is interface volume, 7 is overlay size.
    pub sliders: [SliderHandle; 10],
    pub user: Option<backend::SignedInUser>,
    pub auth_busy: Option<String>,
    pub sync_error: Option<String>,
    pub friends_error: Option<String>,
    pub sync_generation: u64,
    pub window_active: bool,
    pub placeholder_dirty: bool,
    pub clear_auth_inputs: bool,
    pub chat_reply_target: Option<String>,
    pub chat_scroll: gpui::ScrollHandle,
    pub empty_conversation: state::PartnerConversation,
    pub dev_block_writes: bool,
    pub hud: Option<gpui::WindowHandle<hud::Hud>>,
    pub hud_signature: (Option<Vec<components::OverlayPerson>>, f32, bool),
    pub hud_stream: Option<std::sync::Arc<gpui::RenderImage>>,
    pub recording_keybind: Option<usize>,
    pub renaming_sound: Option<String>,
    pub confirming_delete: Option<String>,
    pub sound_name_input: Entity<InputState>,
    pub voice: voice::Controller,
    pub voice_state: voice::State,
    pub voice_events: Option<std::sync::mpsc::Receiver<voice::State>>,
    pub voice_token_at: Instant,
    /// The conversation the current call belongs to, which may not be the open one.
    pub voice_conversation: Option<String>,
    persist_due: std::cell::Cell<Option<Instant>>,
    pub screen_sink: Option<screen_view::FrameSink>,
    pub screen_frame: Option<std::sync::Arc<gpui::RenderImage>>,
    /// Your own share, shown back to you.
    pub share_sink: Option<screen_view::FrameSink>,
    pub share_frame: Option<std::sync::Arc<gpui::RenderImage>>,
    /// Frames to free from the window's image atlas on the next render.
    pub screen_retired: Vec<std::sync::Arc<gpui::RenderImage>>,
    pub screen_notice_id: u64,
    pub typing: Option<realtime::TypingChannel>,
    pub presence: Option<realtime::PresenceChannel>,
    pub update_checked_at: std::time::Instant,
    pub hotkeys: Option<hotkeys::Hotkeys>,
    pub sound_draft: Option<SoundDraft>,
    pub sound_draft_name: Entity<InputState>,
    pub sound_loading: bool,
    pub sound_saving: bool,
    pub pending_media: Option<chat_media::PendingMedia>,
    pub media_busy: Option<&'static str>,
    pub poll_soon: bool,
    applied_icon_theme: Option<state::AppTheme>,
    /// A name from the chosen file, for the name field on its next render.
    pub pending_sound_name: Option<String>,
    pub update: UpdateState,
    pub update_progress: std::sync::Arc<std::sync::atomic::AtomicU32>,
    /// A chat image to show full size. The cell lets images ask from their own handlers.
    pub lightbox: Option<std::path::PathBuf>,
    pub lightbox_request: std::rc::Rc<std::cell::RefCell<Option<std::path::PathBuf>>>,
    /// The sound this app last played for the partner, to stop it again.
    pub sent_sound: Option<(String, String)>,
    pub remote_sound_seen: u64,
    pub remote_sound_at: Option<Instant>,
    pub remote_sound_nonce: Option<String>,
    pub partner_typing_until: Option<Instant>,
    pub self_typing_until: Option<Instant>,
    /// Which message's reaction picker is open, by database id.
    pub reaction_picker: Option<String>,
    pub editing_message: Option<String>,
    pub edit_input: Entity<InputState>,
    pub banner: Option<ActiveBanner>,
    pub last_notice_id: Option<String>,
    pub banner_shown_error: Option<String>,
    applied_theme: Option<state::AppTheme>,
    /// Debug only: the one conversation the dev MCP may write to.
    pub dev_write_chat: Option<String>,
    pub media_files: std::collections::HashMap<String, MediaFile>,
    pub media_wanted: Rc<std::cell::RefCell<Vec<String>>>,
    pub media_queue: std::collections::VecDeque<String>,
    pub media_in_flight: usize,
    pub wants_older: Rc<Cell<bool>>,
    pub loading_older: bool,
    pub older_anchor: Rc<Cell<Option<(f32, f32)>>>,
    pub history_settled_at: Instant,
    pub history_scrolled_up: bool,
    pub soundboard_status: SoundboardStatus,
    pub playing_since: Option<Instant>,
    pub playing_ms: u64,
    pub clip_request: u64,
    pub last_dropdown: Option<state::HeaderDropdown>,
    pub decoration_catalog: Vec<String>,
    pub decoration_catalog_requested: bool,
    pub decoration_files: std::collections::HashMap<String, Option<std::path::PathBuf>>,
    pub decorations_shown: usize,
}
impl Preview {
    pub fn show_banner(
        &mut self,
        severity: components::BannerSeverity,
        message: impl Into<String>,
        notice_id: Option<String>,
    ) {
        self.banner = Some(ActiveBanner {
            severity,
            message: message.into(),
            notice_id,
            shown_at: Instant::now(),
            closing_at: None,
        });
    }

    fn close_banner(&mut self) {
        if let Some(banner) = &mut self.banner
            && banner.closing_at.is_none()
        {
            banner.closing_at = Some(Instant::now());
        }
    }

    /// Advances the banner's slide and auto-dismiss. Returns true while it needs repainting.
    fn tick_banner(&mut self) -> bool {
        let Some(banner) = &self.banner else {
            return false;
        };
        if let Some(closing) = banner.closing_at {
            if closing.elapsed() >= BANNER_SLIDE {
                self.banner = None;
            }
            return true;
        }
        if banner.notice_id.is_none() && banner.shown_at.elapsed() >= BANNER_AUTO_DISMISS {
            self.close_banner();
            return true;
        }
        banner.shown_at.elapsed() < BANNER_SLIDE
    }

    fn render_banner(&mut self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let banner = self.banner.as_ref()?;
        let eased = |elapsed: Duration| {
            let t = (elapsed.as_secs_f32() / BANNER_SLIDE.as_secs_f32()).clamp(0., 1.);
            1. - (1. - t).powi(3)
        };
        let open = match banner.closing_at {
            Some(closing) => 1. - eased(closing.elapsed()),
            None => eased(banner.shown_at.elapsed()),
        };
        let mut actions = vec![];
        if let Some(id) = banner.notice_id.clone() {
            actions.push(
                material_interactive(
                    "banner-dismiss",
                    "banner-dismiss",
                    MATERIAL_DARK,
                    14.,
                    StrokeKind::Neutral,
                )
                .h(px(32.))
                .px(px(14.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .text_size(px(12.5))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(rgb(0xf2f2f3))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.state.preferences.dismissed_notice_id = Some(id.clone());
                    this.persist();
                    this.close_banner();
                    cx.notify();
                }))
                .child("Dismiss"),
            );
        }
        // The banner pushes the app down as it slides in, so it never covers the header.
        Some(
            div()
                .flex_shrink_0()
                .w_full()
                .h(px(components::TOP_BANNER_HEIGHT * open))
                .overflow_hidden()
                .child(components::top_banner(
                    banner.severity,
                    banner.message.clone(),
                    None,
                    actions,
                )),
        )
    }

    /// Saves shortly after the last change, so dragging a slider doesn't sync a file
    /// on every mouse move.
    pub fn persist(&self) {
        self.persist_due.set(Some(Instant::now() + Duration::from_millis(400)));
    }

    pub fn flush_saved_state(&self) {
        self.flush_persist(true);
    }

    fn flush_persist(&self, force: bool) {
        if self.persist_due.get().is_some_and(|due| force || Instant::now() >= due) {
            self.persist_due.set(None);
            state::save_state(&self.state);
        }
    }

    pub fn switch_partner(&mut self, idx: usize, _window: &mut Window, cx: &mut Context<Self>) {
        if idx < self.state.conversations.len() {
            self.state.active_partner = idx;
            self.state.conversations[idx].unread_count = 0;
            self.chat_replying_to = None;
            self.chat_reply_target = None;
            self.placeholder_dirty = true;
            self.chat_scroll.scroll_to_bottom();
            self.history_settled_at = Instant::now();
            self.load_active(cx);
        }
    }

    fn all_inputs(&self) -> [&Entity<InputState>; 9] {
        [
            &self.sound_name_input,
            &self.input,
            &self.chat_input,
            &self.auth_email_input,
            &self.auth_password_input,
            &self.auth_username_input,
            &self.auth_display_name_input,
            &self.onboarding_name_input,
            &self.add_friend_input,
        ]
    }

    /// Paste for every input, using a clipboard reader that survives a busy clipboard
    /// and clipboards that list an image format ahead of text.
    fn paste_into_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let composer_focused = self.chat_input.read(cx).focus_handle(cx).is_focused(window);
        // A copied file or a bare image attaches. Text wins when both are offered, since
        // apps like spreadsheets put a picture of the same text on the clipboard.
        let text = clipboard::read_text().filter(|text| !text.is_empty());
        if composer_focused && text.is_none() {
            if let Some(media) = clipboard::read_media() {
                self.attach_clipboard_media(media, cx);
            }
            return;
        }
        let Some(text) = text else {
            return;
        };
        let text = text.replace(['\r', '\n'], "");
        let focused = self
            .all_inputs()
            .into_iter()
            .find(|input| input.read(cx).focus_handle(cx).is_focused(window))
            .cloned();
        if let Some(input) = focused {
            input.update(cx, |state, cx| state.replace(text, window, cx));
        }
    }

    pub fn slider_change(&self, slot: usize, cx: &mut Context<Self>) -> Rc<dyn Fn(&u16, &mut Window, &mut App)> {
        self.slider_on_change(slot, cx)
    }

    fn slider_on_change(
        &self,
        slot: usize,
        cx: &mut Context<Self>,
    ) -> Rc<dyn Fn(&u16, &mut Window, &mut App)> {
        Rc::new(cx.listener(move |this, value: &u16, _, cx| {
            if slot == 8 || slot == 9 {
                this.set_sound_trim(slot == 8, *value as u32);
                cx.notify();
                return;
            }
            let value = *value as u8;
            match slot {
                0 | 1 => this.slider_value[slot] = value,
                2 => {
                    this.state.preferences.soundboard_volume = value;
                    audio::set_clip_volume(value);
                }
                3 => this.state.preferences.input_volume = value,
                4 => {
                    this.state.preferences.partner_volume = value;
                    this.send_output_level();
                }
                5 => {
                    this.state.preferences.output_volume = value;
                    this.send_output_level();
                }
                6 => this.state.preferences.interface_sound_volume = value,
                7 => this.state.preferences.voice_hud_scale = value.max(50),
                _ => {}
            }
            if slot >= 2 {
                this.persist();
            }
            cx.notify();
        }))
    }

    /// The shared slider for a given slot.
    pub fn slider_for(
        &self,
        slot: usize,
        value: u8,
        max: u16,
        step: u16,
        width: f32,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        slider(
            slot,
            &self.sliders[slot],
            value as u16,
            max,
            step,
            width,
            self.slider_on_change(slot, cx),
        )
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut loaded_state = state::load_state();
        if !loaded_state.preferences.legacy_imported {
            migrate::import_preferences(&mut loaded_state.preferences);
            loaded_state.preferences.legacy_imported = true;
            state::save_state(&loaded_state);
        }

        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Enter your name"));
        let chat_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Message…")
        });
        let edit_input = cx.new(|cx| InputState::new(window, cx));
        let sound_draft_name = cx.new(|cx| InputState::new(window, cx).placeholder("Name this sound"));
        let auth_email_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("you@nitro.chat"));
        let auth_password_input =
            cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("••••••••••••"));
        let auth_username_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("nyxnie"));
        let auth_display_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Display name"));
        let onboarding_name_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("How should your partner see you?")
        });
        let sound_name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Sound name"));
        let add_friend_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Add by username (e.g. sorai)…")
        });

        cx.spawn(async move |view, cx| {
            loop {
                gpui::Timer::after(Duration::from_secs(1)).await;
                if view
                    .update(cx, |this, cx| {
                        if this.call_connected {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        // A watched screen updates at its own rate, far faster than the 20 fps tick below.
        cx.spawn(async move |view, cx| {
            loop {
                gpui::Timer::after(Duration::from_millis(16)).await;
                let alive = view.update(cx, |this, cx| {
                    if this.pump_screen_frames(cx) {
                        this.sync_overlay(cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.on_app_quit(|this, _| {
            this.flush_persist(true);
            async {}
        })
        .detach();
        // Animated names and decorations repaint at 20 fps, and only while one is on screen
        // and the window is focused.
        cx.spawn(async move |view, cx| {
            loop {
                gpui::Timer::after(Duration::from_millis(50)).await;
                let alive = view.update(cx, |this, cx| {
                    if this.window_active && components::take_effect_on_screen() {
                        cx.notify();
                    }
                    this.flush_persist(false);
                    this.pump_voice(cx);
                    this.pump_typing(cx);
                    this.pump_presence(cx);
                    // People leave this app open for days, and both sides of a call need
                    // to be on a compatible version.
                    if this.update_checked_at.elapsed() >= Duration::from_secs(1800) {
                        this.update_checked_at = std::time::Instant::now();
                        if matches!(this.update, UpdateState::Idle) {
                            this.check_for_update(false, cx);
                        }
                    }
                    this.pump_hotkeys(cx);
                    if let Some(path) = this.lightbox_request.borrow_mut().take() {
                        this.lightbox = Some(path);
                        cx.notify();
                    }
                    if matches!(this.update, UpdateState::Downloading(_)) {
                        cx.notify();
                    }
                    this.sync_overlay(cx);
                    if this.tick_banner() {
                        cx.notify();
                    }
                    if let Some(started) = this.playing_since {
                        if started.elapsed().as_millis() as u64 >= this.playing_ms {
                            this.playing_since = None;
                            this.playing_sound_id = None;
                        }
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        // The composer reacts to what's typed, such as showing commands after a slash.
        cx.subscribe(&chat_input, |this: &mut Self, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let has_text = !input.read(cx).value().trim().is_empty();
                this.composer_changed(has_text);
                cx.notify();
            }
        })
        .detach();
        cx.observe_window_activation(window, |this, window, cx| {
            this.window_active = window.is_window_active();
            cx.notify();
        })
        .detach();
        #[cfg(debug_assertions)]
        if dev_mcp::enabled() {
            dev_mcp::attach(window, cx);
        }
        cx.defer_in(window, |this, _, cx| {
            this.check_for_update(false, cx);
            this.restore_saved_session(cx);
            this.refresh_audio_devices(cx);
        });
        Self {
            view_mode: 0,
            state: loaded_state,
            shell_view: state::ShellView::Chat,
            settings_tab: state::SettingsTab::General,
            auth_tab: state::AuthTab::Login,
            auth_error: None,
            auth_email_input,
            auth_password_input,
            auth_username_input,
            auth_display_name_input,
            onboarding_step: 0,
            onboarding_name_input,
            add_friend_input,
            call_connected: false,
            call_started: Instant::now(),
            call_muted: false,
            call_deafened: false,
            call_streaming: false,
            call_voice_hovered: false,
            call_stream_hovered: false,
            call_notice: None,
            chat_input,
            chat_replying_to: None,
            media_sidebar_open: true,
            media_tabs: components::TabsState::new(0),
            open_dropdown: None,
            audio_devices: audio_devices::enumerate_audio_devices(),
            soundboard_clips: vec![],
            soundboard_filter: 0,
            playing_sound_id: None,
            input,
            dropdown_bounds: Rc::new(Cell::new(Bounds::default())),
            dropdown_open: false,
            dropdown_focus: cx.focus_handle().tab_stop(true),
            dropdown_index: 0,
            dropdown_result: None,
            toggles: [false; 4],
            gallery_tabs: components::TabsState::new(0),
            auth_tabs: components::TabsState::new(0),
            alert_visible: true,
            alert_feedback: None,
            button_feedback: "Choose a button".into(),
            slider_value: [30, 40],
            sliders: std::array::from_fn(|_| SliderHandle {
                bounds: Rc::new(Cell::new(Bounds::default())),
                focus: cx.focus_handle().tab_stop(true),
                motion: Default::default(),
            }),
            user: None,
            auth_busy: None,
            sync_error: None,
            friends_error: None,
            sync_generation: 0,
            window_active: window.is_window_active(),
            placeholder_dirty: false,
            clear_auth_inputs: false,
            chat_reply_target: None,
            chat_scroll: gpui::ScrollHandle::new(),
            empty_conversation: state::PartnerConversation::empty(),
            dev_block_writes: false,
            hud: None,
            hud_signature: (None, 0., false),
            hud_stream: None,
            recording_keybind: None,
            renaming_sound: None,
            confirming_delete: None,
            sound_name_input,
            voice: Default::default(),
            voice_state: Default::default(),
            voice_events: None,
            voice_token_at: Instant::now(),
            voice_conversation: None,
            persist_due: Default::default(),
            screen_sink: None,
            screen_frame: None,
            share_sink: None,
            share_frame: None,
            screen_retired: Vec::new(),
            screen_notice_id: 0,
            typing: None,
            presence: None,
            update_checked_at: std::time::Instant::now(),
            hotkeys: None,
            sound_draft: None,
            sound_draft_name,
            sound_loading: false,
            sound_saving: false,
            pending_media: None,
            media_busy: None,
            poll_soon: false,
            applied_icon_theme: None,
            pending_sound_name: None,
            update: UpdateState::Idle,
            update_progress: Default::default(),
            lightbox: None,
            lightbox_request: Default::default(),
            sent_sound: None,
            remote_sound_seen: 0,
            remote_sound_at: None,
            remote_sound_nonce: None,
            partner_typing_until: None,
            self_typing_until: None,
            reaction_picker: None,
            editing_message: None,
            edit_input,
            banner: None,
            last_notice_id: None,
            banner_shown_error: None,
            applied_theme: None,
            #[cfg(debug_assertions)]
            dev_write_chat: std::env::args()
                .find_map(|arg| arg.strip_prefix("--dev-mcp-write-chat=").map(str::to_string)),
            #[cfg(not(debug_assertions))]
            dev_write_chat: None,
            media_files: Default::default(),
            media_wanted: Default::default(),
            media_queue: Default::default(),
            media_in_flight: 0,
            wants_older: Default::default(),
            loading_older: false,
            older_anchor: Default::default(),
            history_settled_at: Instant::now(),
            history_scrolled_up: false,
            soundboard_status: SoundboardStatus::NotLoaded,
            playing_since: None,
            playing_ms: 0,
            clip_request: 0,
            last_dropdown: None,
            decoration_catalog: vec![],
            decoration_catalog_requested: false,
            decoration_files: Default::default(),
            decorations_shown: 24,
        }
    }

    #[inline(never)]
    fn render_gallery(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let primary = tabs(
            "primary-tab",
            vec!["Overview".into(), "Activity".into(), "Settings".into()],
            &self.gallery_tabs,
            92.,
            window,
            Rc::new(cx.listener(|this, index: &usize, _, cx| {
                this.gallery_tabs.select(*index);
                cx.notify();
            })),
        );
        let sliders = div()
            .flex()
            .flex_wrap()
            .gap(px(32.))
            .child(slider_field(
                "Volume",
                format!("{}", self.slider_value[0]),
                320.,
                self.slider_for(0, self.slider_value[0], 100, 1, 320., cx),
            ))
            .child(slider_field(
                "Volume · steps of 5",
                format!("{}", self.slider_value[1]),
                320.,
                self.slider_for(1, self.slider_value[1], 100, 5, 320., cx),
            ));
        let mut buttons = div().flex().flex_wrap().gap(px(10.));
        for (index, (name, bg, fg, stroke)) in [
            ("Primary", ACCENT, 0xffffff, StrokeKind::Colored),
            ("Secondary", MATERIAL_DARK, 0xf2f2f3, StrokeKind::Neutral),
            ("Tertiary", MATERIAL_SUBTLE, 0xd9d9d9, StrokeKind::Neutral),
            ("Danger", MATERIAL_RED, 0xffffff, StrokeKind::Colored),
            ("Danger soft", 0x48252e, 0xffb3be, StrokeKind::Colored),
        ]
        .into_iter()
        .enumerate()
        {
            buttons = buttons.child(
                material_interactive(
                    ("example-button", index),
                    format!("example-button-{index}"),
                    bg,
                    25.01,
                    stroke,
                )
                .h(px(40.))
                .px(px(16.))
                .text_color(rgb(fg))
                .text_size(px(14.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .flex()
                .items_center()
                .justify_center()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.button_feedback = format!("{} pressed", name);
                    cx.notify();
                }))
                .child(name),
            );
        }
        // Mirrors HeroUI's official ButtonGroup basic examples; each segment is an action.
        let examples: &[(&str, u32, &[(&str, &str, &str)])] = &[
            (
                "merge",
                ACCENT,
                &[("", "Merge pull request", ""), ("", "", "ChevronDown")],
            ),
            (
                "fork",
                MATERIAL_DARK,
                &[("CodeFork", "Fork", "24"), ("", "", "ChevronDown")],
            ),
            (
                "qr",
                MATERIAL_DARK,
                &[("QrCode", "", ""), ("", "Scan to pay", "")],
            ),
            (
                "likes",
                MATERIAL_DARK,
                &[("ThumbsUp", "2.4K", ""), ("ThumbsDown", "", "")],
            ),
            ("star", MATERIAL_DARK, &[("Star", "Star", ""), ("", "104", "")]),
            (
                "pinned",
                MATERIAL_DARK,
                &[("Pin", "Pinned", ""), ("", "", "ChevronDown")],
            ),
            (
                "navigation",
                MATERIAL_DARK,
                &[
                    ("ChevronLeft", "Previous", ""),
                    ("", "Next", "ChevronRight"),
                ],
            ),
            (
                "media",
                MATERIAL_DARK,
                &[
                    ("Picture", "Photos", ""),
                    ("Video", "Videos", ""),
                    ("Ellipsis", "", ""),
                ],
            ),
            (
                "text",
                MATERIAL_DARK,
                &[("", "Left", ""), ("", "Center", ""), ("", "Right", "")],
            ),
            (
                "align",
                MATERIAL_DARK,
                &[
                    ("TextAlignLeft", "", ""),
                    ("TextAlignCenter", "", ""),
                    ("TextAlignRight", "", ""),
                    ("TextAlignJustify", "", ""),
                ],
            ),
        ];
        let mut groups = div().flex().flex_wrap().items_center().gap(px(10.));
        for (group, background, parts) in examples.iter().copied() {
            let stroke = if background == ACCENT {
                StrokeKind::Colored
            } else {
                StrokeKind::Neutral
            };
            let mut row = button_group(background, stroke);
            for (index, (leading, text, trailing)) in parts.iter().copied().enumerate() {
                let mut button = button_group_segment(
                    (group, index),
                    index,
                    parts.len(),
                    if background == ACCENT { state::palette().accent_line } else { 0x525259 },
                )
                .px(px(if text.is_empty() { 10. } else { 14. }))
                .text_color(rgb(if background == ACCENT { 0xffffff } else { 0xf1f1f3 }));
                if !leading.is_empty() {
                    button = button.child(svg().path(leading).size(px(16.)).text_color(rgb(
                        if background == ACCENT {
                            0xffffff
                        } else {
                            0xf1f1f3
                        },
                    )));
                }
                if !text.is_empty() {
                    button = button.child(text);
                }
                if trailing == "24" {
                    button = button.child(
                        div()
                            .px(px(5.))
                            .rounded_full()
                            .bg(rgb(state::palette().raised))
                            .text_size(px(12.))
                            .text_color(rgb(state::palette().accent_soft))
                            .child(trailing),
                    );
                } else if !trailing.is_empty() {
                    button = button.child(svg().path(trailing).size(px(16.)).text_color(rgb(
                        if background == ACCENT {
                            0xffffff
                        } else {
                            0xf1f1f3
                        },
                    )));
                }
                button = button.on_click(cx.listener(move |this, _, _, cx| {
                    this.button_feedback = format!(
                        "{} · {}",
                        group,
                        if text.is_empty() { "option" } else { text }
                    );
                    cx.notify();
                }));
                row = row.child(button);
            }
            groups = groups.child(row);
        }
        // This is an isolated visual preview: it does not join a voice room or touch devices.
        let call_icon = |name: &'static str, size: f32, color: u32| {
            svg().path(name).size(px(size)).text_color(rgb(color))
        };
        let mut call_row = div().flex().items_center().gap(px(18.));
        call_row = call_row.child(
            div()
                .id("call-avatar")
                .relative()
                .size(px(40.))
                .flex_shrink_0()
                .cursor_pointer()
                .child(avatar("", 40., MATERIAL_DARK, 0xffffff, true))
                .child(
                    material_surface(0x00d084, 6.5, StrokeKind::Colored)
                        .absolute()
                        .right(px(-2.))
                        .bottom(px(-2.))
                        .size(px(13.)),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.call_notice = Some("Conversation switcher · preview");
                    cx.notify();
                })),
        );
        if self.call_connected {
            call_row = call_row.child(components::call_pill(
                "preview-voice",
                vec![
                    ("".into(), MATERIAL_DARK, true.into()),
                    ("N".into(), 0x3d4a58, false.into()),
                ],
                self.call_started.elapsed().as_secs(),
                self.call_voice_hovered,
                cx.listener(|this, hovered: &bool, _, cx| {
                    this.call_voice_hovered = *hovered;
                    cx.notify();
                }),
                cx.listener(|this, _, _, cx| {
                    audio::play(audio::UiSound::VoiceLeave);
                    this.call_connected = false;
                    this.call_voice_hovered = false;
                    this.call_notice = Some("Call ended · preview");
                    cx.notify();
                }),
            ));
            call_row = call_row.child(
                material_interactive(
                    "preview-soundboard",
                    "preview-soundboard",
                    MATERIAL_DARK,
                    25.01,
                    StrokeKind::Neutral,
                )
                .w(px(62.))
                .h(px(40.))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(5.))
                .child(call_icon("CallAudioLines", 20., 0xd4d4d8))
                .child(call_icon("CallChevronDown", 14., 0xd4d4d8))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.call_notice = Some("Soundboard · preview");
                    cx.notify();
                })),
            );
            let stream_fill = if self.call_streaming {
                if self.call_stream_hovered {
                    MATERIAL_RED
                } else {
                    MATERIAL_GREEN
                }
            } else {
                MATERIAL_DARK
            };
            call_row = call_row.child(
                material_interactive(
                    "preview-stream",
                    "preview-stream",
                    stream_fill,
                    25.01,
                    if self.call_streaming {
                        StrokeKind::Colored
                    } else {
                        StrokeKind::Neutral
                    },
                )
                .w(px(40.))
                .h(px(40.))
                .flex()
                .items_center()
                .justify_center()
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    this.call_stream_hovered = *hovered;
                    cx.notify();
                }))
                .child(call_icon(
                    if self.call_streaming {
                        "CallScreenShareOff"
                    } else {
                        "CallScreenShare"
                    },
                    20.,
                    if self.call_streaming {
                        0xffffff
                    } else {
                        0xd4d4d8
                    },
                ))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.call_streaming = !this.call_streaming;
                    this.call_notice = Some(if this.call_streaming {
                        "Streaming enabled · preview"
                    } else {
                        "Streaming stopped · preview"
                    });
                    cx.notify();
                })),
            );
            for (index, icon, disabled) in [
                (
                    0usize,
                    if self.call_muted {
                        "CallMicOff"
                    } else {
                        "CallMic"
                    },
                    self.call_muted,
                ),
                (
                    1,
                    if self.call_deafened {
                        "CallHeadphoneOff"
                    } else {
                        "CallHeadphones"
                    },
                    self.call_deafened,
                ),
            ] {
                let icon_color = if disabled { 0xffffff } else { 0xd4d4d8 };
                call_row = call_row.child(split_toggle(
                    "preview-audio",
                    index,
                    icon,
                    if disabled { MATERIAL_RED } else { MATERIAL_DARK },
                    if disabled { StrokeKind::Colored } else { StrokeKind::Neutral },
                    icon_color,
                    cx.listener(move |this, _, _, cx| {
                        if index == 0 {
                            this.call_muted = !this.call_muted;
                            audio::play(if this.call_muted {
                                audio::UiSound::MuteOn
                            } else {
                                audio::UiSound::MuteOff
                            });
                            this.call_notice = Some(if this.call_muted {
                                "Microphone muted · preview"
                            } else {
                                "Microphone enabled · preview"
                            });
                        } else {
                            this.call_deafened = !this.call_deafened;
                            audio::play(if this.call_deafened {
                                audio::UiSound::MuteOn
                            } else {
                                audio::UiSound::MuteOff
                            });
                            this.call_notice = Some(if this.call_deafened {
                                "Audio deafened · preview"
                            } else {
                                "Audio enabled · preview"
                            });
                        }
                        cx.notify();
                    }),
                    cx.listener(move |this, _, _, cx| {
                        this.call_notice = Some(if index == 0 {
                            "Microphone settings · preview"
                        } else {
                            "Audio settings · preview"
                        });
                        cx.notify();
                    }),
                ));
            }
        } else {
            call_row = call_row
                .child(components::voice_button(
                    "preview-voice",
                    "Voice",
                    vec![],
                    cx.listener(|this, _, _, cx| {
                        audio::play(audio::UiSound::VoiceJoin);
                        this.call_connected = true;
                        this.call_started = Instant::now();
                        this.call_notice = Some("Connected · preview");
                        cx.notify();
                    }),
                ))
                .child(components::voice_button(
                    "preview-voice-join",
                    "Join",
                    vec![("N".into(), 0x3d4a58, Default::default())],
                    cx.listener(|this, _, _, cx| {
                        audio::play(audio::UiSound::VoiceJoin);
                        this.call_connected = true;
                        this.call_started = Instant::now();
                        this.call_notice = Some("Connected · preview");
                        cx.notify();
                    }),
                ));
        }
        let call_preview = material_surface(0x161618, 16., StrokeKind::Neutral)
            .w_full()
            .h(px(112.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .child(
                div()
                    .id("call-preview-scroll")
                    .w_full()
                    .overflow_x_scroll()
                    .child(
                        div()
                            .w_full()
                            .min_w(px(490.))
                            .py(px(4.))
                            .flex()
                            .justify_center()
                            .child(call_row),
                    ),
            )
            .child(label(
                self.call_notice.unwrap_or("Preview only · no live call"),
            ));
        let mut toggle_group = material_surface(MATERIAL_DARK, 25.01, StrokeKind::Neutral)
            .flex()
            .items_center()
            .p(px(1.25));
        for (index, icon) in ["Bold", "Italic", "Underline", "Strikethrough"]
            .into_iter()
            .enumerate()
        {
            let selected = self.toggles[index];
            let mut button = div()
                .id(("format-toggle", index))
                .relative()
                .cursor_pointer()
                .size(px(37.5))
                .flex()
                .items_center()
                .justify_center()
                .bg(if selected {
                    rgba((state::palette().accent << 8) | 0x38)
                } else {
                    rgba(0x00000000)
                })
                .hover(|s| {
                    s.bg(if selected {
                        rgba((state::palette().accent << 8) | 0x50)
                    } else {
                        rgba(0xffffff18)
                    })
                })
                .child(svg().path(icon).size(px(16.)).text_color(rgb(if selected {
                    0x80bfff
                } else {
                    0xe8e8ea
                })));
            if index == 0 {
                button = button.rounded_l(px(18.75));
            }
            if index == 3 {
                button = button.rounded_r(px(18.75));
            }
            if index > 0 {
                button = button.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(9.5))
                        .w(px(1.))
                        .h(px(18.))
                        .bg(rgb(0x525259)),
                );
            }
            toggle_group =
                toggle_group.child(button.on_click(cx.listener(move |this, _, _, cx| {
                    this.toggles[index] = !this.toggles[index];
                    cx.notify();
                })));
        }
        let dropdown_focus = self.dropdown_focus.clone();
        let trigger_bounds = self.dropdown_bounds.clone();
        let mut dropdown = div().flex().flex_col().items_start().gap(px(4.)).child(
            material_interactive(
                "dropdown-trigger",
                "dropdown-trigger",
                MATERIAL_DARK,
                25.01,
                StrokeKind::Neutral,
            )
            .track_focus(&self.dropdown_focus)
            .h(px(40.))
            .px(px(16.))
            .text_color(rgb(0x80bfff))
            .text_size(px(14.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |this, _, window, cx| {
                this.dropdown_open = !this.dropdown_open;
                window.focus(&dropdown_focus);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "escape" if this.dropdown_open => this.dropdown_open = false,
                    "down" => {
                        this.dropdown_index = if this.dropdown_open {
                            (this.dropdown_index + 1).min(3)
                        } else {
                            0
                        };
                        this.dropdown_open = true;
                    }
                    "up" => {
                        this.dropdown_open = true;
                        this.dropdown_index = this.dropdown_index.saturating_sub(1);
                    }
                    "enter" | "space" if this.dropdown_open => {
                        this.dropdown_result = Some(
                            ["New file", "Copy link", "Edit file", "Delete file"]
                                [this.dropdown_index],
                        );
                        this.dropdown_open = false;
                    }
                    "enter" | "space" => this.dropdown_open = true,
                    _ => return,
                }
                cx.notify();
                cx.stop_propagation();
            }))
            .child("Actions")
            .child(
                canvas(
                    move |bounds, _, _| trigger_bounds.set(bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            ),
        );
        if self.dropdown_open {
            let mut menu = material_surface(0x252626, 18., StrokeKind::Neutral)
                .w(px(220.))
                .p(px(6.))
                .flex()
                .flex_col();
            for (index, name) in ["New file", "Copy link", "Edit file", "Delete file"]
                .into_iter()
                .enumerate()
            {
                menu = menu.child(
                    div()
                        .id(("dropdown-item", index))
                        .cursor_pointer()
                        .h(px(36.))
                        .px(px(12.))
                        .rounded(px(12.))
                        .flex()
                        .items_center()
                        .text_size(px(14.))
                        .text_color(rgb(if index == 3 { 0xff858d } else { 0xf2f2f3 }))
                        .hover(|s| s.bg(rgba(0xffffff1a)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.dropdown_result = Some(name);
                            this.dropdown_open = false;
                            this.dropdown_index = index;
                            cx.notify();
                        }))
                        .child(name),
                );
            }
            let trigger = self.dropdown_bounds.get();
            dropdown =
                dropdown.child(
                    gpui::deferred(
                        gpui::anchored()
                            .position(gpui::point(trigger.left(), trigger.bottom() + px(6.)))
                            .snap_to_window_with_margin(px(8.))
                            .child(menu.id("dropdown-menu").occlude().on_mouse_down_out(
                                cx.listener(|this, _, _, cx| {
                                    this.dropdown_open = false;
                                    cx.notify();
                                }),
                            )),
                    )
                    .with_priority(1),
                );
        }
        dropdown = dropdown.child(
            div().h(px(16.)).child(label(
                self.dropdown_result
                    .map(|name| format!("Selected: {name}"))
                    .unwrap_or_default(),
            )),
        );
        let input_focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let name_input = material_surface(
            MATERIAL_DARK,
            25.01,
            if input_focused {
                StrokeKind::AccentFocus
            } else {
                StrokeKind::Neutral
            },
        )
        .w(px(256.))
        .h(px(40.))
        .px(px(6.))
        .flex()
        .items_center()
        .child(
            Input::new(&self.input)
                .appearance(false)
                .w_full()
                .text_size(px(14.)),
        );
        let action = |id: &'static str, text: &'static str, bg: u32, fg: u32| {
            material_interactive(id, id, bg, 25.01, StrokeKind::Colored)
                .flex_shrink_0()
                .h(px(34.))
                .px(px(14.))
                .text_color(rgb(fg))
                .text_size(px(14.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .flex()
                .items_center()
                .justify_center()
                .child(text)
        };
        let mut alerts = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .w(px(576.))
            .max_w_full()
            .child(alert(
                "HeroInfoIcon",
                "New features available",
                "Check out the latest updates and improvements.",
                0xf2f2f3,
            ))
            .child(
                alert(
                    "HeroInfoIcon",
                    "Update available",
                    "A new version is available with the latest features and fixes.",
                    0x80bfff,
                )
                .child(
                    action("alert-refresh", "Refresh", ACCENT, 0xffffff).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.alert_feedback = Some("Refresh selected");
                            cx.notify();
                        },
                    )),
                ),
            )
            .child(
                alert(
                    "HeroDangerIcon",
                    "Unable to connect to server",
                    "Check your connection and try again.",
                    0xff858d,
                )
                .child(
                    action("alert-retry", "Retry", MATERIAL_RED, 0xffffff).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.alert_feedback = Some("Retry selected");
                            cx.notify();
                        },
                    )),
                ),
            );
        if self.alert_visible {
            alerts = alerts.child(
                alert(
                    "HeroSuccessIcon",
                    "Profile updated successfully",
                    "",
                    0x71dba2,
                )
                .child(
                    material_interactive(
                        "dismiss-alert",
                        "dismiss-alert",
                        MATERIAL_DARK,
                        12.,
                        StrokeKind::Neutral,
                    )
                    .size(px(24.))
                    .flex_shrink_0()
                    .text_color(rgb(0xc2c2c8))
                    .text_size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.alert_visible = false;
                        cx.notify();
                    }))
                    .child("×"),
                ),
            );
        } else {
            alerts = alerts.child(
                div()
                    .id("restore-alert")
                    .cursor_pointer()
                    .text_size(px(13.))
                    .text_color(rgb(0x80bfff))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.alert_visible = true;
                        cx.notify();
                    }))
                    .child("Restore success alert"),
            );
        }
        alerts = alerts
            .child(alert(
                "HeroSpinner",
                "Processing your request",
                "Please wait while your data is synced.",
                0x80bfff,
            ))
            .child(alert(
                "HeroWarningIcon",
                "Scheduled maintenance",
                "Services will be unavailable during scheduled maintenance.",
                0xf6c65d,
            ));
        if let Some(feedback) = self.alert_feedback {
            alerts = alerts.child(label(feedback));
        }
        div()
            .id("gallery")
            .w_full()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .bg(rgb(state::palette().canvas))
            .text_color(rgb(0xf3f3f4))
            .font_family("Inter")
            .child(
                div()
                            .max_w(px(920.))
                            .mx_auto()
                            .px(px(36.))
                            .py(px(32.))
                            .flex()
                            .flex_col()
                            .gap(px(30.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(div().text_size(px(23.)).child("Component preview"))
                                    .child(label("Native GPUI recreations · HeroUI v3 reference")),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.))
                                    .child(section(
                                        "Call header",
                                        "Nitro voice controls · interactive preview",
                                    ))
                                    .child(call_preview),
                            )
                            .child(div().h(px(1.)).w_full().bg(rgb(0x29292c)))
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(56.))
                                    .child(
                                        div()
                                            .min_w(px(220.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(16.))
                                            .child(section("Avatar", "Sizes, fallback and image"))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(12.))
                                                    .child(avatar(
                                                        "AN", 32., 0x343438, 0xe4e4e7, false,
                                                    ))
                                                    .child(avatar(
                                                        "JD", 40., 0x343438, 0xe4e4e7, false,
                                                    ))
                                                    .child(avatar(
                                                        "SK", 48., 0x343438, 0xe4e4e7, false,
                                                    ))
                                                    .child(avatar(
                                                        "", 48., 0x343438, 0xffffff, true,
                                                    ))
                                                    .child(avatar(
                                                        "SK",
                                                        48.,
                                                        0x343438,
                                                        0xe4e4e7,
                                                        components::AvatarImage {
                                                            path: None,
                                                            decoration: Some(
                                                                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                                                                    .join("assets/decoration-sample.png"),
                                                            ),
                                                        },
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap(px(10.))
                                                    .child(avatar(
                                                        "A", 40., 0x163e60, 0xa8d5ff, false,
                                                    ))
                                                    .child(avatar(
                                                        "S", 40., 0x203f32, 0x81e1ab, false,
                                                    ))
                                                    .child(avatar(
                                                        "W", 40., 0x4a3a1d, 0xfbd77c, false,
                                                    ))
                                                    .child(avatar(
                                                        "D", 40., 0x4a252e, 0xff9baa, false,
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(220.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(16.))
                                            .child(section(
                                                "Avatar group",
                                                "Overlapping avatars and overflow",
                                            ))
                                            .child(avatar_group())
                                            .child(label("4 visible · 3 more")),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(220.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(16.))
                                            .child(section("Badge", "Counts, labels and status"))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child(badge("3", ACCENT, 0xffffff))
                                                    .child(badge("12", 0x343438, 0xf1f1f3))
                                                    .child(badge("99+", 0x343438, 0xf1f1f3))
                                                    .child(badge("New", 0x173d5c, 0xb1d9ff)),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(28.))
                                                    .pt(px(4.))
                                                    .child(badge_anchor(
                                                        "JD", "5", 0xe14759, false, false,
                                                    ))
                                                    .child(badge_anchor(
                                                        "AB", "New", ACCENT, false, false,
                                                    ))
                                                    .child(badge_anchor(
                                                        "", "", 0x30d38a, true, true,
                                                    )),
                                            ),
                                    ),
                            )
                            .child(div().h(px(1.)).w_full().bg(rgb(0x29292c)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.))
                                    .child(section(
                                        "Tabs",
                                        "Select a tab to see the indicator move",
                                    ))
                                    .child(
                                        div().flex().flex_col().gap(px(10.)).child(primary).child(
                                            label(format!(
                                                "{} panel",
                                                ["Overview", "Activity", "Settings"][self.gallery_tabs.selected]
                                            )),
                                        ),
                                    ),
                            )
                            .child(div().h(px(1.)).w_full().bg(rgb(0x29292c)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.))
                                    .child(section("Slider", "Drag the thumb or select a point"))
                                    .child(sliders),
                            )
                            .child(div().h(px(1.)).w_full().bg(rgb(0x29292c)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.))
                                    .child(section("Button", "Variants and interaction"))
                                    .child(buttons)
                                    .child(section("Button group", "Related actions"))
                                    .child(groups)
                                    .child(label(self.button_feedback.clone())),
                            )
                            .child(div().h(px(1.)).w_full().bg(rgb(0x29292c)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.))
                                    .child(section(
                                        "Media grid",
                                        "Bursts condensed into one card, with counts for identical files",
                                    ))
                                    .child(media_grid_examples()),
                            )
                            .child(div().h(px(1.)).w_full().bg(rgb(0x29292c)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.))
                                    .child(section(
                                        "Top banner",
                                        "App-wide notices and errors",
                                    ))
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.))
                                            .child(components::top_banner(
                                                components::BannerSeverity::Neutral,
                                                "App got updated to the latest version.",
                                                None,
                                                vec![],
                                            ))
                                            .child(components::top_banner(
                                                components::BannerSeverity::Warning,
                                                "Your connection looks unstable.",
                                                None,
                                                vec![],
                                            ))
                                            .child(components::top_banner(
                                                components::BannerSeverity::Danger,
                                                "Couldn't reach Nitro.",
                                                None,
                                                vec![],
                                            ))
                                            .child(components::top_banner(
                                                components::BannerSeverity::Neutral,
                                                "Downloading update…",
                                                Some(0.42),
                                                vec![],
                                            )),
                                    )
                                    .child(
                                        material_interactive(
                                            "gallery-show-banner",
                                            "gallery-show-banner",
                                            MATERIAL_DARK,
                                            18.,
                                            StrokeKind::Neutral,
                                        )
                                        .h(px(36.))
                                        .w(px(150.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(13.))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.show_banner(
                                                components::BannerSeverity::Neutral,
                                                "This banner slides in and dismisses itself.",
                                                None,
                                            );
                                            cx.notify();
                                        }))
                                        .child("Show banner"),
                                    )
                                    .child(section(
                                        "System message",
                                        "Dates and call events, written by the app",
                                    ))
                                    .child(components::system_marker("Fri, Nov 6"))
                                    .child(components::system_marker(
                                        "Voice call ended · 5:37:57 · 15:47",
                                    ))
                                    .child(section(
                                        "Call overlay",
                                        "On top of other apps during a call, with rings that follow voices",
                                    ))
                                    .child(components::call_overlay(
                                        &[
                                            components::OverlayPerson {
                                                id: "sample-a".into(),
                                                name: "Speaking".into(),
                                                initials: "A".into(),
                                                color: 0x3d4a58,
                                                art: true.into(),
                                                muted: false,
                                                level: 0.7,
                                            },
                                            components::OverlayPerson {
                                                id: "sample-b".into(),
                                                name: "Muted".into(),
                                                initials: "N".into(),
                                                color: 0x3d4a58,
                                                art: false.into(),
                                                muted: true,
                                                level: 0.,
                                            },
                                        ],
                                        1.,
                                        true,
                                        None,
                                    ))
                                    .child(section(
                                        "Name effects",
                                        "Animated display names, always on",
                                    ))
                                    .child(components::NameEffect::ALL.into_iter().fold(
                                        div().flex().flex_wrap().gap(px(28.)).text_size(px(16.)),
                                        |row, effect| {
                                            row.child(components::name_text(
                                                effect.label(),
                                                Some(effect),
                                                16.,
                                            ))
                                        },
                                    )),
                            )
                            .child(div().h(px(1.)).w_full().bg(rgb(0x29292c)))
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(48.))
                                    .child(
                                        div()
                                            .min_w(px(256.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(16.))
                                            .child(section("Dropdown", "Choose an action"))
                                            .child(dropdown),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(256.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(16.))
                                            .child(section("Input", "Single-line text entry"))
                                            .child(name_input),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(180.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(16.))
                                            .child(section(
                                                "Toggle button group",
                                                "Select multiple options",
                                            ))
                                            .child(div().flex().child(toggle_group)),
                                    ),
                            )
                            .child(div().h(px(1.)).w_full().bg(rgb(0x29292c)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.))
                                    .child(section("Alert", "Status variants and dismissal"))
                                    .child(alerts),
                            ),
                    )
    }
}
fn media_grid_examples() -> gpui::Div {
    use components::{MEDIA_GRID_TILE_HEIGHT, MediaTile, media_grid};
    let sample = || {
        img(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/avatar.webp"))
            .size_full()
            .object_fit(gpui::ObjectFit::Cover)
    };
    let tile = |count: usize| MediaTile {
        content: div().size_full().child(sample()),
        count,
    };
    div()
        .flex()
        .flex_wrap()
        .items_start()
        .gap(px(16.))
        .child(media_grid(vec![tile(1), tile(1)], "2 images", MEDIA_GRID_TILE_HEIGHT))
        .child(media_grid(
            (0..11).map(|_| tile(1)).collect(),
            "11 images",
            MEDIA_GRID_TILE_HEIGHT,
        ))
        .child(media_grid(vec![tile(20)], "20 images", 240.))
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for frame in self.screen_retired.drain(..) {
            let _ = window.drop_image(frame);
        }
        state::set_active_theme(self.state.preferences.theme);
        if self.applied_icon_theme != Some(self.state.preferences.theme) {
            self.applied_icon_theme = Some(self.state.preferences.theme);
            app_icon::apply(window, self.state.preferences.theme);
        }
        audio::configure(
            self.state.preferences.interface_sounds,
            self.state.preferences.interface_sound_volume,
        );
        // Keep the input library's colours on the theme; Default Dark uses its own.
        if self.applied_theme != Some(self.state.preferences.theme) {
            self.applied_theme = Some(self.state.preferences.theme);
            Theme::change(ThemeMode::Dark, None, cx);
            {
                let palette = state::palette();
                let colors = Theme::global_mut(cx);
                colors.foreground = gpui::rgb(palette.text).into();
                colors.muted_foreground = gpui::rgb(palette.muted).into();
                colors.caret = gpui::rgb(palette.text).into();
                colors.selection = gpui::rgba((palette.accent << 8) | 0x66).into();
            }
        }
        if std::mem::take(&mut self.placeholder_dirty) {
            let placeholder = match self.state.conversations.get(self.state.active_partner) {
                Some(conversation) => format!("Message @{}…", conversation.username),
                None => "Add a friend to start chatting".to_string(),
            };
            self.chat_input
                .update(cx, |input, cx| input.set_placeholder(placeholder, window, cx));
        }
        if let Some(name) = self.pending_sound_name.take() {
            self.sound_draft_name.update(cx, |input, cx| input.set_value(name, window, cx));
        }
        if std::mem::take(&mut self.clear_auth_inputs) {
            for input in [&self.auth_password_input, &self.auth_email_input] {
                input.update(cx, |state, cx| state.set_value("", window, cx));
            }
        }
        if self.sync_error != self.banner_shown_error {
            self.banner_shown_error = self.sync_error.clone();
            if let Some(message) = self.sync_error.clone() {
                self.show_banner(components::BannerSeverity::Danger, message, None);
            }
        }
        let banner = self.render_banner(cx);
        let body = if self.view_mode == 0 {
            self.render_chat_shell(window, cx)
        } else {
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .child(self.render_gallery(window, cx))
                .child(
                    div()
                        .id("gallery-top-drag")
                        .absolute()
                        .top_0()
                        .left_0()
                        .right(px(280.))
                        .h(px(32.))
                        .bg(gpui::transparent_black())
                        .window_control_area(gpui::WindowControlArea::Drag),
                )
                .child(self.render_window_controls(window, cx))
        };
        let lightbox = self.lightbox.clone().map(|path| {
            gpui::deferred(
                div()
                    .id("lightbox")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .p(px(48.))
                    .bg(gpui::rgba(0x000000d9))
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.lightbox = None;
                        cx.notify();
                    }))
                    .child(gpui::img(path).size_full().object_fit(gpui::ObjectFit::Contain)),
            )
            .with_priority(2)
        });
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .children(lightbox)
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if this.handle_shortcut(&event.keystroke, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &Paste, window, cx| {
                cx.stop_propagation();
                this.paste_into_focused(window, cx);
            }))
            .bg(rgb(self.state.preferences.theme.bg_hex()))
            .text_color(rgb(0xf3f3f4))
            .font_family("Inter")
            .children(banner)
            .child(div().flex_1().min_h_0().w_full().flex().child(body))
    }
}
fn main() {
    #[cfg(debug_assertions)]
    if std::env::args().any(|arg| arg == "--mcp-bridge") {
        dev_mcp::bridge();
        return;
    }
    Application::new().with_assets(Icons).run(|cx: &mut App| {
        gpui_component::init(cx);
        // Windows' classic clipboard shortcuts, alongside the Ctrl+C/X/V defaults.
        cx.bind_keys([
            KeyBinding::new("shift-insert", Paste, Some("Input")),
            KeyBinding::new("ctrl-insert", InputCopy, Some("Input")),
            KeyBinding::new("shift-delete", Cut, Some("Input")),
        ]);
        Theme::change(ThemeMode::Dark, None, cx);
        cx.text_system()
            .add_fonts(vec![Cow::Borrowed(include_bytes!(
                "../assets/fonts/Inter-Variable.ttf"
            ))])
            .expect("bundled Inter font must load");
        let bounds = Bounds::centered(None, size(px(1240.), px(820.)), cx);
        let preview = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(480.), px(320.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Nitro".into()),
                        appears_transparent: true,
                        ..Default::default()
                    }),
                    app_id: Some("io.github.nostalg1cc.dislight".into()),
                    ..Default::default()
                },
                |window, cx| {
                    let shell = cx.new(|cx| Preview::new(window, cx));
                    cx.new(|cx| Root::new(shell, window, cx))
                },
            )
            .expect("could not open the Nitro Hero preview window");
        cx.on_window_closed(move |cx| {
            if !cx
                .windows()
                .iter()
                .any(|window| window.window_id() == preview.window_id())
            {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
