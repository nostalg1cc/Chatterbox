//! Connects the shell to Supabase: session lifecycle, loading, polling and writes.
//! Network work runs on background tasks; results are applied on the UI thread.
use std::time::Duration;

use gpui::{AppContext, Context, Window};
use uuid::Uuid;

use crate::{
    Preview,
    backend::{self, ApiError, Participants, Person, SignedInUser, Snapshot},
    state::{
        AuthStatus, AuthTab, ChatAttachment, ChatMessage, PartnerConversation, ReactionChip,
        ShellView, UserProfile, initials_for, is_valid_username,
    },
};

/// Seconds between checks for new messages while the window is focused.
const POLL_SECONDS: u32 = 3;
/// Seconds between checks while it isn't.
const POLL_SECONDS_INACTIVE: u32 = 15;
/// Seconds between re-reading the visible page (edits, deletes, reactions).
const PAGE_REFRESH_SECONDS: u32 = 30;
/// Seconds between re-reading conversations, unread counts and friend requests.
const SNAPSHOT_REFRESH_SECONDS: u32 = 60;

/// Scroll-to-latest appears past this distance, or 2.25 viewports if larger, matching Nitro.
const SCROLL_TO_LATEST_DISTANCE: f32 = 1560.;

fn person_me(profile: &UserProfile) -> Person {
    Person {
        id: profile.id.clone(),
        name: profile.display_name.clone(),
        initials: profile.initials.clone(),
        color: profile.avatar_color,
        photo: profile.photo.clone(),
        name_color: profile.name_color,
        name_weight: profile.name_weight_value,
        name_effect: profile.name_effect,
    }
}

fn person_partner(conversation: &PartnerConversation) -> Person {
    Person {
        id: conversation.partner_id.clone(),
        name: conversation.display_name.clone(),
        initials: conversation.initials.clone(),
        color: conversation.color,
        photo: conversation.photo.clone(),
        name_color: conversation.name_color,
        name_weight: conversation.name_weight,
        name_effect: conversation.name_effect,
    }
}

/// Marks the trim preview in the clip slot, as no real sound has this id.
const SOUND_PREVIEW_ID: &str = "sound-draft-preview";

impl Preview {
    fn signed_in_user(&self) -> Option<SignedInUser> {
        self.user.clone()
    }

    /// The dev MCP must never change anything on the backend.
    fn writes_allowed(&mut self, cx: &mut Context<Self>) -> bool {
        if self.dev_block_writes {
            self.sync_error = Some("Blocked: the dev MCP can't write to Nitro.".into());
            cx.notify();
            return false;
        }
        true
    }

    /// Sends and reactions. The dev MCP may only do these in its allowed test chat.
    fn chat_writes_allowed(&mut self, cx: &mut Context<Self>) -> bool {
        let allowed_chat = self.dev_write_chat.is_some()
            && self
                .state
                .conversations
                .get(self.state.active_partner)
                .is_some_and(|conversation| Some(&conversation.username) == self.dev_write_chat.as_ref());
        allowed_chat || self.writes_allowed(cx)
    }

    fn report(&mut self, error: ApiError, cx: &mut Context<Self>) {
        if matches!(error, ApiError::SignedOut) {
            self.reset_to_signed_out(Some(error.message()), cx);
            return;
        }
        self.sync_error = Some(error.message());
        cx.notify();
    }

    fn reset_to_signed_out(&mut self, message: Option<String>, cx: &mut Context<Self>) {
        self.sync_generation += 1;
        self.user = None;
        self.state.auth_status = AuthStatus::SignedOut;
        self.state.profile = UserProfile::default();
        self.state.conversations.clear();
        self.state.friend_requests.clear();
        self.state.active_partner = 0;
        self.sidebar_media.clear();
        self.message_jump = None;
        self.soundboard_clips.clear();
        self.voice.control(crate::voice::Control::Stop);
        self.voice_events = None;
        self.voice_conversation = None;
        self.voice_state = Default::default();
        crate::audio::stop_clip();
        self.playing_sound_id = None;
        self.playing_since = None;
        self.soundboard_status = crate::SoundboardStatus::NotLoaded;
        // Anything still queued was dropped, so it must be able to request again next login.
        self.media_queue.clear();
        self.media_files
            .retain(|_, file| !matches!(file, crate::MediaFile::Loading));
        self.auth_tab = AuthTab::Login;
        self.auth_busy = None;
        self.auth_error = message;
        self.sync_error = None;
        self.chat_replying_to = None;
        self.chat_reply_target = None;
        self.shell_view = ShellView::Chat;
        self.call_connected = false;
        self.placeholder_dirty = true;
        cx.notify();
    }

    // -----------------------------------------------------------------------
    // Session
    // -----------------------------------------------------------------------

    pub fn restore_saved_session(&mut self, cx: &mut Context<Self>) {
        if !backend::may_restore_session() {
            return;
        }
        self.auth_busy = Some("Restoring your session…".into());
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move { backend::restore_session() })
                .await;
            let _ = view.update(cx, |this, cx| match result {
                Ok(Some(user)) => this.enter_session(user, cx),
                Ok(None) => {
                    this.auth_busy = None;
                    cx.notify();
                }
                Err(error) => {
                    this.auth_busy = None;
                    this.auth_error = Some(error.message());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn submit_login(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.auth_busy.is_some() {
            return;
        }
        let email = self.auth_email_input.read(cx).value().trim().to_string();
        let password = self.auth_password_input.read(cx).value().to_string();
        if email.is_empty() || password.is_empty() {
            self.auth_error = Some("Enter both your email and password.".into());
            cx.notify();
            return;
        }
        self.auth_error = None;
        self.auth_busy = Some("Logging in…".into());
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move { backend::sign_in(&email, &password) })
                .await;
            let _ = view.update(cx, |this, cx| match result {
                Ok(user) => this.enter_session(user, cx),
                Err(error) => {
                    this.auth_busy = None;
                    this.auth_error = Some(error.message());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn submit_signup(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.auth_busy.is_some() || !self.writes_allowed(cx) {
            return;
        }
        let username = self
            .auth_username_input
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let display_name = self
            .auth_display_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let email = self.auth_email_input.read(cx).value().trim().to_string();
        let password = self.auth_password_input.read(cx).value().to_string();
        if !is_valid_username(&username) {
            self.auth_error = Some("Usernames are 3–20 characters: a–z, 0–9, underscores.".into());
            cx.notify();
            return;
        }
        if email.is_empty() || !email.contains('@') {
            self.auth_error = Some("Enter a valid email address.".into());
            cx.notify();
            return;
        }
        if password.len() < 8 {
            self.auth_error = Some("Password needs at least 8 characters.".into());
            cx.notify();
            return;
        }
        let shown_name = if display_name.is_empty() {
            username.clone()
        } else {
            display_name
        };
        self.auth_error = None;
        self.auth_busy = Some("Creating your account…".into());
        cx.notify();
        let verify_email = email.clone();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    backend::sign_up(&email, &password, &username, &shown_name)
                })
                .await;
            let _ = view.update(cx, |this, cx| match result {
                Ok(Some(user)) => this.enter_session(user, cx),
                Ok(None) => {
                    this.auth_busy = None;
                    this.clear_auth_inputs = true;
                    this.auth_tab = AuthTab::VerifyEmail(verify_email);
                    cx.notify();
                }
                Err(error) => {
                    this.auth_busy = None;
                    this.auth_error = Some(error.message());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn enter_session(&mut self, user: SignedInUser, cx: &mut Context<Self>) {
        self.clear_auth_inputs = true;
        self.auth_error = None;
        self.auth_busy = Some("Loading your conversations…".into());
        self.user = Some(user.clone());
        self.sync_generation += 1;
        let generation = self.sync_generation;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move { backend::load_snapshot(&user) })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.sync_generation != generation {
                    return;
                }
                this.auth_busy = None;
                match result {
                    Ok(snapshot) => {
                        this.apply_snapshot(snapshot, true, cx);
                        this.start_sync(cx);
                        this.start_media_pump(cx);
                    }
                    Err(ApiError::SignedOut) => {
                        this.reset_to_signed_out(Some(ApiError::SignedOut.message()), cx);
                    }
                    Err(error) => {
                        this.user = None;
                        this.auth_error = Some(error.message());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub fn sign_out(&mut self, cx: &mut Context<Self>) {
        if self.dev_block_writes {
            self.sync_error = Some("Blocked: the dev MCP can't log out.".into());
            cx.notify();
            return;
        }
        self.reset_to_signed_out(None, cx);
        cx.background_spawn(async move { backend::sign_out() })
            .detach();
    }

    // -----------------------------------------------------------------------
    // Loading
    // -----------------------------------------------------------------------

    fn apply_snapshot(&mut self, snapshot: Snapshot, first: bool, cx: &mut Context<Self>) {
        let active_id = self
            .state
            .conversations
            .get(self.state.active_partner)
            .map(|conversation| conversation.id.clone());
        let mut next = snapshot.conversations;
        for conversation in &mut next {
            if let Some(previous) = self
                .state
                .conversations
                .iter_mut()
                .find(|existing| existing.id == conversation.id)
            {
                conversation.messages = std::mem::take(&mut previous.messages);
                conversation.shared_media = std::mem::take(&mut previous.shared_media);
                conversation.loaded = previous.loaded;
                conversation.voice_started_at = previous.voice_started_at.take();
                conversation.voice_participants = std::mem::take(&mut previous.voice_participants);
                conversation.has_more = previous.has_more;
            }
        }
        self.state.conversations = next;
        self.state.active_partner = active_id
            .and_then(|id| {
                self.state
                    .conversations
                    .iter()
                    .position(|conversation| conversation.id == id)
            })
            .unwrap_or(0);
        if let Some(active) = self.state.conversations.get_mut(self.state.active_partner) {
            active.unread_count = 0;
        }
        self.state.friend_requests = snapshot.friend_requests;
        if let Some(on) = snapshot.snow {
            self.set_snow(on, cx);
        }
        if let Some((id, severity, message)) = snapshot.notice
            && self.state.preferences.dismissed_notice_id.as_deref() != Some(id.as_str())
            && self.last_notice_id.as_deref() != Some(id.as_str())
        {
            self.last_notice_id = Some(id.clone());
            self.show_banner(
                crate::components::BannerSeverity::parse(&severity),
                message,
                Some(id),
            );
        }
        self.state.profile = snapshot.profile;
        if first {
            self.state.auth_status = if snapshot.onboarded {
                AuthStatus::SignedIn
            } else {
                AuthStatus::Onboarding
            };
            self.onboarding_step = 0;
            self.shell_view = ShellView::Chat;
            self.load_active(cx);
        }
        self.placeholder_dirty = true;
        cx.notify();
    }

    fn reload_snapshot(&mut self, cx: &mut Context<Self>) {
        let Some(user) = self.signed_in_user() else {
            return;
        };
        let generation = self.sync_generation;
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move { backend::load_snapshot(&user) })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.sync_generation != generation {
                    return;
                }
                match result {
                    Ok(snapshot) => this.apply_snapshot(snapshot, false, cx),
                    Err(error) => this.report(error, cx),
                }
            });
        })
        .detach();
    }

    fn active_request(&self) -> Option<(String, Participants, Option<String>)> {
        let conversation = self.state.conversations.get(self.state.active_partner)?;
        let last = conversation
            .messages
            .iter()
            .rev()
            .find(|message| !message.pending && !message.db_id.starts_with("local-"))
            .map(|message| message.created_at.clone());
        Some((
            conversation.id.clone(),
            Participants {
                me: person_me(&self.state.profile),
                partner: person_partner(conversation),
            },
            last,
        ))
    }

    fn apply_messages(
        &mut self,
        conversation_id: &str,
        incoming: Vec<ChatMessage>,
        has_more: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        let active = self.state.active_partner;
        let Some(index) = self
            .state
            .conversations
            .iter()
            .position(|conversation| conversation.id == conversation_id)
        else {
            return;
        };
        let conversation = &mut self.state.conversations[index];
        let before = conversation.messages.len();
        let had_messages = conversation.loaded;
        let known: std::collections::HashSet<String> =
            conversation.messages.iter().map(|message| message.db_id.clone()).collect();
        let newest_known = conversation.messages.last().map(|message| message.created_at.clone());
        let arrived = incoming.iter().any(|message| {
            !message.is_self
                && !message.system
                && !known.contains(&message.db_id)
                && newest_known.as_ref().is_some_and(|newest| message.created_at > *newest)
        });
        if had_messages && arrived {
            crate::audio::play(crate::audio::UiSound::Message);
        }
        backend::merge_messages(&mut conversation.messages, incoming);
        let older = self.sidebar_media.get(conversation_id).map_or(&[][..], |pages| &pages.messages[..]);
        conversation.shared_media = backend::media_items_with(&conversation.messages, older);
        conversation.loaded = true;
        if let Some(has_more) = has_more {
            conversation.has_more = has_more;
        }
        if index == active && conversation.messages.len() != before {
            self.chat_scroll.scroll_to_bottom();
            self.history_settled_at = std::time::Instant::now();
        }
        cx.notify();
    }

    /// Loads the open conversation and marks it read.
    pub fn load_active(&mut self, cx: &mut Context<Self>) {
        self.refresh_voice(cx);
        let Some((conversation_id, who, _)) = self.active_request() else {
            return;
        };
        let reader = who.me.id.clone();
        let generation = self.sync_generation;
        let blocked = self.dev_block_writes;
        cx.spawn(async move |view, cx| {
            let id = conversation_id.clone();
            let result = cx
                .background_spawn(async move {
                    let page = backend::load_latest(&id, &who);
                    if page.is_ok() && !blocked {
                        let _ = backend::mark_read(&id, &reader);
                    }
                    page
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.sync_generation != generation {
                    return;
                }
                match result {
                    Ok((messages, has_more)) => {
                        this.apply_messages(&conversation_id, messages, Some(has_more), cx)
                    }
                    Err(error) => this.report(error, cx),
                }
            });
        })
        .detach();
    }

    fn start_sync(&mut self, cx: &mut Context<Self>) {
        let generation = self.sync_generation;
        cx.spawn(async move |view, cx| {
            let mut elapsed = 0u32;
            let mut since_poll = 0u32;
            loop {
                gpui::Timer::after(Duration::from_secs(1)).await;
                elapsed += 1;
                since_poll += 1;
                let Ok(step) = view.update(cx, |this, cx| {
                    if this.sync_generation != generation {
                        return None;
                    }
                    if elapsed % SNAPSHOT_REFRESH_SECONDS == 0 {
                        this.reload_snapshot(cx);
                    }
                    Some((this.window_active, this.active_request(), std::mem::take(&mut this.poll_soon)))
                }) else {
                    break;
                };
                let Some((window_active, request, poll_soon)) = step else {
                    break;
                };
                let interval = if window_active { POLL_SECONDS } else { POLL_SECONDS_INACTIVE };
                if since_poll < interval && !poll_soon {
                    continue;
                }
                since_poll = 0;
                let _ = view.update(cx, |this, cx| this.refresh_voice(cx));
                let Some((conversation_id, who, last)) = request else {
                    continue;
                };
                let full_page = last.is_none() || elapsed % PAGE_REFRESH_SECONDS < interval;
                let id = conversation_id.clone();
                let result = cx
                    .background_spawn(async move {
                        match last {
                            Some(after) if !full_page => backend::load_newer(&id, &after, &who),
                            _ => backend::load_latest(&id, &who).map(|(rows, _)| rows),
                        }
                    })
                    .await;
                let keep_going = view.update(cx, |this, cx| {
                    if this.sync_generation != generation {
                        return false;
                    }
                    match result {
                        Ok(messages) if !messages.is_empty() => {
                            this.apply_messages(&conversation_id, messages, None, cx)
                        }
                        Ok(_) => {}
                        // Offline blips shouldn't spam the UI; the next poll retries.
                        Err(ApiError::Network(_)) => {}
                        Err(error) => this.report(error, cx),
                    }
                    true
                });
                if !matches!(keep_going, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    // -----------------------------------------------------------------------
    // Media, history and soundboard
    // -----------------------------------------------------------------------

    /// Fetches images that have scrolled into view (a few at a time) and older history.
    fn start_media_pump(&mut self, cx: &mut Context<Self>) {
        let generation = self.sync_generation;
        cx.spawn(async move |view, cx| {
            loop {
                gpui::Timer::after(Duration::from_millis(120)).await;
                let keep_going = view.update(cx, |this, cx| {
                    if this.sync_generation != generation {
                        return false;
                    }
                    let wanted: Vec<String> = this.media_wanted.borrow_mut().drain(..).collect();
                    for path in wanted {
                        if !this.media_files.contains_key(&path) {
                            this.media_files.insert(path.clone(), crate::MediaFile::Loading);
                            this.media_queue.push_back(path);
                        }
                    }
                    while this.media_in_flight < 3 {
                        let Some(path) = this.media_queue.pop_front() else {
                            break;
                        };
                        this.fetch_image(path, cx);
                    }
                    // Anything that was resting and is still on screen asks again from here.
                    let now = std::time::Instant::now();
                    let resting = this.link_cards.len();
                    this.link_cards.retain(|_, state| {
                        !matches!(state, crate::links::LinkState::Later(at) if now >= *at)
                    });
                    if this.link_cards.len() != resting {
                        cx.notify();
                    }
                    let wanted: Vec<String> = this.link_wanted.borrow_mut().drain(..).collect();
                    for url in wanted {
                        if !this.link_cards.contains_key(&url) {
                            this.link_cards.insert(url.clone(), crate::links::LinkState::Loading);
                            this.link_queue.push_back(url);
                        }
                    }
                    while this.link_in_flight < 2 {
                        let Some(url) = this.link_queue.pop_front() else {
                            break;
                        };
                        this.fetch_link_card(url, cx);
                    }
                    if this.wants_older.take() {
                        this.load_older_messages(cx);
                    }
                    let distance = f32::from(this.chat_scroll.max_offset().height)
                        + f32::from(this.chat_scroll.offset().y);
                    let viewport = f32::from(this.chat_scroll.bounds().size.height);
                    let scrolled_up = distance > SCROLL_TO_LATEST_DISTANCE.max(viewport * 2.25);
                    if scrolled_up != this.history_scrolled_up {
                        this.history_scrolled_up = scrolled_up;
                        cx.notify();
                    }
                    true
                });
                if !matches!(keep_going, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn stop_video(&mut self) {
        if let Some(frame) = self.playing_video.take().and_then(|playing| playing.frame) {
            self.screen_retired.push(frame);
        }
    }

    pub fn toggle_video_pause(&mut self) {
        if let Some(player) = self.playing_video.as_ref().and_then(|playing| playing.player.as_ref()) {
            player.set_paused(!player.paused());
        }
    }

    /// Plays a video in chat, or pauses it if it's the one already going. It's downloaded on
    /// the first play and kept. A format Windows can't play opens in the default player.
    pub fn toggle_video(&mut self, path: String, cx: &mut Context<Self>) {
        let same = self.playing_video.as_ref().is_some_and(|playing| playing.path == path);
        if same && self.playing_video.as_ref().is_some_and(|playing| playing.player.is_some()) {
            self.toggle_video_pause();
            cx.notify();
            return;
        }
        self.stop_video();
        if same {
            // Pressed again while it was still downloading.
            cx.notify();
            return;
        }
        self.playing_video = Some(crate::PlayingVideo {
            path: path.clone(),
            conversation: self.active_conv().id.clone(),
            expanded: false,
            player: None,
            frame: None,
        });
        let muted = self.call_deafened;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let requested = path.clone();
            let loaded = cx
                .background_spawn(async move {
                    let file = backend::fetch_chat_video(&requested)?;
                    let poster = backend::video_poster_file(&requested);
                    if !poster.exists() {
                        let _ = crate::video::save_poster(&file, &poster);
                    }
                    let player = crate::video::Player::start(&file, muted).ok();
                    Ok::<_, backend::ApiError>((file, player))
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                // They may have stopped it, or started another, during the download.
                // A thumbnail that couldn't be made from the file's start can be made now.
                if matches!(this.media_files.get(&path), Some(crate::MediaFile::Failed)) {
                    this.media_files.remove(&path);
                }
                let wanted = this.playing_video.as_ref().is_some_and(|playing| playing.path == path && playing.player.is_none());
                match loaded {
                    Ok((_, Some(player))) if wanted => {
                        if let Some(playing) = &mut this.playing_video {
                            playing.player = Some(player);
                        }
                    }
                    Ok((file, None)) if wanted => {
                        this.stop_video();
                        cx.open_with_system(&file);
                    }
                    Err(error) if wanted => {
                        this.stop_video();
                        this.report(error, cx);
                    }
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn fetch_link_card(&mut self, url: String, cx: &mut Context<Self>) {
        self.link_in_flight += 1;
        cx.spawn(async move |view, cx| {
            let requested = url.clone();
            let state = cx.background_spawn(async move { crate::links::load(&requested) }).await;
            let _ = view.update(cx, |this, cx| {
                this.link_in_flight = this.link_in_flight.saturating_sub(1);
                // A card arriving makes the history taller, so someone reading the latest
                // message stays with it.
                let distance = f32::from(this.chat_scroll.max_offset().height)
                    + f32::from(this.chat_scroll.offset().y);
                if matches!(state, crate::links::LinkState::Ready(_)) && distance < 60. {
                    this.chat_scroll.scroll_to_bottom();
                }
                this.link_cards.insert(url, state);
                cx.notify();
            });
        })
        .detach();
    }

    fn fetch_image(&mut self, path: String, cx: &mut Context<Self>) {
        self.media_in_flight += 1;
        cx.spawn(async move |view, cx| {
            let requested = path.clone();
            let result = cx
                .background_spawn(async move {
                    // A video's stand-in is its first picture.
                    let is_video = requested.starts_with("cloudinary:video:") || requested.ends_with(".webm");
                    let file = if is_video {
                        backend::fetch_video_poster(&requested)
                    } else {
                        backend::fetch_chat_image(&requested)
                    };
                    file.map(|original| {
                        let sidebar = backend::sidebar_thumbnail(&original);
                        crate::MediaFile::Ready { original, sidebar }
                    })
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                this.media_in_flight = this.media_in_flight.saturating_sub(1);
                this.media_files.insert(
                    path,
                    match result {
                        Ok(file) => file,
                        Err(_) => crate::MediaFile::Failed,
                    },
                );
                cx.notify();
            });
        })
        .detach();
    }

    fn load_older_messages(&mut self, cx: &mut Context<Self>) {
        if self.loading_older {
            return;
        }
        let Some((conversation_id, who, _)) = self.active_request() else {
            return;
        };
        let conversation = self.active_conv();
        if !conversation.has_more {
            return;
        }
        let Some(before) = conversation
            .messages
            .iter()
            .find(|message| !message.pending && !message.db_id.starts_with("local-"))
            .map(|message| message.created_at.clone())
        else {
            return;
        };
        self.loading_older = true;
        let generation = self.sync_generation;
        cx.spawn(async move |view, cx| {
            let id = conversation_id.clone();
            let result = cx
                .background_spawn(async move { backend::load_older(&id, &before, &who) })
                .await;
            let _ = view.update(cx, |this, cx| {
                this.loading_older = false;
                if this.sync_generation != generation {
                    return;
                }
                match result {
                    Ok((messages, has_more)) => {
                        // Keep the viewport on the same message once the page is laid out.
                        let before_max = f32::from(this.chat_scroll.max_offset().height);
                        let before_y = f32::from(this.chat_scroll.offset().y);
                        this.older_anchor.set(Some((before_max, before_y)));
                        let keep = this.history_settled_at;
                        this.apply_messages(&conversation_id, messages, Some(has_more), cx);
                        this.history_settled_at = keep;
                    }
                    Err(error) => this.report(error, cx),
                }
            });
        })
        .detach();
    }

    /// Plays a sound here on this device, or stops it if it's already playing.
    /// Quietly at startup; `manual` also reports being up to date or failing.
    pub fn check_for_update(&mut self, manual: bool, cx: &mut Context<Self>) {
        self.update = crate::UpdateState::Checking;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx.background_spawn(async move { crate::updater::check() }).await;
            let _ = view.update(cx, |this, cx| {
                this.update = match result {
                    Ok(Some(update)) => {
                        let message = format!("Nitro {} is available. Install it from Settings, About.", update.version);
                        this.show_banner(crate::components::BannerSeverity::Neutral, message, None);
                        crate::UpdateState::Available(update)
                    }
                    Ok(None) if manual => crate::UpdateState::UpToDate,
                    Err(message) if manual => crate::UpdateState::Failed(message),
                    _ => crate::UpdateState::Idle,
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// Asks first if there's a call, since updating restarts the app.
    pub fn install_update(&mut self, update: crate::updater::Update, cx: &mut Context<Self>) {
        if self.voice_events.is_some() {
            self.update = crate::UpdateState::Failed("Leave your call before updating.".into());
            cx.notify();
            return;
        }
        // Only release builds replace the installed app.
        if cfg!(debug_assertions) {
            self.update = crate::UpdateState::Failed("Test builds don't install updates.".into());
            cx.notify();
            return;
        }
        self.update_progress.store(0, std::sync::atomic::Ordering::Relaxed);
        self.update = crate::UpdateState::Downloading(update.version.clone());
        let progress = self.update_progress.clone();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    let file = crate::updater::download(&update, progress)?;
                    crate::updater::install(&file)
                })
                .await;
            let _ = view.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.flush_saved_state();
                    cx.quit();
                }
                Err(message) => {
                    this.update = crate::UpdateState::Failed(message);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn choose_sound_file(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose audio".into()),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let _ = view.update(cx, |this, cx| {
                this.sound_loading = true;
                cx.notify();
            });
            let file = path.clone();
            let result = cx
                .background_spawn(async move { crate::sound_file::decode_source(&file) })
                .await;
            let _ = view.update(cx, |this, cx| {
                this.sound_loading = false;
                match result {
                    Ok(samples) => {
                        let duration_ms = (samples.len() as u64 * 1000 / 48_000) as u32;
                        let stem = path.file_stem().map(|stem| stem.to_string_lossy().chars().take(32).collect::<String>());
                        this.sound_draft = Some(crate::SoundDraft {
                            file_name: path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(),
                            samples: std::sync::Arc::new(samples),
                            duration_ms,
                            start_ms: 0,
                            end_ms: duration_ms.min(crate::sound_file::MAX_CLIP_MS),
                        });
                        this.pending_sound_name = stem;
                    }
                    Err(message) => this.sync_error = Some(message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// `value` is in slider steps. The ends can't cross, and stay a playable length apart.
    pub fn set_sound_trim(&mut self, start: bool, value: u32) {
        let min = crate::sound_file::MIN_CLIP_MS;
        let Some(draft) = &mut self.sound_draft else { return };
        let at = (value * draft.trim_unit()).min(draft.duration_ms);
        if start {
            draft.start_ms = at.min(draft.end_ms.saturating_sub(min));
        } else {
            draft.end_ms = at.max(draft.start_ms + min).min(draft.duration_ms);
        }
        self.stop_sound_preview();
    }

    pub fn stop_sound_preview(&mut self) {
        if self.playing_sound_id.as_deref() == Some(SOUND_PREVIEW_ID) {
            crate::audio::stop_clip();
            self.playing_sound_id = None;
            self.playing_since = None;
        }
    }

    pub fn preview_sound_draft(&mut self, cx: &mut Context<Self>) {
        if self.playing_sound_id.as_deref() == Some(SOUND_PREVIEW_ID) {
            self.stop_sound_preview();
            cx.notify();
            return;
        }
        let Some(draft) = &self.sound_draft else { return };
        let clip = crate::sound_file::preview(&draft.samples, draft.start_ms, draft.end_ms);
        let duration = clip.len() as u64 * 1000 / 48_000;
        match crate::audio::play_samples(clip, self.state.preferences.soundboard_volume) {
            Ok(()) => {
                self.playing_sound_id = Some(SOUND_PREVIEW_ID.to_string());
                self.playing_since = Some(std::time::Instant::now());
                self.playing_ms = duration;
            }
            Err(message) => self.sync_error = Some(message),
        }
        cx.notify();
    }

    pub fn discard_sound_draft(&mut self, cx: &mut Context<Self>) {
        self.stop_sound_preview();
        self.sound_draft = None;
        cx.notify();
    }

    pub fn save_sound_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.writes_allowed(cx) {
            return;
        }
        let name: String = self.sound_draft_name.read(cx).value().trim().chars().take(32).collect();
        if name.is_empty() {
            self.sync_error = Some("Give the sound a name before saving it.".into());
            cx.notify();
            return;
        }
        let Some(draft) = &self.sound_draft else { return };
        let (samples, start, end) = (draft.samples.clone(), draft.start_ms, draft.end_ms);
        self.stop_sound_preview();
        self.sound_saving = true;
        cx.notify();
        let _ = window;
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    let prepared = crate::sound_file::prepare(&samples, start, end)?;
                    backend::upload_sound(&name, prepared.webm, prepared.duration_ms)
                        .map_err(|error| error.message())
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                this.sound_saving = false;
                match result {
                    Ok(()) => {
                        this.sound_draft = None;
                        this.soundboard_status = crate::SoundboardStatus::NotLoaded;
                        this.load_soundboard(cx);
                    }
                    Err(message) => this.sync_error = Some(message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Plays a sound for both people in the call, as main Nitro's soundboard does.
    pub fn play_clip_in_call(&mut self, id: String, cx: &mut Context<Self>) {
        let stopping = self.playing_sound_id.as_deref() == Some(id.as_str());
        self.play_clip(id.clone(), cx);
        if self.voice_events.is_none() {
            return;
        }
        if stopping {
            if let Some((sent_id, nonce)) = self.sent_sound.take() {
                self.voice.control(crate::voice::Control::PeerData(serde_json::json!({
                    "type":"soundboard-stop",
                    "payload":{"version":1,"id":sent_id,"nonce":nonce},
                })));
            }
            return;
        }
        let name = self.soundboard_clips.iter().find(|clip| clip.id == id).map(|clip| clip.name.clone()).unwrap_or_default();
        let nonce = Uuid::new_v4().to_string();
        self.sent_sound = Some((id.clone(), nonce.clone()));
        let voice = self.voice.clone();
        let conversation = self.voice_conversation.clone();
        cx.background_spawn(async move {
            if let Ok(link) = backend::sound_link(&id, conversation.as_deref()) {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.as_millis() as u64);
                voice.control(crate::voice::Control::PeerData(serde_json::json!({
                    "type":"soundboard",
                    "payload":{"version":1,"id":id,"name":name,"signedUrl":link,"playAt":now,"nonce":nonce},
                })));
            }
        })
        .detach();
    }

    /// Acts on sounds the partner played.
    fn pump_remote_sounds(&mut self, state: &crate::voice::State, cx: &mut Context<Self>) {
        for (count, message) in &state.sound_events {
            if *count <= self.remote_sound_seen {
                continue;
            }
            self.remote_sound_seen = *count;
            let payload = &message["payload"];
            let id = payload["id"].as_str().unwrap_or("").to_string();
            match message["type"].as_str() {
                Some("soundboard") | Some("soundboard-prepare") => {
                    let play = message["type"] == "soundboard";
                    if play && (self.call_deafened || self.remote_sound_at.is_some_and(|at| at.elapsed().as_millis() < 750)) {
                        continue;
                    }
                    if play {
                        self.remote_sound_at = Some(std::time::Instant::now());
                        self.remote_sound_nonce = payload["nonce"].as_str().map(String::from);
                    }
                    let url = payload["signedUrl"].as_str().unwrap_or("").to_string();
                    let volume = self.state.preferences.soundboard_volume;
                    cx.background_spawn(async move {
                        let Ok(file) = backend::fetch_shared_sound(&id, &url) else { return };
                        if play && let Ok(clip) = crate::audio::decode_clip(&file) {
                            let _ = crate::audio::play_clip(&clip, volume);
                        }
                    })
                    .detach();
                }
                Some("soundboard-stop") if payload["nonce"].as_str() == self.remote_sound_nonce.as_deref() => {
                    self.remote_sound_nonce = None;
                    crate::audio::stop_clip();
                }
                _ => {}
            }
        }
    }

    /// Partner and output volume together are what this call plays at.
    pub fn send_output_level(&self) {
        let prefs = &self.state.preferences;
        let level = (prefs.partner_volume as f32 / 100.) * (prefs.output_volume as f32 / 100.);
        self.voice.control(crate::voice::Control::OutputLevel(level.clamp(0., 1.)));
    }

    /// Registers the call shortcuts system-wide during a call, and acts on them.
    pub fn pump_hotkeys(&mut self, cx: &mut Context<Self>) {
        if self.voice_events.is_none() {
            self.hotkeys = None;
            return;
        }
        let binds = self.state.preferences.keybinds.to_vec();
        let hotkeys = self.hotkeys.get_or_insert_with(crate::hotkeys::Hotkeys::start);
        hotkeys.set(&binds);
        for action in hotkeys.take() {
            self.run_shortcut(action, cx);
        }
    }

    pub fn play_clip(&mut self, id: String, cx: &mut Context<Self>) {
        if self.playing_sound_id.as_deref() == Some(id.as_str()) {
            crate::audio::stop_clip();
            self.playing_sound_id = None;
            self.playing_since = None;
            cx.notify();
            return;
        }
        self.clip_request += 1;
        let request = self.clip_request;
        let volume = self.state.preferences.soundboard_volume;
        let conversation = self.voice_conversation.clone();
        cx.spawn(async move |view, cx| {
            let clip_id = id.clone();
            let result = cx
                .background_spawn(async move {
                    let file = backend::fetch_sound(&clip_id, conversation.as_deref()).map_err(|error| error.message())?;
                    let clip = crate::audio::decode_clip(&file)?;
                    crate::audio::play_clip(&clip, volume)?;
                    Ok::<u64, String>(clip.duration_ms())
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.clip_request != request {
                    return;
                }
                match result {
                    Ok(duration) => {
                        this.playing_sound_id = Some(id);
                        this.playing_since = Some(std::time::Instant::now());
                        this.playing_ms = duration;
                    }
                    Err(message) => {
                        this.playing_sound_id = None;
                        this.playing_since = None;
                        this.sync_error = Some(message);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Joins this conversation's voice room for real: microphone, room membership, then a
    /// direct peer connection, relayed through Cloudflare only if direct fails.
    pub fn join_voice(&mut self, cx: &mut Context<Self>) {
        if self.state.conversations.is_empty() {
            return;
        }
        crate::voice::note("join requested");
        // This click wins: whatever call the app thinks it's in ends, and this one starts.
        if self.voice_events.is_some() || self.call_connected {
            self.end_call_state(cx);
        }
        let conversation = self.active_conv();
        let (conversation_id, partner) = (conversation.id.clone(), conversation.partner_id.clone());
        let user = self.state.profile.id.clone();
        let session = Uuid::new_v4().to_string();
        let saved_input = self.state.preferences.input_device_id.clone();
        let saved_output = self.state.preferences.output_device_id.clone();
        self.voice_conversation = Some(conversation_id.clone());
        // In a call the list includes the partner's sounds.
        self.soundboard_status = crate::SoundboardStatus::NotLoaded;
        self.voice.want(session.clone());
        self.call_connected = true;
        self.call_started = std::time::Instant::now();
        crate::audio::play(crate::audio::UiSound::VoiceJoin);
        cx.notify();
        // A saved device is passed as it is: the engine falls back to the Windows default for
        // this call if it's unplugged, and it's still remembered for when it comes back.
        let route = |saved: String| (saved != "default").then_some(saved);
        let (input, output) = (route(saved_input), route(saved_output));
        cx.spawn(async move |view, cx| {
            let credentials = cx.background_spawn(async move { backend::voice_credentials() }).await;
            let _ = view.update(cx, |this, cx| {
                let (url, key, token) = match credentials {
                    Ok(credentials) => credentials,
                    Err(error) => {
                        this.call_connected = false;
                        this.report(error, cx);
                        return;
                    }
                };
                if !this.voice.wanted(&session) {
                    return;
                }
                let (sender, receiver) = std::sync::mpsc::channel();
                let handle = crate::voice::start(
                    crate::voice::Start {
                        input,
                        output,
                        url,
                        key,
                        token,
                        user,
                        partner,
                        conversation: conversation_id,
                        session: session.clone(),
                    },
                    sender,
                );
                this.voice.attach(&session, handle);
                this.voice_events = Some(receiver);
                this.voice_token_at = std::time::Instant::now();
            });
        })
        .detach();
    }

    pub fn leave_voice(&mut self, cx: &mut Context<Self>) {
        crate::voice::note("leave requested (button or shortcut)");
        self.end_call_state(cx);
        crate::audio::play(crate::audio::UiSound::VoiceLeave);
        cx.notify();
        self.refresh_voice(cx);
    }

    fn end_call_state(&mut self, cx: &mut Context<Self>) {
        self.soundboard_status = crate::SoundboardStatus::NotLoaded;
        self.voice.control(crate::voice::Control::Stop);
        self.voice_events = None;
        self.voice_conversation = None;
        self.voice_state = Default::default();
        self.end_screen_view(cx);
        self.end_share_preview(cx);
        self.call_connected = false;
        self.call_voice_hovered = false;
        self.call_streaming = false;
        self.call_muted = false;
        self.call_deafened = false;
        self.open_dropdown = None;
        self.call_notice = None;
        crate::audio::stop_clip();
    }

    pub fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        self.call_muted = !self.call_muted;
        crate::audio::play(if self.call_muted {
            crate::audio::UiSound::MuteOn
        } else {
            crate::audio::UiSound::MuteOff
        });
        self.send_voice_controls();
        cx.notify();
    }

    pub fn toggle_deafen(&mut self, cx: &mut Context<Self>) {
        self.call_deafened = !self.call_deafened;
        crate::audio::play(if self.call_deafened {
            crate::audio::UiSound::MuteOn
        } else {
            crate::audio::UiSound::MuteOff
        });
        if self.call_deafened {
            self.call_muted = true;
        }
        self.send_voice_controls();
        cx.notify();
    }

    /// Runs an in-app shortcut, or records one while a keybind is being set.
    /// Returns true if the key was used.
    pub fn handle_shortcut(&mut self, keystroke: &gpui::Keystroke, cx: &mut Context<Self>) -> bool {
        let pressed = keystroke.unparse();
        if let Some(index) = self.recording_keybind {
            let lone_modifier = matches!(
                keystroke.key.as_str(),
                "control" | "shift" | "alt" | "platform" | "function"
            );
            if lone_modifier {
                return true;
            }
            self.recording_keybind = None;
            if pressed != "escape" {
                // A duplicate moves here, as in Nitro.
                for bind in &mut self.state.preferences.keybinds {
                    if *bind == pressed {
                        bind.clear();
                    }
                }
                if self.state.preferences.wheel_key == pressed {
                    self.state.preferences.wheel_key.clear();
                }
                match self.state.preferences.keybinds.get_mut(index) {
                    Some(bind) => *bind = pressed,
                    None => self.state.preferences.wheel_key = pressed,
                }
                self.persist();
            }
            cx.notify();
            return true;
        }
        let Some(action) = self
            .state
            .preferences
            .keybinds
            .iter()
            .position(|bind| !bind.is_empty() && *bind == pressed)
        else {
            return false;
        };
        if !self.call_connected {
            return false;
        }
        self.run_shortcut(action, cx);
        true
    }

    fn run_shortcut(&mut self, action: usize, cx: &mut Context<Self>) {
        match action {
            0 => self.toggle_mute(cx),
            1 => self.toggle_deafen(cx),
            2 => self.leave_voice(cx),
            _ => {
                self.load_soundboard(cx);
                self.open_dropdown = if self.open_dropdown == Some(crate::state::HeaderDropdown::Soundboard) {
                    None
                } else {
                    Some(crate::state::HeaderDropdown::Soundboard)
                };
                cx.notify();
            }
        }
    }

    /// Keeps the desktop overlay matching the call.
    pub fn sync_overlay(&mut self, cx: &mut Context<Self>) {
        let prefs = &self.state.preferences;
        let scale = prefs.voice_hud_scale as f32 / 100.;
        let show_names = prefs.voice_hud_show_names;
        let call = self.state.conversations.iter().find(|conversation| {
            Some(&conversation.id) == self.voice_conversation.as_ref()
        });
        let people = call.filter(|_| prefs.voice_hud_enabled).map(|conversation| {
            let me = &self.state.profile;
            let level = |id: &str| self.voice_state.levels.get(id).copied().unwrap_or(0.);
            let partner_in = self.voice_state.participants.contains(&conversation.partner_id);
            let mut people = vec![crate::components::OverlayPerson {
                id: me.id.clone(),
                name: me.display_name.clone().into(),
                initials: me.initials.clone().into(),
                color: me.avatar_color,
                art: me.photo.clone(),
                muted: self.call_muted || self.call_deafened,
                level: level(&me.id),
            }];
            if partner_in {
                people.push(crate::components::OverlayPerson {
                    id: conversation.partner_id.clone(),
                    name: conversation.display_name.clone().into(),
                    initials: conversation.initials.clone().into(),
                    color: conversation.color,
                    art: conversation.photo.clone(),
                    muted: self.voice_state.remote_muted,
                    level: level(&conversation.partner_id),
                });
            }
            people
        });
        // Your own share if you're sharing, otherwise the one you chose to watch.
        let me = self.state.profile.id.clone();
        let partner = call.map(|conversation| conversation.partner_id.clone()).unwrap_or_default();
        let stream = match (&self.share_frame, &self.screen_frame) {
            (Some(frame), _) => Some(crate::components::OverlayStream { owner: me, frame: frame.clone() }),
            (None, Some(frame)) => Some(crate::components::OverlayStream { owner: partner, frame: frame.clone() }),
            _ => None,
        };
        let stream_same = match (&stream, &self.hud_stream) {
            (Some(a), Some(b)) => std::sync::Arc::ptr_eq(&a.frame, b),
            (None, None) => true,
            _ => false,
        };
        self.hud_stream = stream.as_ref().map(|stream| stream.frame.clone());
        let signature = (people.clone(), scale, show_names);
        if signature == self.hud_signature && stream_same {
            let animated = signature.0.as_ref().is_some_and(|people| {
                people.iter().any(|person| crate::components::art_is_animated(&person.art))
            });
            if animated && let Some(hud) = &self.hud {
                let _ = hud.update(cx, |_, _, cx| cx.notify());
            }
            return;
        }
        self.hud_signature = signature;
        let mut handle = self.hud.take();
        crate::hud::sync(&mut handle, people, scale, show_names, stream, cx);
        self.hud = handle;
    }

    pub fn delete_sound(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.writes_allowed(cx) {
            return;
        }
        self.confirming_delete = None;
        self.sound_write(cx, move || backend::delete_sound(&id));
    }

    pub fn save_sound_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.renaming_sound.take() else {
            return;
        };
        if !self.writes_allowed(cx) {
            return;
        }
        let name: String = self.sound_name_input.read(cx).value().trim().chars().take(32).collect();
        self.sound_name_input.update(cx, |state, cx| state.set_value("", window, cx));
        if name.is_empty() {
            cx.notify();
            return;
        }
        self.sound_write(cx, move || backend::rename_sound(&id, &name));
    }

    fn sound_write(
        &mut self,
        cx: &mut Context<Self>,
        write: impl FnOnce() -> Result<(), ApiError> + Send + 'static,
    ) {
        cx.spawn(async move |view, cx| {
            let result = cx.background_spawn(async move { write() }).await;
            let _ = view.update(cx, |this, cx| {
                if let Err(error) = result {
                    this.report(error, cx);
                }
                this.soundboard_status = crate::SoundboardStatus::NotLoaded;
                this.load_soundboard(cx);
            });
        })
        .detach();
    }

    /// Lists devices as the voice engine sees them, so selections are valid for calls.
    pub fn refresh_audio_devices(&mut self, cx: &mut Context<Self>) {
        // The engine holds the audio devices during a call, so keep the current list then.
        if self.voice_events.is_some() {
            return;
        }
        cx.spawn(async move |view, cx| {
            let found = cx.background_spawn(async move { crate::voice::devices() }).await;
            let Ok(found) = found else { return };
            let _ = view.update(cx, |this, cx| {
                let list = |devices: Vec<crate::voice::Device>, default: &str| {
                    std::iter::once(crate::audio_devices::AudioDevice {
                        id: "default".into(),
                        name: default.into(),
                    })
                    .chain(devices.into_iter().map(|device| crate::audio_devices::AudioDevice {
                        id: device.id,
                        name: device.name,
                    }))
                    .collect()
                };
                this.audio_devices = crate::audio_devices::SystemAudioDevices {
                    inputs: list(found.inputs, "Default microphone"),
                    outputs: list(found.outputs, "Default speakers"),
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub fn send_voice_routes(&self) {
        let route = |id: &String| (id != "default").then(|| id.clone());
        self.voice.control(crate::voice::Control::Routes {
            input: route(&self.state.preferences.input_device_id),
            output: route(&self.state.preferences.output_device_id),
        });
    }

    pub fn send_voice_controls(&self) {
        self.voice.control(crate::voice::Control::Controls {
            muted: self.call_muted,
            deafened: self.call_deafened,
        });
    }

    /// Applies updates from the voice worker and keeps its login fresh.
    pub fn start_screen_share(&mut self, id: String, kind: String, title: String, cx: &mut Context<Self>) {
        self.open_dropdown = None;
        match crate::screen_view::FrameSink::start() {
            Ok(sink) => {
                self.voice.control(crate::voice::Control::ScreenShare {
                    id,
                    kind,
                    title,
                    preview_port: sink.port,
                    preview_token: sink.token.clone(),
                });
                self.share_sink = Some(sink);
            }
            Err(_) => self.sync_error = Some("Couldn't open the screen preview.".into()),
        }
        cx.notify();
    }

    pub fn start_watching_screen(&mut self, cx: &mut Context<Self>) {
        match crate::screen_view::FrameSink::start() {
            Ok(sink) => {
                self.voice.control(crate::voice::Control::ScreenWatch {
                    port: sink.port,
                    token: sink.token.clone(),
                });
                self.screen_sink = Some(sink);
            }
            Err(_) => self.sync_error = Some("Couldn't open the screen view.".into()),
        }
        cx.notify();
    }

    pub fn stop_watching_screen(&mut self, cx: &mut Context<Self>) {
        self.voice.control(crate::voice::Control::ScreenUnwatch);
        self.end_screen_view(cx);
    }

    fn end_screen_view(&mut self, cx: &mut Context<Self>) {
        self.screen_sink = None;
        if let Some(frame) = self.screen_frame.take() {
            self.screen_retired.push(frame);
        }
        cx.notify();
    }

    fn end_share_preview(&mut self, cx: &mut Context<Self>) {
        self.share_sink = None;
        if let Some(frame) = self.share_frame.take() {
            self.screen_retired.push(frame);
        }
        cx.notify();
    }

    /// Moves the newest received frame to the screen, and frees the one it replaces.
    pub fn pump_screen_frames(&mut self, cx: &mut Context<Self>) -> bool {
        let mut changed = false;
        fn newest(sink: &Option<crate::screen_view::FrameSink>) -> Option<std::sync::Arc<gpui::RenderImage>> {
            let frame = sink.as_ref()?.take()?;
            let pixels = image::RgbaImage::from_raw(frame.width, frame.height, frame.pixels)?;
            Some(std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)])))
        }
        if let Some(next) = newest(&self.screen_sink) {
            if let Some(old) = self.screen_frame.replace(next) {
                self.screen_retired.push(old);
            }
            changed = true;
        }
        // Nothing keeps playing out of sight: another chat, Settings, or signed out.
        let in_view = self.playing_video.as_ref().is_none_or(|playing| {
            self.shell_view == crate::state::ShellView::Chat
                && self.state.conversations.get(self.state.active_partner).is_some_and(|chat| chat.id == playing.conversation)
        });
        if !in_view {
            self.stop_video();
            changed = true;
        }
        let (silent, volume) = (self.call_deafened || self.video_muted, self.video_volume);
        if let Some(playing) = &mut self.playing_video {
            if let Some(player) = &playing.player {
                player.set_muted(silent);
                player.set_volume(volume);
            }
            let frame = playing.player.as_ref().and_then(|player| player.take()).and_then(|frame| {
                let pixels = image::RgbaImage::from_raw(frame.width, frame.height, frame.pixels)?;
                Some(std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)])))
            });
            if let Some(frame) = frame {
                if let Some(old) = playing.frame.replace(frame) {
                    self.screen_retired.push(old);
                }
                changed = true;
            }
            if playing.player.as_ref().is_some_and(|player| player.ended()) {
                self.stop_video();
                changed = true;
            }
        }
        if let Some(next) = newest(&self.share_sink) {
            if let Some(old) = self.share_frame.replace(next) {
                self.screen_retired.push(old);
            }
            changed = true;
        }
        if changed {
            cx.notify();
        }
        changed
    }

    pub fn pump_voice(&mut self, cx: &mut Context<Self>) {
        let Some(events) = &self.voice_events else {
            return;
        };
        let mut latest = None;
        let mut ended = false;
        loop {
            match events.try_recv() {
                Ok(state) => latest = Some(state),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    ended = true;
                    break;
                }
            }
        }
        if let Some(state) = latest {
            if state.status != self.voice_state.status {
                crate::voice::note(&format!("status {} -> {}", self.voice_state.status, state.status));
            }
            if state.status == "failed" {
                crate::voice::note(&format!("call failed: {}", state.error.as_deref().unwrap_or("no reason")));
                self.sync_error = state.error.clone();
                ended = true;
            }
            if state.screen.notice_id != self.screen_notice_id {
                self.screen_notice_id = state.screen.notice_id;
                if state.screen.notice.is_some() {
                    self.sync_error = state.screen.notice.clone();
                }
            }
            // The flow ended watching itself, for example at its time limit.
            let was_watching = self.voice_state.screen.watch != crate::screen_share::Stage::Idle;
            if was_watching && state.screen.watch == crate::screen_share::Stage::Idle {
                self.end_screen_view(cx);
            }
            let was_sharing = self.voice_state.screen.share != crate::screen_share::Stage::Idle;
            if was_sharing && state.screen.share == crate::screen_share::Stage::Idle {
                self.end_share_preview(cx);
            }
            self.pump_remote_sounds(&state, cx);
            let me = self.state.profile.id.clone();
            let someone_else = |voice: &crate::voice::State| voice.participants.iter().any(|person| *person != me);
            let (was, now) = (self.voice_state.status.as_str(), state.status.as_str());
            if was == "solo" && now != "solo" && someone_else(&state) {
                crate::audio::play(crate::audio::UiSound::VoiceJoin);
            } else if was != "solo" && now == "solo" && someone_else(&self.voice_state) {
                crate::audio::play(crate::audio::UiSound::VoiceLeave);
            } else if was == "connected" && now == "reconnecting" {
                crate::audio::play(crate::audio::UiSound::VoiceReconnect);
            }
            let became_connected = state.receiving && !self.voice_state.receiving;
            self.voice_state = state;
            if became_connected {
                self.send_output_level();
            }
            cx.notify();
        }
        if ended {
            crate::voice::note("call ended by the voice worker");
            self.end_screen_view(cx);
            self.end_share_preview(cx);
            self.voice_events = None;
            self.voice_conversation = None;
            self.voice.control(crate::voice::Control::Stop);
            self.voice_state = Default::default();
            self.call_connected = false;
            self.call_notice = None;
            cx.notify();
            return;
        }
        // Often enough that a renewed login reaches the call before the old one runs out, which
        // it does a minute after renewal. Every 5 minutes left the call rejected for up to 4,
        // long enough for the server to drop both people from it.
        if self.voice_token_at.elapsed() >= Duration::from_secs(20) {
            self.voice_token_at = std::time::Instant::now();
            let voice = self.voice.clone();
            cx.background_spawn(async move {
                if let Ok((_, _, token)) = backend::voice_credentials() {
                    voice.control(crate::voice::Control::Token(token));
                }
            })
            .detach();
        }
    }

    /// Reads who's really in the open conversation's voice room.
    pub fn refresh_voice(&mut self, cx: &mut Context<Self>) {
        let Some(conversation) = self.state.conversations.get(self.state.active_partner) else {
            return;
        };
        let id = conversation.id.clone();
        cx.spawn(async move |view, cx| {
            let lookup = id.clone();
            let result = cx
                .background_spawn(async move { backend::load_voice(&lookup) })
                .await;
            let _ = view.update(cx, |this, cx| {
                if let (Ok((started, people)), Some(conversation)) = (
                    result,
                    this.state.conversations.iter_mut().find(|row| row.id == id),
                ) && (conversation.voice_started_at != started
                    || conversation.voice_participants != people)
                {
                    conversation.voice_started_at = started;
                    conversation.voice_participants = people;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Saves part of this account's profile.
    fn save_my_style(&mut self, fields: serde_json::Value, cx: &mut Context<Self>) {
        if !self.writes_allowed(cx) {
            return;
        }
        let user_id = self.state.profile.id.clone();
        let profile = &self.state.profile;
        let (color, weight, effect, photo) = (
            profile.name_color,
            profile.name_weight_value,
            profile.name_effect,
            profile.photo.clone(),
        );
        for conversation in &mut self.state.conversations {
            for message in conversation.messages.iter_mut().filter(|message| message.is_self) {
                message.name_color = color;
                message.name_weight = weight;
                message.name_effect = effect;
                message.photo = photo.clone();
            }
        }
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move { backend::update_profile(&user_id, fields) })
                .await;
            if let Err(error) = result {
                let _ = view.update(cx, |this, cx| this.report(error, cx));
            }
        })
        .detach();
    }

    pub fn set_name_color(&mut self, color: u32, cx: &mut Context<Self>) {
        let Some((key, _, _)) = backend::NAME_COLORS.iter().find(|(_, value, _)| *value == color) else {
            return;
        };
        self.state.profile.name_color = color;
        self.save_my_style(serde_json::json!({ "name_color": key }), cx);
    }

    pub fn set_name_weight(&mut self, weight: u16, cx: &mut Context<Self>) {
        let Some((key, _, _)) = backend::NAME_WEIGHTS.iter().find(|(_, value, _)| *value == weight) else {
            return;
        };
        self.state.profile.name_weight_value = weight;
        self.save_my_style(serde_json::json!({ "name_weight": key }), cx);
    }

    pub fn set_name_effect(&mut self, effect: Option<crate::components::NameEffect>, cx: &mut Context<Self>) {
        self.state.profile.name_effect = effect;
        let value = effect.map(|effect| effect.label().to_lowercase());
        self.save_my_style(serde_json::json!({ "name_decoration": value }), cx);
    }

    pub fn set_decoration(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.state.profile.photo.decoration = id
            .as_ref()
            .and_then(|id| self.decoration_files.get(id).cloned().flatten());
        self.state.profile.avatar_decoration = id.clone();
        self.save_my_style(serde_json::json!({ "avatar_decoration": id }), cx);
    }

    /// Loads the decoration catalogue once, and only the thumbnails currently shown.
    pub fn ensure_decorations(&mut self, cx: &mut Context<Self>) {
        if !self.decoration_catalog_requested {
            self.decoration_catalog_requested = true;
            cx.spawn(async move |view, cx| {
                let ids = cx.background_spawn(async move { backend::decoration_catalog() }).await;
                let _ = view.update(cx, |this, cx| {
                    this.decoration_catalog = ids;
                    cx.notify();
                });
            })
            .detach();
        }
        let missing: Vec<String> = self
            .decoration_catalog
            .iter()
            .take(self.decorations_shown)
            .filter(|id| !self.decoration_files.contains_key(*id))
            .cloned()
            .collect();
        if missing.is_empty() {
            return;
        }
        for id in &missing {
            self.decoration_files.insert(id.clone(), None);
        }
        cx.spawn(async move |view, cx| {
            for id in missing {
                let lookup = id.clone();
                let file = cx
                    .background_spawn(async move { backend::decoration_file(&lookup) })
                    .await;
                let alive = view.update(cx, |this, cx| {
                    this.decoration_files.insert(id, file);
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn load_soundboard(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.soundboard_status,
            crate::SoundboardStatus::Loading | crate::SoundboardStatus::Loaded
        ) {
            return;
        }
        self.soundboard_status = crate::SoundboardStatus::Loading;
        let conversation = self.voice_conversation.clone();
        let my_id = self.state.profile.id.clone();
        let mut owners: std::collections::HashMap<String, (String, u32, crate::components::AvatarImage)> = self
            .state
            .conversations
            .iter()
            .map(|conversation| {
                (
                    conversation.partner_id.clone(),
                    (
                        conversation.display_name.clone(),
                        conversation.color,
                        conversation.photo.clone(),
                    ),
                )
            })
            .collect();
        owners.insert(
            my_id.clone(),
            (
                self.state.profile.display_name.clone(),
                self.state.profile.avatar_color,
                self.state.profile.photo.clone(),
            ),
        );
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move { backend::load_sounds(&my_id, &owners, conversation.as_deref()) })
                .await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(clips) => {
                        this.soundboard_clips = clips;
                        this.soundboard_status = crate::SoundboardStatus::Loaded;
                    }
                    Err(error) => {
                        this.soundboard_status = crate::SoundboardStatus::Failed(error.message());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    // -----------------------------------------------------------------------
    // Writes
    // -----------------------------------------------------------------------

    pub fn choose_attachment(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Attach".into()),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let _ = view.update(cx, |this, cx| this.attach_file(path, cx));
        })
        .detach();
    }

    pub fn attach_file(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        if self.media_busy.is_some() {
            return;
        }
        let progress = std::sync::Arc::new(crate::compress::Progress::default());
        self.media_progress = Some(progress.clone());
        self.prepare_attachment(cx, move || crate::chat_media::from_file(&path, &progress));
    }

    /// Files dropped on the chat. One message holds one attachment, so that's the first file.
    pub fn attach_dropped(&mut self, paths: &[std::path::PathBuf], cx: &mut Context<Self>) {
        let Some(first) = paths.first() else { return };
        if self.media_busy.is_some() {
            self.sync_error = Some("Wait for the current attachment to finish first.".into());
            cx.notify();
            return;
        }
        if paths.len() > 1 {
            self.sync_error = Some("Only the first file was attached. Send it, then drop the next.".into());
        }
        self.attach_file(first.clone(), cx);
    }

    pub fn cancel_preparing_media(&mut self) {
        if let Some(progress) = &self.media_progress {
            progress.cancelled.store(true, std::sync::atomic::Ordering::Release);
        }
    }

    /// Starts or stops the snow. Moving it redraws the window, so it's kept to 25 frames a second
    /// and holds still while another window has focus, a game for instance.
    pub fn set_snow(&mut self, on: bool, cx: &mut Context<Self>) {
        if on == self.snow.is_some() {
            return;
        }
        self.snow = on.then(|| {
            let task = cx.spawn(async move |view, cx| {
                loop {
                    gpui::Timer::after(Duration::from_millis(40)).await;
                    let drawn = view.update(cx, |this, cx| {
                        if this.window_active {
                            cx.notify();
                        }
                    });
                    if drawn.is_err() {
                        break;
                    }
                }
            });
            (std::time::Instant::now(), task)
        });
        cx.notify();
    }

    /// Fetches the next ten messages with media for the sidebar. These are only rows: pictures
    /// still load when their card is on screen, and once fetched come from the disk cache.
    pub fn load_media_page(&mut self, cx: &mut Context<Self>) {
        let Some((conversation_id, who, _)) = self.active_request() else { return };
        let pages = self.sidebar_media.entry(conversation_id.clone()).or_default();
        if pages.loading || pages.done || pages.retry_at.is_some_and(|at| at > std::time::Instant::now()) {
            return;
        }
        pages.loading = true;
        let before = pages.messages.first().map(|message| message.created_at.clone());
        cx.spawn(async move |view, cx| {
            let id = conversation_id.clone();
            let result = cx
                .background_spawn(async move { backend::load_media_page(&id, before.as_deref(), &who) })
                .await;
            let _ = view.update(cx, |this, cx| {
                let pages = this.sidebar_media.entry(conversation_id.clone()).or_default();
                pages.loading = false;
                match result {
                    Ok((mut older, has_more)) => {
                        pages.done = !has_more;
                        older.append(&mut pages.messages);
                        pages.messages = older;
                        let older = &pages.messages;
                        if let Some(conversation) = this.state.conversations.iter_mut().find(|row| row.id == conversation_id) {
                            conversation.shared_media = backend::media_items_with(&conversation.messages, older);
                        }
                    }
                    Err(_) => pages.retry_at = Some(std::time::Instant::now() + Duration::from_secs(30)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Carries on towards a sidebar item whose message isn't in the loaded chat yet.
    pub fn pump_message_jump(&mut self, cx: &mut Context<Self>) {
        const MOST_PAGES: u32 = 40;
        let Some((id, pages)) = self.message_jump.clone() else { return };
        if self.message_tops.borrow().contains_key(&id) {
            self.message_jump = None;
            self.scroll_to_message(id, cx);
            return;
        }
        if self.loading_older {
            return;
        }
        if pages >= MOST_PAGES || !self.active_conv().has_more {
            self.message_jump = None;
            return;
        }
        self.message_jump = Some((id, pages + 1));
        self.load_older_messages(cx);
    }

    /// Scrolls chat to a message from the media sidebar, and tints it for a moment.
    pub fn scroll_to_message(&mut self, id: String, cx: &mut Context<Self>) {
        const BELOW_HEADER: f32 = 72.;
        let Some(top) = self.message_tops.borrow().get(&id).copied() else {
            self.message_jump = Some((id, 0));
            return;
        };
        let mut offset = self.chat_scroll.offset();
        offset.y += gpui::px(BELOW_HEADER - top);
        self.chat_scroll.set_offset(offset);
        self.highlighted_message = Some(id.clone());
        cx.notify();
        cx.spawn(async move |view, cx| {
            gpui::Timer::after(Duration::from_millis(1400)).await;
            let _ = view.update(cx, |this, cx| {
                if this.highlighted_message.as_ref() == Some(&id) {
                    this.highlighted_message = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn attach_clipboard_media(&mut self, media: crate::clipboard::ClipboardMedia, cx: &mut Context<Self>) {
        use crate::clipboard::ClipboardMedia;
        match media {
            ClipboardMedia::File(path) => self.attach_file(path, cx),
            ClipboardMedia::Image(bytes) => {
                self.prepare_attachment(cx, move || crate::chat_media::from_bytes(bytes, "Pasted image".into()));
            }
        }
    }

    fn prepare_attachment(
        &mut self,
        cx: &mut Context<Self>,
        read: impl FnOnce() -> Result<crate::chat_media::PendingMedia, String> + Send + 'static,
    ) {
        self.media_busy = Some("Preparing media…");
        cx.notify();
        // Keeps the percentage moving while the work happens elsewhere.
        cx.spawn(async move |view, cx| {
            loop {
                gpui::Timer::after(Duration::from_millis(250)).await;
                let busy = view.update(cx, |this, cx| {
                    let preparing = this.media_progress.is_some();
                    if preparing {
                        cx.notify();
                    }
                    preparing
                });
                if !matches!(busy, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |view, cx| {
            let result = cx.background_spawn(async move { read() }).await;
            let _ = view.update(cx, |this, cx| {
                this.media_busy = None;
                this.media_progress = None;
                match result {
                    Ok(media) => this.set_pending_media(Some(media)),
                    // Empty means it was cancelled, which needs no message.
                    Err(message) if message.is_empty() => {}
                    Err(message) => this.sync_error = Some(message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn set_pending_media(&mut self, media: Option<crate::chat_media::PendingMedia>) {
        if let Some(old) = std::mem::replace(&mut self.pending_media, media) {
            self.screen_retired.push(old.thumbnail);
        }
    }

    /// Sends what's attached, with whatever's typed as its caption.
    fn send_pending_media(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.media_busy.is_some() || !self.chat_writes_allowed(cx) {
            return;
        }
        let caption = self.chat_input.read(cx).value().trim().to_string();
        if caption.chars().count() > 4000 {
            self.sync_error = Some("Messages can be up to 4000 characters.".into());
            cx.notify();
            return;
        }
        let Some(media) = self.pending_media.take() else { return };
        self.chat_replying_to = None;
        let reply_target = self.chat_reply_target.take();
        let (conversation, sender) = (self.active_conv().id.clone(), self.state.profile.id.clone());
        self.chat_input.update(cx, |state, cx| state.set_value("", window, cx));
        self.pending_media = Some(media);
        self.media_busy = Some("Uploading media…");
        // It stays on screen while uploading, so this holds it for the background call.
        let upload = self.pending_media.as_ref().map(|media| crate::chat_media::PendingUpload {
            bytes: media.bytes.clone(),
            upload_mime: media.upload_mime,
            width: media.width,
            height: media.height,
            seconds: media.seconds,
        });
        cx.notify();
        let Some(upload) = upload else { return };
        cx.spawn(async move |view, cx| {
            let id = Uuid::new_v4().to_string();
            let result = cx
                .background_spawn(async move {
                    backend::send_media_message(&id, &conversation, &sender, &caption, reply_target.as_deref(), &upload)
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                this.media_busy = None;
                match result {
                    Ok(()) => {
                        this.set_pending_media(None);
                        this.poll_soon = true;
                        this.chat_scroll.scroll_to_bottom();
                    }
                    Err(error) => this.report(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn send_chat_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_media.is_some() {
            self.send_pending_media(window, cx);
            return;
        }
        let raw = self.chat_input.read(cx).value().trim().to_string();
        let command = raw.split_whitespace().next().filter(|word| word.starts_with('/'));
        if raw.is_empty() {
            return;
        }
        if self.state.conversations.is_empty() || !self.chat_writes_allowed(cx) {
            return;
        }
        // /ping reports locally: there's no real voice session to measure yet.
        if command == Some("/ping") {
            self.chat_input
                .update(cx, |state, cx| state.set_value("", window, cx));
            let me = person_me(&self.state.profile);
            let stamp = backend::now_stamp();
            let conversation = self.active_conv_mut();
            backend::merge_messages(
                &mut conversation.messages,
                vec![ChatMessage {
                    id: backend::next_row_id(),
                system: false,
                    db_id: format!("local-{}", Uuid::new_v4()),
                    pending: false,
                    sender: me.name,
                    initials: me.initials,
                    avatar_color: me.color,
                    photo: me.photo,
                    name_color: me.name_color,
                    name_weight: me.name_weight,
                name_effect: me.name_effect,
                    media_path: None,
                    media_size: None,
                media_bytes: None,
                    role_badge: None,
                    time: backend::clock_label(&stamp),
                    created_at: stamp,
                    text: "Only you can see this · no voice session to ping".into(),
                    reply_to: None,
                    attachment: ChatAttachment::None,
                    reactions: vec![],
                    is_self: true,
                    pinned: false,
                    edited: false,
                    deleted: false,
                    marker_before: None,
                }],
            );
            self.chat_scroll.scroll_to_bottom();
            cx.notify();
            return;
        }
        let content = match command {
            Some("/shrug") => {
                let rest = raw["/shrug".len()..].trim();
                if rest.is_empty() {
                    "¯\\_(ツ)_/¯".to_string()
                } else {
                    format!("{rest} ¯\\_(ツ)_/¯")
                }
            }
            _ => raw.clone(),
        };
        if content.is_empty() {
            return;
        }
        if content.chars().count() > 4000 {
            self.sync_error = Some("Messages can be up to 4000 characters.".into());
            cx.notify();
            return;
        }
        let reply_label = self.chat_replying_to.take();
        let reply_target = self.chat_reply_target.take();
        let me = person_me(&self.state.profile);
        let conversation_id = self.active_conv().id.clone();
        let message_id = Uuid::new_v4().to_string();
        let stamp = backend::now_stamp();
        let conversation = self.active_conv_mut();
        backend::merge_messages(
            &mut conversation.messages,
            vec![ChatMessage {
                id: backend::next_row_id(),
                system: false,
                db_id: message_id.clone(),
                pending: true,
                sender: me.name,
                initials: me.initials,
                avatar_color: me.color,
                photo: me.photo,
                name_color: me.name_color,
                name_weight: me.name_weight,
                name_effect: me.name_effect,
                media_path: None,
                media_size: None,
                media_bytes: None,
                role_badge: None,
                time: backend::clock_label(&stamp),
                created_at: stamp,
                text: content.clone(),
                reply_to: reply_label,
                attachment: ChatAttachment::None,
                reactions: vec![],
                is_self: true,
                pinned: false,
                edited: false,
                deleted: false,
                marker_before: None,
            }],
        );
        self.chat_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.chat_scroll.scroll_to_bottom();
        self.sync_error = None;
        cx.notify();

        let sender = me.id;
        cx.spawn(async move |view, cx| {
            let id = message_id.clone();
            let conv = conversation_id.clone();
            let result = cx
                .background_spawn(async move {
                    backend::send_message(&id, &conv, &sender, &content, reply_target.as_deref())
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                let row = this
                    .state
                    .conversations
                    .iter_mut()
                    .find(|conversation| conversation.id == conversation_id);
                if let Some(conversation) = row {
                    match &result {
                        Ok(()) => {
                            if let Some(message) = conversation
                                .messages
                                .iter_mut()
                                .find(|message| message.db_id == message_id)
                            {
                                message.pending = false;
                            }
                        }
                        Err(_) => conversation
                            .messages
                            .retain(|message| message.db_id != message_id),
                    }
                }
                match result {
                    Ok(()) => cx.notify(),
                    Err(error) => this.report(error, cx),
                }
            });
        })
        .detach();
    }

    /// Called as the composer's text changes. Main Nitro's 2.5 seconds, for your own line.
    pub fn composer_changed(&mut self, has_text: bool) {
        if !has_text {
            self.self_typing_until = None;
            return;
        }
        self.self_typing_until = Some(std::time::Instant::now() + std::time::Duration::from_millis(2500));
        let user = self.state.profile.id.clone();
        if let Some(channel) = &mut self.typing {
            channel.send_typing(&user);
        }
    }

    /// Keeps every conversation's online dot current, including lists loaded since.
    pub fn pump_presence(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.state.auth_status, crate::state::AuthStatus::SignedIn)
            || self.state.profile.id.is_empty()
        {
            self.presence = None;
            return;
        }
        let user = self.state.profile.id.clone();
        let channel = self
            .presence
            .get_or_insert_with(|| crate::realtime::PresenceChannel::start(user));
        channel.refresh();
        if self.window_active {
            channel.note_active();
        }
        let mut changed = false;
        for conversation in &mut self.state.conversations {
            let status = channel.status(&conversation.partner_id);
            if conversation.presence != status {
                conversation.presence = status.to_string();
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
    }

    pub fn pump_typing(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.state.auth_status, crate::state::AuthStatus::SignedIn)
            || self.state.conversations.is_empty()
        {
            self.typing = None;
            return;
        }
        let conversation = self.active_conv();
        let (id, partner) = (conversation.id.clone(), conversation.partner_id.clone());
        let channel = self.typing.get_or_insert_with(crate::realtime::TypingChannel::start);
        channel.watch(&id);
        if channel.take().iter().any(|user| *user == partner) {
            self.partner_typing_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
            cx.notify();
        }
        let now = std::time::Instant::now();
        for until in [&mut self.partner_typing_until, &mut self.self_typing_until] {
            if until.is_some_and(|until| now >= until) {
                *until = None;
                cx.notify();
            }
        }
    }

    pub fn start_editing(&mut self, db_id: String, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.reaction_picker = None;
        self.editing_message = Some(db_id);
        self.edit_input.update(cx, |input, cx| {
            input.set_value(text, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub fn cancel_editing(&mut self, cx: &mut Context<Self>) {
        self.editing_message = None;
        cx.notify();
    }

    pub fn save_edit(&mut self, cx: &mut Context<Self>) {
        let Some(db_id) = self.editing_message.take() else { return };
        cx.notify();
        let text = self.edit_input.read(cx).value().trim().to_string();
        let found = self.active_conv().messages.iter().find(|message| message.db_id == db_id);
        if text.is_empty() || text.len() > 4000 || found.is_none_or(|message| message.text == text) {
            return;
        }
        if !self.chat_writes_allowed(cx) {
            return;
        }
        if let Some(message) = self.active_conv_mut().messages.iter_mut().find(|m| m.db_id == db_id) {
            message.text = text.clone();
            message.edited = true;
        }
        cx.spawn(async move |view, cx| {
            let result = cx.background_spawn(async move { backend::edit_message(&db_id, &text) }).await;
            if let Err(error) = result {
                let _ = view.update(cx, |this, cx| this.report(error, cx));
            }
        })
        .detach();
    }

    /// Deletes these messages, which together make up one card.
    pub fn delete_messages(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        if !self.chat_writes_allowed(cx) {
            return;
        }
        self.reaction_picker = None;
        for message in &mut self.active_conv_mut().messages {
            if ids.contains(&message.db_id) {
                message.deleted = true;
            }
        }
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    ids.iter().try_for_each(|id| backend::delete_message(id))
                })
                .await;
            if let Err(error) = result {
                let _ = view.update(cx, |this, cx| this.report(error, cx));
            }
        })
        .detach();
    }

    pub fn toggle_reaction(&mut self, message_index: usize, emoji: &str, cx: &mut Context<Self>) {
        if !self.chat_writes_allowed(cx) {
            return;
        }
        let user_id = self.state.profile.id.clone();
        let Some(message) = self.active_conv_mut().messages.get_mut(message_index) else {
            return;
        };
        if message.pending || message.db_id.starts_with("local-") {
            return;
        }
        let message_id = message.db_id.clone();
        crate::audio::play(crate::audio::UiSound::Reaction);
        let removing = message
            .reactions
            .iter()
            .find(|chip| chip.emoji == emoji && chip.mine)
            .and_then(|chip| chip.mine_row.clone());
        match message.reactions.iter_mut().find(|chip| chip.emoji == emoji) {
            Some(chip) if chip.mine => {
                chip.mine = false;
                chip.mine_row = None;
                chip.count = chip.count.saturating_sub(1);
            }
            Some(chip) => {
                chip.mine = true;
                chip.count += 1;
            }
            None => message.reactions.push(ReactionChip {
                emoji: emoji.to_string(),
                count: 1,
                mine: true,
                mine_row: None,
            }),
        }
        message.reactions.retain(|chip| chip.count > 0);
        cx.notify();
        let emoji = emoji.to_string();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    match removing {
                        Some(row) => backend::remove_reaction(&row),
                        None => backend::add_reaction(&message_id, &user_id, &emoji),
                    }
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                // Re-read so row ids and counts match the server, or undo on failure.
                this.load_active(cx);
                if let Err(error) = result {
                    this.report(error, cx);
                }
            });
        })
        .detach();
    }

    fn friend_write(
        &mut self,
        cx: &mut Context<Self>,
        write: impl FnOnce() -> Result<(), ApiError> + Send + 'static,
    ) {
        if !self.writes_allowed(cx) {
            return;
        }
        self.friends_error = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx.background_spawn(async move { write() }).await;
            let _ = view.update(cx, |this, cx| match result {
                Ok(()) => this.reload_snapshot(cx),
                Err(ApiError::SignedOut) => this.report(ApiError::SignedOut, cx),
                Err(error) => {
                    this.friends_error = Some(error.message());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn request_friend(&mut self, username: String, cx: &mut Context<Self>) {
        let my_id = self.state.profile.id.clone();
        let my_username = self.state.profile.username.clone();
        self.friend_write(cx, move || {
            backend::send_friend_request(&username, &my_id, &my_username)
        });
    }

    pub fn accept_friend(&mut self, id: String, cx: &mut Context<Self>) {
        self.friend_write(cx, move || backend::accept_friend_request(&id));
    }

    pub fn remove_friendship(&mut self, id: String, cx: &mut Context<Self>) {
        self.friend_write(cx, move || backend::delete_friendship(&id));
    }

    pub fn finish_onboarding(&mut self, cx: &mut Context<Self>) {
        if self.auth_busy.is_some() || !self.writes_allowed(cx) {
            return;
        }
        let user_id = self.state.profile.id.clone();
        let display_name = self.state.profile.display_name.clone();
        self.state.profile.initials = initials_for(&display_name);
        self.auth_busy = Some("Saving your profile…".into());
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    backend::complete_onboarding(&user_id, &display_name)
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                this.auth_busy = None;
                match result {
                    Ok(()) => {
                        this.state.profile.onboarding_completed = true;
                        this.state.auth_status = AuthStatus::SignedIn;
                        this.shell_view = ShellView::Chat;
                        this.load_active(cx);
                    }
                    Err(error) => this.report(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
