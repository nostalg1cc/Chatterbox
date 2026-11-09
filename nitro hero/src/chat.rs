use std::time::Duration;

use gpui::{
    Context, Div, InteractiveElement, MouseButton,
    ParentElement, Pixels, SharedString, Stateful, StatefulInteractiveElement, Styled,
    StyledImage, Window, div, prelude::*, px, rgba, svg,
};
use gpui_component::input::Input;

use crate::components::rgb;
use crate::{
    Preview,
    components::{
        self, ACCENT, MATERIAL_DARK, MATERIAL_GREEN, MATERIAL_PANEL, MATERIAL_RED, MATERIAL_SUBTLE,
        StrokeKind, badge, label, material_surface,
    },
    state::{AuthStatus, ChatAttachment, ChatMessage, HeaderDropdown, ShellView},
};

fn material_interactive(
    id: impl Into<gpui::ElementId>,
    fill_hex: u32,
    radius: f32,
    stroke: StrokeKind,
) -> Stateful<Div> {
    let elem_id: gpui::ElementId = id.into();
    let group: SharedString = format!("mi-{elem_id:?}").into();
    components::material_interactive(elem_id, group, fill_hex, radius, stroke)
}

fn avatar(
    initials: impl Into<SharedString>,
    bg_hex: u32,
    size: f32,
    image: impl Into<components::AvatarImage>,
) -> Div {
    components::avatar(initials, size, bg_hex, 0xffffff, image)
}

fn alert(
    icon: &'static str,
    color: u32,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    action: Option<Stateful<Div>>,
) -> Div {
    components::alert(icon, title, description, color).when_some(action, |d, a| d.child(a))
}

/// The ambient green glow while in a call. The chat and sidebar each paint it at the same
/// position, above scrolling content but below their controls, so it reads as one gradient.
fn call_glow() -> Div {
    div()
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .right(px(0.))
        .h(px(160.))
        .bg(gpui::linear_gradient(
            180.,
            gpui::linear_color_stop(rgba(0x00b3392e), 0.0),
            gpui::linear_color_stop(rgba(0x00b33900), 1.0),
        ))
}

/// Narrower than this, the sidebar would squeeze the chat too far.
const SIDEBAR_NEEDS: f32 = 940.;

/// The background as it looks under the top of `call_glow`.
fn with_call_glow(background: u32) -> u32 {
    const GLOW: u32 = 0x00b339;
    const STRENGTH: f32 = 0.17;
    let channel = |shift: u32| {
        let (from, to) = (((background >> shift) & 0xff) as f32, ((GLOW >> shift) & 0xff) as f32);
        ((from + (to - from) * STRENGTH).round() as u32) << shift
    };
    channel(16) | channel(8) | channel(0)
}

/// Height for a message image shown at the card's 400px width.
/// An image's size in chat: its own shape, no larger than it really is or than fits.
fn message_image_size(size: Option<(u32, u32)>) -> (f32, f32) {
    const WIDEST: f32 = 420.;
    const TALLEST: f32 = 360.;
    const SMALLEST: f32 = 64.;
    let Some((width, height)) = size.filter(|(width, height)| *width > 0 && *height > 0) else {
        return (320., 240.);
    };
    let (width, height) = (width as f32, height as f32);
    let scale = (WIDEST / width).min(TALLEST / height).min(1.);
    ((width * scale).max(SMALLEST), (height * scale).max(SMALLEST))
}

/// Message text you can select, copy and click links in.
fn message_text(
    message: usize,
    text: String,
    selection: std::rc::Rc<std::cell::RefCell<Option<crate::links::TextSelection>>>,
    focus: gpui::FocusHandle,
) -> Div {
    use crate::links;
    let found = std::rc::Rc::new(links::find(&text));
    let selected = selection
        .borrow()
        .as_ref()
        // Its text is compared because a message can be edited while part of it is selected.
        .filter(|current| current.message == message && *current.text == text)
        .map(links::TextSelection::range);
    let styled = gpui::StyledText::new(text.clone()).with_highlights(links::highlights(
        &found,
        selected,
        gpui::rgb(0x7ab8ff).into(),
        gpui::rgba(0x4d8dff66).into(),
    ));
    let layout = styled.layout().clone();
    let text = std::rc::Rc::new(text);
    let nearest = {
        let layout = layout.clone();
        move |position| layout.index_for_position(position).unwrap_or_else(|closest| closest)
    };
    let link_at = {
        let (layout, found) = (layout.clone(), found.clone());
        move |position| {
            let index = layout.index_for_position(position).ok()?;
            found.iter().find(|range| range.start <= index && index < range.end).cloned()
        }
    };
    // The press is on the element, so anything in front of the text blocks it as usual.
    let (press_selection, press_text, press_nearest, press_focus) =
        (selection.clone(), text.clone(), nearest.clone(), focus.clone());
    div()
        .relative()
        .cursor_text()
        .on_mouse_down(MouseButton::Left, move |event, window, _| {
            let index = links::on_character(&press_text, press_nearest(event.position));
            let range = match event.click_count {
                1 => index..index,
                2 => links::word_at(&press_text, index),
                _ => 0..press_text.len(),
            };
            *press_selection.borrow_mut() = Some(links::TextSelection {
                message,
                text: press_text.clone(),
                anchor: range.start,
                head: range.end,
                dragging: event.click_count == 1,
            });
            if event.click_count > 1 {
                press_focus.focus(window);
            }
            window.refresh();
        })
        .child(styled)
        .child(
            gpui::canvas(
                // An area over each line of each link. GPUI picks the cursor from these on every
                // mouse move, without having to paint again.
                {
                    let (layout, found) = (layout.clone(), found.clone());
                    move |bounds, window, _| {
                        let line = layout.line_height();
                        let mut areas = Vec::new();
                        for range in found.iter() {
                            let (Some(from), Some(to)) =
                                (layout.position_for_index(range.start), layout.position_for_index(range.end))
                            else {
                                continue;
                            };
                            let mut area = |left: Pixels, top: Pixels, right: Pixels| {
                                let corners = gpui::Bounds::from_corners(gpui::point(left, top), gpui::point(right, top + line));
                                areas.push(window.insert_hitbox(corners, gpui::HitboxBehavior::Normal));
                            };
                            let (mut left, mut top) = (from.x, from.y);
                            while top + px(1.) < to.y {
                                area(left, top, bounds.right());
                                left = bounds.left();
                                top += line;
                            }
                            area(left, top, to.x);
                        }
                        areas
                    }
                },
                move |bounds, areas, window, _| {
                    for area in &areas {
                        window.set_cursor_style(gpui::CursorStyle::PointingHand, area);
                    }
                    // A press anywhere else ends this selection. Capture runs before the
                    // press on the text, so a press on the same text starts it afresh.
                    let outside = selection.clone();
                    window.on_mouse_event(move |event: &gpui::MouseDownEvent, phase, window, _| {
                        let mine = outside.borrow().as_ref().is_some_and(|current| current.message == message);
                        if phase.capture() && mine && !bounds.contains(&event.position) {
                            *outside.borrow_mut() = None;
                            window.refresh();
                        }
                    });
                    let (moving, moving_text) = (selection.clone(), text.clone());
                    window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, window, _| {
                        if !phase.capture() {
                            return;
                        }
                        let mut current = moving.borrow_mut();
                        let Some(active) = current.as_mut().filter(|current| current.message == message && current.dragging) else {
                            return;
                        };
                        // The button came up where this text couldn't see it, such as outside
                        // the window, or while the message was scrolled away.
                        if event.pressed_button != Some(MouseButton::Left) {
                            active.dragging = false;
                            if active.anchor == active.head {
                                *current = None;
                            }
                            window.refresh();
                            return;
                        }
                        let head = links::on_character(&moving_text, nearest(event.position));
                        if head != active.head {
                            active.head = head;
                            window.refresh();
                        }
                    });
                    let released = selection.clone();
                    window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, window, cx| {
                        if !phase.capture() || event.button != MouseButton::Left {
                            return;
                        }
                        let mut current = released.borrow_mut();
                        let Some(active) = current.as_mut().filter(|current| current.message == message && current.dragging) else {
                            return;
                        };
                        active.dragging = false;
                        if active.anchor == active.head {
                            *current = None;
                            window.refresh();
                            if let Some(link) = link_at(event.position) {
                                cx.open_url(&text[link]);
                            }
                        } else {
                            // A plain click leaves focus alone, so typing carries on in the composer.
                            focus.focus(window);
                        }
                    });
                },
            )
            .absolute()
            .size_full(),
        )
}

impl Preview {
    /// An image from chat storage. It asks to be fetched only once it's near the viewport.
    /// A video in chat, filling whatever holds it: its first picture, then its frames while
    /// it plays. `height` is the tile's own, for the picture behind it.
    fn render_video_tile(&self, id: SharedString, path: Option<String>, duration: String, width: f32, height: f32, in_grid: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let playing = self
            .playing_video
            .as_ref()
            .filter(|playing| path.as_ref().is_some_and(|path| *path == playing.path));
        let frame = playing.and_then(|playing| playing.frame.clone());
        let loading = playing.is_some_and(|playing| playing.player.is_none());
        let active = playing.is_some();
        let paused = playing.and_then(|playing| playing.player.as_ref()).is_some_and(|player| player.paused());
        let controls = if active && !loading { self.render_video_controls(width, false, cx) } else { None };
        let group: SharedString = format!("{id}-tile").into();
        // A message's own video has its own shape already. A grid tile doesn't, so it fills.
        let fit = if in_grid { gpui::ObjectFit::Cover } else { gpui::ObjectFit::Contain };
        let picture = self.render_media_picture(path.clone(), height, fit, false);
        material_surface(MATERIAL_PANEL, 12., StrokeKind::Neutral)
            .id(id)
            .group(group.clone())
            .relative()
            .size_full()
            .overflow_hidden()
            .cursor_pointer()
            .when_some(path, |tile, path| {
                tile.on_click(cx.listener(move |this, _, _, cx| this.toggle_video(path.clone(), cx)))
            })
            .map(|tile| match frame {
                Some(frame) => tile.child(components::video_frame(frame, 12.)),
                None => tile.child(picture),
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!active || loading || paused, |center| {
                        center.child(
                            material_surface(MATERIAL_DARK, 22., StrokeKind::Neutral)
                                .size(px(44.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .map(|button| {
                                    if loading {
                                        button.text_size(px(11.)).text_color(rgb(0xe4e4e7)).child("…")
                                    } else {
                                        button.child(svg().path("Play").size(px(18.)).text_color(rgb(0xffffff)))
                                    }
                                }),
                        )
                    }),
            )
            .when(!active, |tile| {
                tile.child(
                    div().absolute().bottom(px(8.)).right(px(8.)).child(badge(duration, MATERIAL_DARK, 0xf2f2f3)),
                )
            })
            // Out of the way while it plays, back when the mouse is over it or it's paused.
            .when_some(controls, |tile, controls| {
                tile.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .opacity(if paused { 1. } else { 0. })
                        .group_hover(group, |style| style.opacity(1.))
                        .child(controls),
                )
            })
    }

    /// The preview cards for a message's links, each fetched once it's near the viewport.
    fn render_link_cards(&self, message: usize, text: &str, cx: &mut Context<Self>) -> Option<Div> {
        let mut urls: Vec<String> = Vec::new();
        for range in crate::links::find(text) {
            let url = text[range].to_string();
            if !urls.contains(&url) {
                urls.push(url);
            }
        }
        urls.truncate(crate::links::MAX_CARDS);
        if urls.is_empty() {
            return None;
        }
        let mut column = div().mt(px(6.)).flex().flex_col().gap(px(6.));
        for (index, url) in urls.into_iter().enumerate() {
            match self.link_cards.get(&url) {
                Some(crate::links::LinkState::Ready(loaded)) => {
                    // Checked again here, as this one came from the server or a saved file.
                    let target = Some(loaded.card.url.clone()).filter(|url| crate::links::can_open(url));
                    column = column.child(
                        div()
                            .id(SharedString::from(format!("link-card-{message}-{index}")))
                            .w(px(components::LINK_CARD_WIDTH))
                            .cursor_pointer()
                            .hover(|card| card.opacity(0.92))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                if let Some(target) = &target {
                                    cx.open_url(target);
                                }
                            }))
                            .child(components::link_card(components::LinkCardView {
                                site: loaded.card.site.clone().into(),
                                title: loaded.card.title.clone().into(),
                                description: loaded.card.description.clone().into(),
                                image: loaded.image.clone(),
                                aspect: loaded.aspect,
                                playable: loaded.card.playable,
                            })),
                    );
                }
                Some(_) => {}
                None => {
                    let wanted = self.link_wanted.clone();
                    column = column.child(
                        gpui::canvas(
                            move |bounds, window, _| {
                                let viewport = f32::from(window.viewport_size().height);
                                let top = f32::from(bounds.origin.y);
                                // Not while it's above the view: a card appearing there would
                                // push down whatever is being read.
                                if top < viewport + 300. && top > -40. {
                                    wanted.borrow_mut().push(url.clone());
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .w_full()
                        .h(px(1.)),
                    );
                }
            }
        }
        Some(column)
    }

    fn render_chat_image(&self, path: Option<String>, height: f32) -> Div {
        self.render_media_image(path, height, gpui::ObjectFit::Contain)
    }

    fn render_media_image(&self, path: Option<String>, height: f32, fit: gpui::ObjectFit) -> Div {
        self.render_media_picture(path, height, fit, true)
    }

    /// `opens` is whether pressing it shows it full size. The sidebar and video tiles do
    /// something else when pressed.
    fn render_media_picture(&self, path: Option<String>, height: f32, fit: gpui::ObjectFit, opens: bool) -> Div {
        let state = path.as_ref().and_then(|path| self.media_files.get(path));
        let ready = match state {
            Some(crate::MediaFile::Ready { original, sidebar }) => {
                if matches!(fit, gpui::ObjectFit::Cover) { sidebar.clone() } else { Some(original.clone()) }
            },
            _ => None,
        };
        let wanted = self.media_wanted.clone();
        let request = path.filter(|_| state.is_none());
        let original = match state {
            Some(crate::MediaFile::Ready { original, .. }) if opens => Some(original.clone()),
            _ => None,
        };
        let lightbox = self.lightbox_request.clone();
        div()
            .relative()
            .w_full()
            .h(px(height))
            .rounded(px(12.))
            .overflow_hidden()
            .when(ready.is_none(), |d| d.bg(rgb(crate::state::palette().media)))
            .flex()
            .items_center()
            .justify_center()
            .when_some(original, |d, original| {
                d.cursor_pointer().on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                    *lightbox.borrow_mut() = Some(original.clone());
                })
            })
            .map(|d| match ready {
                Some(file) => d.child(
                    gpui::img(file)
                        .size_full()
                        .rounded(px(if matches!(fit, gpui::ObjectFit::Cover) { 10.75 } else { 12. }))
                        .object_fit(if matches!(fit, gpui::ObjectFit::Cover) { gpui::ObjectFit::Fill } else { fit }),
                ),
                None => d.child(svg().path("Picture").size(px(24.)).text_color(rgb(0x6d6d73))),
            })
            .child(
                gpui::canvas(
                    move |bounds, window, _| {
                        let Some(path) = &request else { return };
                        let viewport = f32::from(window.viewport_size().height);
                        let top = f32::from(bounds.origin.y);
                        if top < viewport + 300. && top + f32::from(bounds.size.height) > -300. {
                            wanted.borrow_mut().push(path.clone());
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
    }

    pub fn active_conv(&self) -> &crate::state::PartnerConversation {
        self.state
            .conversations
            .get(self.state.active_partner)
            .unwrap_or(&self.empty_conversation)
    }

    pub fn active_conv_mut(&mut self) -> &mut crate::state::PartnerConversation {
        let index = self.state.active_partner;
        self.state
            .conversations
            .get_mut(index)
            .unwrap_or(&mut self.empty_conversation)
    }

    /// Global Windows-style caption controls + Settings & Media Sidebar toggles
    /// at the top-right (`top: 0, right: 0, height: 32px, background: transparent`),
    /// matching `src/components/titlebar.tsx` and `.global-window-controls` in `styles.css`.
    #[inline(never)]
    pub fn render_window_controls(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let signed_in = self.state.auth_status == AuthStatus::SignedIn;
        let in_chat = self.shell_view == ShellView::Chat && self.view_mode == 0;
        let in_settings = self.shell_view == ShellView::Settings && self.view_mode == 0;
        let sidebar_open = self.media_sidebar_open;

        div()
            .id("global-window-controls")
            .absolute()
            .top(px(0.))
            .right(px(0.))
            .h(px(32.))
            .bg(gpui::transparent_black())
            .flex()
            .items_center()
            .gap(px(0.))
            // Mode pill (Nitro App vs UI Gallery), for test builds only, tucked cleanly to the left of caption strip
            .when(cfg!(debug_assertions), |row| row.child(
                div()
                    .id("toggle-ui-gallery-pill")
                    .cursor_pointer()
                    .mr(px(6.))
                    .h(px(22.))
                    .px(px(9.))
                    .rounded_full()
                    .bg(rgba(0xffffff0d))
                    .hover(|s| s.bg(rgba(0xffffff1c)))
                    .flex()
                    .items_center()
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(if self.view_mode == 1 {
                        0xffffff
                    } else {
                        0xa1a1aa
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.view_mode = if this.view_mode == 0 { 1 } else { 0 };
                        cx.notify();
                    }))
                    .child(if self.view_mode == 0 {
                        "UI gallery"
                    } else {
                        "← Back to Nitro"
                    }),
            ))
            // The glass trial's switch, in development builds only.
            .when(cfg!(debug_assertions), |d| {
                d.child(
                    div()
                        .id("win-ctrl-glass")
                        .cursor_pointer()
                        .h(px(24.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .text_size(px(11.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(rgb(if components::glass_ui() { 0xffffff } else { 0xa1a1aa }))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| this.set_glass(!components::glass_ui(), cx)))
                        .child(if components::glass_ui() { "Glass on" } else { "Glass off" }),
                )
            })
            // Media Sidebar Toggle (right next to Settings, when signed in & in Chat view)
            .when(signed_in && in_chat, |d| {
                d.child(
                    div()
                        .id("win-ctrl-sidebar")
                        .cursor_pointer()
                        .w(px(46.))
                        .h(px(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::transparent_black())
                        .hover(|s| s.bg(rgba(0xffffff1a)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.media_sidebar_open = !this.media_sidebar_open;
                            cx.notify();
                        }))
                        .child(
                            svg()
                                .path("SidebarRight")
                                .size(px(14.))
                                .text_color(rgb(if sidebar_open {
                                    0xffffff
                                } else {
                                    0xc4c4cc
                                })),
                        ),
                )
            })
            // Settings Toggle (next to Windows controls when signed in)
            .when(signed_in && self.view_mode == 0, |d| {
                d.child(
                    div()
                        .id("win-ctrl-settings")
                        .cursor_pointer()
                        .w(px(46.))
                        .h(px(32.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::transparent_black())
                        .hover(|s| s.bg(rgba(0xffffff1a)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_dropdown = None;
                            if this.shell_view == ShellView::Settings {
                                this.shell_view = ShellView::Chat;
                            } else {
                                this.shell_view = ShellView::Settings;
                            }
                            cx.notify();
                        }))
                        .child(
                            svg()
                                .path("Settings")
                                .size(px(14.))
                                .text_color(rgb(if in_settings {
                                    0xffffff
                                } else {
                                    0xc4c4cc
                                })),
                        ),
                )
            })
            // Native Windows Minimize (─)
            .child(
                div()
                    .id("win-ctrl-minimize")
                    .cursor_pointer()
                    .w(px(46.))
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::transparent_black())
                    .hover(|s| s.bg(rgba(0xffffff1a)))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(|_, window, _| window.minimize_window())
                    .child(
                        svg()
                            .path("WindowMinimize")
                            .size(px(12.))
                            .text_color(rgb(0xd8d8df)),
                    ),
            )
            // Native Windows Maximize / Restore (□)
            .child(
                div()
                    .id("win-ctrl-maximize")
                    .cursor_pointer()
                    .w(px(46.))
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::transparent_black())
                    .hover(|s| s.bg(rgba(0xffffff1a)))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(|_, window, _| window.zoom_window())
                    .child(
                        svg()
                            .path("WindowMaximize")
                            .size(px(12.))
                            .text_color(rgb(0xd8d8df)),
                    ),
            )
            // Native Windows Close (✕) with #e81123 red hover
            .child(
                div()
                    .id("win-ctrl-close")
                    .cursor_pointer()
                    .w(px(46.))
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::transparent_black())
                    .hover(|s| s.bg(rgb(0xe81123)))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(|_, _, cx| cx.quit())
                    .child(
                        svg()
                            .path("WindowClose")
                            .size(px(12.))
                            .text_color(rgb(0xf2f2f3)),
                    ),
            )
    }

    #[inline(never)]
    pub fn render_chat_shell(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        if self.state.auth_status == AuthStatus::SignedOut {
            return self
                .render_auth_screen(window, cx)
                .child(self.render_window_controls(window, cx));
        }
        if self.state.auth_status == AuthStatus::Onboarding {
            return self
                .render_onboarding_screen(window, cx)
                .child(self.render_window_controls(window, cx));
        }

        let stage_bg = self.state.preferences.theme.stage_bg();
        let window_width = f32::from(window.viewport_size().width);
        let sidebar_shown = self.media_sidebar_open && self.shell_view == ShellView::Chat && window_width >= SIDEBAR_NEEDS;
        self.header_clear_of_buttons = !sidebar_shown && window_width < 1180.;
        let active_call = self.call_connected;

        let main_stage = match self.shell_view {
            ShellView::Settings => div()
                .flex_1()
                .min_w_0()
                .h_full()
                .relative()
                .child(self.render_settings_view(window, cx))
                .when(active_call, |d| d.child(call_glow()))
                .child(self.render_floating_header(cx)),
            ShellView::Friends => div()
                .flex_1()
                .min_w_0()
                .h_full()
                .relative()
                .child(self.render_dashboard_view(window, cx))
                .when(active_call, |d| d.child(call_glow()))
                .child(self.render_floating_header(cx)),
            ShellView::Chat => div()
                .flex_1()
                .min_w_0()
                .h_full()
                .relative()
                .flex()
                .flex_col()
                .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, _, cx| this.attach_dropped(paths.paths(), cx)))
                .child(self.render_message_history(cx))
                .child(
                    div()
                        .absolute()
                        .top(px(0.))
                        .left(px(0.))
                        .right(px(0.))
                        .h(px(96.))
                        .bg(gpui::linear_gradient(
                            180.,
                            gpui::linear_color_stop(rgb(stage_bg), 0.0),
                            gpui::linear_color_stop(rgba((stage_bg << 8) | 0x00), 1.0),
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(px(0.))
                        .left(px(0.))
                        .right(px(0.))
                        .h(px(108.))
                        .bg(gpui::linear_gradient(
                            0.,
                            gpui::linear_color_stop(rgb(stage_bg), 0.0),
                            gpui::linear_color_stop(rgba((stage_bg << 8) | 0x00), 1.0),
                        )),
                )
                // Messages scroll under the header, but can't be pressed through it. The header
                // itself is painted above this, and it's all still a window drag.
                .child(
                    div()
                        .id("chat-header-shield")
                        .absolute()
                        .top(px(0.))
                        .left(px(0.))
                        .right(px(0.))
                        .h(px(52.))
                        .block_mouse_except_scroll()
                        .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, _, cx| this.attach_dropped(paths.paths(), cx)))
                        .window_control_area(gpui::WindowControlArea::Drag),
                )
                .when(active_call, |d| d.child(call_glow()))
                .when_some(self.render_screen_panel(cx), |d, panel| d.child(panel))
                .child(self.render_floating_header(cx))
                .child(self.render_composer_dock(window, cx))
                // Shows that a dragged file can be dropped. It takes no presses of its own.
                .when(cx.has_active_drag(), |stage| {
                    stage.child(
                        div()
                            .absolute()
                            .inset(px(12.))
                            .rounded(px(16.))
                            .border_2()
                            .border_color(rgb(ACCENT))
                            .bg(rgba(0x7ab8ff14))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                material_surface(MATERIAL_DARK, 14., StrokeKind::Neutral)
                                    .px(px(16.))
                                    .py(px(10.))
                                    .text_size(px(14.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xf2f2f3))
                                    .child("Drop to attach an image or video"),
                            ),
                    )
                }),
        };

        div()
            .size_full()
            .relative()
            .bg(rgb(stage_bg))
            .overflow_hidden()
            .flex()
            // Transparent top drag region (`.window-drag-region`)
            .child(
                div()
                    .id("transparent-titlebar-drag-strip")
                    .absolute()
                    .top(px(0.))
                    .left(px(0.))
                    .right(px(260.))
                    .h(px(52.))
                    .bg(gpui::transparent_black())
                    .window_control_area(gpui::WindowControlArea::Drag),
            )
            // Main chat / dashboard / settings column
            .child(main_stage)
            // Right-hand Media Sidebar with completely transparent background
            .when(
                sidebar_shown,
                |d| d.child(self.render_media_sidebar(window, cx)),
            )
            // Content runs on under the buttons, so they get a fade to sit on.
            .child({
                let (width, soft) = if sidebar_shown { (344., 40.) } else { (300., 96.) };
                let colour = if self.call_connected { with_call_glow(stage_bg) } else { stage_bg };
                components::window_buttons_fade(colour, width, soft)
            })
            // Global top-right Windows caption controls + Settings + Media Sidebar toggle
            .child(self.render_window_controls(window, cx))
    }

    #[inline(never)]
    fn render_floating_header(&mut self, cx: &mut Context<Self>) -> Div {
        if !self.call_connected
            && matches!(
                self.open_dropdown,
                Some(HeaderDropdown::Screen | HeaderDropdown::Soundboard | HeaderDropdown::Microphone | HeaderDropdown::Output)
            )
        {
            self.open_dropdown = None;
        }
        if self.open_dropdown != self.last_dropdown {
            crate::audio::play(if self.open_dropdown.is_some() {
                crate::audio::UiSound::MenuOpen
            } else {
                crate::audio::UiSound::MenuClose
            });
            self.last_dropdown = self.open_dropdown;
        }
        let conv = self.active_conv().clone();
        let presence_color = match conv.presence.as_str() {
            "online" => 0x00d084,
            "away" => 0xf5c344,
            _ => 0x6b6b6b,
        };

        // 1. Partner Switcher Pill (3-Layer Glass Pill)
        let partner_pill = material_interactive(
            "chat-partner-switcher",
            MATERIAL_DARK,
            19.,
            StrokeKind::Neutral,
        )
        .h(px(40.))
        .pl(px(4.))
        .pr(px(13.))
        .flex()
        .items_center()
        .gap(px(9.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(|this, _, _, cx| {
            this.open_dropdown = if this.open_dropdown == Some(HeaderDropdown::Switcher) {
                None
            } else {
                Some(HeaderDropdown::Switcher)
            };
            cx.notify();
        }))
        .child(
            div()
                .relative()
                .size(px(30.))
                .child(avatar(
                    conv.initials.clone(),
                    conv.color,
                    30.,
                    conv.photo.clone(),
                ))
                .child(
                    div()
                        .absolute()
                        .bottom(px(-1.))
                        .right(px(-1.))
                        .size(px(10.))
                        .rounded_full()
                        .bg(rgb(presence_color))
                        .border_2()
                        .border_color(rgb(MATERIAL_DARK)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(gpui::FontWeight(conv.name_weight as f32))
                                .text_color(rgb(conv.name_color))
                                .child(components::name_text(
                                    conv.display_name.clone(),
                                    conv.name_effect,
                                    13.,
                                )),
                        )
                        .child(
                            svg()
                                .path("CallChevronDown")
                                .size(px(11.))
                                .text_color(rgb(0xa1a1aa)),
                        ),
                ),
        );

        // 2. Voice: the shared Call Pill in a call, otherwise Voice or Join with who's in.
        let my_id = self.state.profile.id.clone();
        let person_for = |user: &String| -> Option<(SharedString, u32, components::AvatarImage)> {
            if *user == my_id {
                Some((
                    self.state.profile.initials.clone().into(),
                    self.state.profile.avatar_color,
                    self.state.profile.photo.clone(),
                ))
            } else if *user == conv.partner_id {
                Some((conv.initials.clone().into(), conv.color, conv.photo.clone()))
            } else {
                None
            }
        };
        let in_room: Vec<_> = conv.voice_participants.iter().filter_map(person_for).collect();
        // A preview has no conversation; a real call only shows in its own chat.
        let call_here = self.call_connected
            && self
                .voice_conversation
                .as_ref()
                .is_none_or(|id| *id == conv.id);
        let voice_pill = if call_here {
            // Membership reads lag joining by a moment, so this device always shows you.
            let mut people = in_room;
            if !conv.voice_participants.contains(&my_id)
                && let Some(me) = person_for(&my_id)
            {
                people.push(me);
            }
            let seconds = conv
                .voice_started_at
                .as_deref()
                .and_then(crate::backend::seconds_since)
                .unwrap_or_else(|| self.call_started.elapsed().as_secs());
            components::call_pill(
                "chat-voice-pill",
                people,
                seconds,
                self.call_voice_hovered,
                cx.listener(|this, hovered: &bool, _, cx| {
                    this.call_voice_hovered = *hovered;
                    cx.notify();
                }),
                cx.listener(|this, _, _, cx| this.leave_voice(cx)),
            )
        } else {
            components::voice_button(
                "chat-voice-pill",
                if in_room.is_empty() { "Voice" } else { "Join" },
                in_room,
                cx.listener(|this, _, _, cx| this.join_voice(cx)),
            )
        };

        // 3. Dedicated Soundboard Pill (opens Stepped Volume Slider + Soundboard Pad Grid)
        let sb_open = self.open_dropdown == Some(HeaderDropdown::Soundboard);
        let soundboard_pill = components::material_quiet(
            "chat-soundboard-pill",
            "chat-soundboard-pill",
            if sb_open { 0x34343a } else { MATERIAL_DARK },
            19.,
            if sb_open {
                StrokeKind::AccentFocus
            } else {
                StrokeKind::Neutral
            },
        )
        .w(px(58.))
        .h(px(40.))
        .px(px(11.))
        .flex()
        .items_center()
        .justify_between()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(|this, _, _, cx| {
            this.load_soundboard(cx);
            this.open_dropdown = if this.open_dropdown == Some(HeaderDropdown::Soundboard) {
                None
            } else {
                Some(HeaderDropdown::Soundboard)
            };
            cx.notify();
        }))
        .child(
            svg()
                .path("CallAudioLines")
                .size(px(17.))
                .text_color(rgb(if self.playing_sound_id.is_some() {
                    0x00d084
                } else {
                    0xf2f2f3
                })),
        )
        .child(
            svg()
                .path("CallChevronDown")
                .size(px(12.))
                .text_color(rgb(0xc4c4cc)),
        );

        // 4. Screen Share Toggle Pill
        let sharing = self.voice_state.screen.share != crate::screen_share::Stage::Idle;
        let stream_open = self.open_dropdown == Some(HeaderDropdown::Screen);
        let stream_bg = if !sharing {
            MATERIAL_DARK
        } else if self.call_stream_hovered {
            MATERIAL_RED
        } else {
            MATERIAL_GREEN
        };
        let stream_pill = material_interactive(
            "chat-stream-pill",
            stream_bg,
            19.,
            if sharing {
                StrokeKind::Colored
            } else if stream_open {
                StrokeKind::AccentFocus
            } else {
                StrokeKind::Neutral
            },
        )
        .size(px(38.))
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_hover(cx.listener(|this, hovered, _, cx| {
            this.call_stream_hovered = *hovered;
            cx.notify();
        }))
        .on_click(cx.listener(|this, _, _, cx| {
            if this.voice_state.screen.share != crate::screen_share::Stage::Idle {
                this.voice.control(crate::voice::Control::ScreenStop);
            } else if this.open_dropdown == Some(HeaderDropdown::Screen) {
                this.open_dropdown = None;
            } else {
                this.voice.control(crate::voice::Control::ScreenSources);
                this.open_dropdown = Some(HeaderDropdown::Screen);
            }
            cx.notify();
        }))
        .child(
            svg()
                .path(if sharing && self.call_stream_hovered {
                    "CallScreenShareOff"
                } else {
                    "CallScreenShare"
                })
                .size(px(17.))
                .text_color(rgb(0xf5fff9)),
        );

        // 5. Split Microphone Pill (Left = Mute toggle, Right Chevron = Input Device + Input Volume Dropdown)
        let mic_open = self.open_dropdown == Some(HeaderDropdown::Microphone);
        let mic_pill = components::split_toggle(
            "chat-call-control",
            0,
            if self.call_muted { "CallMicOff" } else { "CallMic" },
            if self.call_muted { MATERIAL_RED } else if mic_open { 0x34343a } else { MATERIAL_DARK },
            if self.call_muted { StrokeKind::Colored } else if mic_open { StrokeKind::AccentFocus } else { StrokeKind::Neutral },
            0xffffff,
            cx.listener(|this, _, _, cx| this.toggle_mute(cx)),
            cx.listener(|this, _, _, cx| {
                    if this.open_dropdown == Some(HeaderDropdown::Microphone) {
                        this.open_dropdown = None;
                    } else {
                        this.refresh_audio_devices(cx);
                        this.open_dropdown = Some(HeaderDropdown::Microphone);
                    }
                    cx.notify();
            }),
        );

        // 6. Split Deafen / Output Pill (Left = Deafen toggle, Right Chevron = Output Device + Partner/Output Volume Dropdown)
        let out_open = self.open_dropdown == Some(HeaderDropdown::Output);
        let deafen_pill = components::split_toggle(
            "chat-call-control",
            1,
            if self.call_deafened { "CallHeadphoneOff" } else { "CallHeadphones" },
            if self.call_deafened { MATERIAL_RED } else if out_open { 0x34343a } else { MATERIAL_DARK },
            if self.call_deafened { StrokeKind::Colored } else if out_open { StrokeKind::AccentFocus } else { StrokeKind::Neutral },
            0xffffff,
            cx.listener(|this, _, _, cx| this.toggle_deafen(cx)),
            cx.listener(|this, _, _, cx| {
                    if this.open_dropdown == Some(HeaderDropdown::Output) {
                        this.open_dropdown = None;
                    } else {
                        this.refresh_audio_devices(cx);
                        this.open_dropdown = Some(HeaderDropdown::Output);
                    }
                    cx.notify();
            }),
        );

        let open_popover = match self.open_dropdown {
            Some(HeaderDropdown::Switcher) => Some(self.render_switcher_dropdown(cx)),
            Some(HeaderDropdown::Soundboard) => Some(self.render_soundboard_dropdown(cx)),
            Some(HeaderDropdown::Microphone) => Some(self.render_microphone_dropdown(cx)),
            Some(HeaderDropdown::Output) => Some(self.render_output_dropdown(cx)),
            Some(HeaderDropdown::Screen) => Some(self.render_screen_dropdown(cx)),
            None => None,
        };

        // Transparent header container (`bg(gpui::transparent_black())`)
        div()
            .absolute()
            .top(px(16.))
            .left(px(0.))
            .right(px(if self.header_clear_of_buttons { 240. } else { 0. }))
            .bg(gpui::transparent_black())
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .bg(gpui::transparent_black())
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(partner_pill)
                    .child(voice_pill)
                    .when(call_here, |row| {
                        row.child(soundboard_pill)
                            .child(stream_pill)
                            .child(mic_pill)
                            .child(deafen_pill)
                    }),
            )
            .when_some(open_popover, |d, pop| d.child(pop))
    }

    #[inline(never)]
    fn render_switcher_dropdown(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut dm_list = div().flex().flex_col().gap(px(4.));
        for (idx, conv) in self.state.conversations.iter().enumerate() {
            let selected = self.state.active_partner == idx && self.shell_view == ShellView::Chat;
            let dot_col = match conv.presence.as_str() {
                "online" => 0x00d084,
                "away" => 0xf5c344,
                _ => 0x6b6b6b,
            };
            dm_list = dm_list.child(
                material_interactive(
                    ("dm-switch-item", idx),
                    if selected { 0x323238 } else { MATERIAL_SUBTLE },
                    11.,
                    StrokeKind::Neutral,
                )
                .px(px(9.))
                .py(px(7.))
                .flex()
                .items_center()
                .justify_between()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.switch_partner(idx, window, cx);
                    this.shell_view = ShellView::Chat;
                    this.open_dropdown = None;
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(9.))
                        .child(
                            div()
                                .relative()
                                .size(px(28.))
                                .child(avatar(
                                    conv.initials.clone(),
                                    conv.color,
                                    28.,
                                    conv.photo.clone(),
                                ))
                                .child(
                                    div()
                                        .absolute()
                                        .right(px(-1.))
                                        .bottom(px(-1.))
                                        .size(px(9.))
                                        .rounded_full()
                                        .bg(rgb(dot_col))
                                        .border_2()
                                        .border_color(rgb(MATERIAL_PANEL)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(0xf2f2f3))
                                        .child(components::name_text(conv.display_name.clone(), conv.name_effect, 13.)),
                                ),
                        ),
                )
                .when(conv.unread_count > 0, |d| {
                    d.child(badge(
                        format!("{}", conv.unread_count),
                        ACCENT,
                        0xffffff,
                    ))
                }),
            );
        }

        material_surface(MATERIAL_PANEL, 16., StrokeKind::Neutral)
            .id("switcher-dropdown-panel")
            .w(px(252.))
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.5))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x8e8e98))
                            .child("DIRECT MESSAGES"),
                    )
                    .child(badge(
                        format!("{}", self.state.conversations.len()),
                        0x2a2a2e,
                        0xd4d4d8,
                    )),
            )
            .child(dm_list)
            .child(div().h(px(1.)).bg(rgba(0xffffff14)))
            .child(
                material_interactive(
                    "switcher-open-dashboard",
                    MATERIAL_DARK,
                    11.,
                    StrokeKind::Neutral,
                )
                .h(px(34.))
                .px(px(10.))
                .flex()
                .items_center()
                .justify_between()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.shell_view = ShellView::Friends;
                    this.open_dropdown = None;
                    cx.notify();
                }))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0xf2f2f3))
                        .child("Dashboard & Friends"),
                )
                .when(!self.state.friend_requests.is_empty(), |d| {
                    d.child(badge(
                        format!("{}", self.state.friend_requests.len()),
                        ACCENT,
                        0xffffff,
                    ))
                }),
            )
    }

    #[inline(never)]
    fn render_soundboard_dropdown(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let sb_vol = self.state.preferences.soundboard_volume;
        let stepped_slider = self.slider_for(2, sb_vol, 100, 5, 408., cx);

        // Filter pills: All | Pinned ★ | Mine | Partner
        let mut filter_bar = div().flex().items_center().gap(px(6.));
        for (f_idx, f_label) in ["All", "Pinned ★", "Mine", "Partner"].into_iter().enumerate() {
            let active = self.soundboard_filter == f_idx;
            filter_bar = filter_bar.child(
                material_interactive(
                    ("sb-filter", f_idx),
                    if active { ACCENT } else { MATERIAL_DARK },
                    10.,
                    if active {
                        StrokeKind::Colored
                    } else {
                        StrokeKind::Neutral
                    },
                )
                .h(px(26.))
                .px(px(10.))
                .flex()
                .items_center()
                .text_size(px(11.5))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(rgb(if active { 0xffffff } else { 0xc4c4cc }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.soundboard_filter = f_idx;
                    cx.notify();
                }))
                .child(f_label),
            );
        }

        // Order clips with pinned first, filtered by category
        let mut clips = self.soundboard_clips.clone();
        clips.sort_by_key(|c| !self.state.preferences.pinned_sound_ids.contains(&c.id));
        let filtered: Vec<_> = clips
            .into_iter()
            .filter(|c| match self.soundboard_filter {
                1 => self.state.preferences.pinned_sound_ids.contains(&c.id),
                2 => c.is_mine,
                3 => !c.is_mine,
                _ => true,
            })
            .collect();

        let mut grid = div().flex().flex_col().gap(px(7.));
        for pair in filtered.chunks(2) {
            let mut row = div().flex().items_center().gap(px(7.));
            for clip in pair {
                let clip_id = clip.id.clone();
                let clip_id_pin = clip.id.clone();
                let clip_name = clip.name.clone();
                let playing = self.playing_sound_id.as_deref() == Some(clip.id.as_str());
                let pinned = self
                    .state
                    .preferences
                    .pinned_sound_ids
                    .contains(&clip.id);

                row = row.child(
                    material_surface(MATERIAL_DARK, 13., StrokeKind::Neutral)
                    .when(playing, |card| {
                        let progress = self
                            .playing_since
                            .map(|started| {
                                started.elapsed().as_millis() as f32 / self.playing_ms.max(1) as f32
                            })
                            .unwrap_or(0.)
                            .clamp(0., 1.);
                        // The main app's simple progress bar behind the card's contents.
                        card.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .bottom_0()
                                .w(px(200.5 * progress))
                                .rounded(px(12.))
                                .bg(rgba(0xffffff26)),
                        )
                    })
                    .w(px(200.5))
                    .flex_shrink_0()
                    .h(px(50.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(6.))
                    .child(
                        div()
                            .id(SharedString::from(format!("sb-play-{}", clip.id)))
                            .cursor_pointer()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.play_clip_in_call(clip_id.clone(), cx);
                            }))
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .size(px(28.))
                                    .rounded(px(8.))
                                    .bg(rgba(if playing { 0xffffff26 } else { 0xffffff12 }))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        svg()
                                            .path("Play")
                                            .size(px(12.))
                                            .text_color(rgb(0xf2f2f3)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xf5f5f7))
                                            .truncate()
                                            .child(clip_name),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(rgb(0x9e9ea8))
                                            .child(clip.duration_label.clone()),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap(px(4.))
                            .child(avatar(
                                clip.owner_initials.clone(),
                                clip.owner_color,
                                20.,
                                clip.owner_art.clone(),
                            ))
                            .child(
                                div()
                                    .id(SharedString::from(format!("sb-pin-{}", clip.id)))
                                    .cursor_pointer()
                                    .size(px(24.))
                                    .rounded(px(6.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .hover(|s| s.bg(rgba(0xffffff14)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(pos) = this
                                            .state
                                            .preferences
                                            .pinned_sound_ids
                                            .iter()
                                            .position(|x| x == &clip_id_pin)
                                        {
                                            this.state.preferences.pinned_sound_ids.remove(pos);
                                        } else {
                                            this.state
                                                .preferences
                                                .pinned_sound_ids
                                                .push(clip_id_pin.clone());
                                        }
                                        this.persist();
                                        cx.notify();
                                    }))
                                    .child(
                                        svg()
                                            .path("Star")
                                            .size(px(13.))
                                            .text_color(rgb(if pinned {
                                                0xf5c84b
                                            } else {
                                                0x71717a
                                            })),
                                    ),
                            ),
                    ),
                );
            }
            if pair.len() == 1 {
                row = row.child(div().flex_1());
            }
            grid = grid.child(row);
        }

        material_surface(MATERIAL_PANEL, 16., StrokeKind::Neutral)
            .id("soundboard-dropdown-panel")
            .w(px(432.))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .child(
                                svg()
                                    .path("CallAudioLines")
                                    .size(px(14.))
                                    .text_color(rgb(0x00d084)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xf2f2f3))
                                    .child("Soundboard"),
                            ),
                    )
                    .child(filter_bar),
            )
            // Stepped Soundboard Volume Slider
            .child(components::slider_field(
                "Soundboard volume",
                format!("{sb_vol}%"),
                408.,
                stepped_slider,
            ))
            .map(|panel| match &self.soundboard_status {
                crate::SoundboardStatus::Loading | crate::SoundboardStatus::NotLoaded => {
                    panel.child(label("Loading sounds…"))
                }
                crate::SoundboardStatus::Failed(message) => panel.child(label(message.clone())),
                crate::SoundboardStatus::Loaded if self.soundboard_clips.is_empty() => {
                    panel.child(label("No sounds yet"))
                }
                crate::SoundboardStatus::Loaded => panel.child(
                    div()
                        .id("soundboard-clips")
                        .max_h(px(344.))
                        .overflow_y_scroll()
                        .child(grid),
                ),
            })
    }

    #[inline(never)]
    fn render_microphone_dropdown(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let in_vol = self.state.preferences.input_volume;
        let in_slider = self.slider_for(3, in_vol, 100, 1, 260., cx);

        let mut dev_list = div().flex().flex_col().gap(px(3.));
        for (idx, dev) in self.audio_devices.inputs.iter().enumerate() {
            let selected = self.state.preferences.input_device_id == dev.id;
            let dev_id = dev.id.clone();
            dev_list = dev_list.child(
                material_interactive(
                    ("mic-dev-item", idx),
                    if selected { 0x323238 } else { MATERIAL_SUBTLE },
                    10.,
                    if selected {
                        StrokeKind::AccentFocus
                    } else {
                        StrokeKind::Neutral
                    },
                )
                .px(px(9.))
                .py(px(7.))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.state.preferences.input_device_id = dev_id.clone();
                                this.send_voice_routes();
                    this.persist();
                    cx.notify();
                }))
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(if selected {
                            gpui::FontWeight::SEMIBOLD
                        } else {
                            gpui::FontWeight::NORMAL
                        })
                        .text_color(rgb(if selected { 0xffffff } else { 0xd4d4d8 }))
                        .child(dev.name.clone()),
                )
                .when(selected, |d| {
                    d.child(
                        div()
                            .size(px(8.))
                            .rounded_full()
                            .bg(rgb(0x00d084)),
                    )
                }),
            );
        }

        material_surface(MATERIAL_PANEL, 16., StrokeKind::Neutral)
            .id("microphone-dropdown-panel")
            .w(px(284.))
            .p(px(11.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(components::slider_field(
                "Input volume",
                format!("{in_vol}%"),
                260.,
                in_slider,
            ))
            .child(
                div()
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.5))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x8e8e98))
                            .child("INPUT DEVICE"),
                    )
                    .child(
                        div()
                            .id("rescan-mic-devices")
                            .cursor_pointer()
                            .text_size(px(10.5))
                            .text_color(rgb(crate::state::palette().accent_soft))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_audio_devices(cx);
                                cx.notify();
                            }))
                            .child("Refresh"),
                    ),
            )
            .child(dev_list)
    }

    #[inline(never)]
    /// The share picker, in the same style as the device lists.
    fn render_screen_dropdown(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut list = div().flex().flex_col().gap(px(3.));
        for (index, source) in self.voice_state.screen.sources.iter().enumerate() {
            let chosen = source.clone();
            list = list.child(
                material_interactive(("screen-source", index), MATERIAL_SUBTLE, 10., StrokeKind::Neutral)
                    .px(px(9.))
                    .py(px(7.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.start_screen_share(
                            chosen.id.clone(),
                            chosen.kind.clone(),
                            chosen.title.clone(),
                            cx,
                        );
                    }))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(0xd4d4d8))
                            .truncate()
                            .child(source.title.clone()),
                    ),
            );
        }
        material_surface(MATERIAL_PANEL, 16., StrokeKind::Neutral)
            .id("screen-dropdown-panel")
            .w(px(292.))
            .p(px(11.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(label("Share your screen"))
            .child(
                div()
                    .id("screen-source-list")
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .child(list),
            )
            .child(label("One viewer at a time. Stops after 10 minutes."))
    }

    /// Who's sharing, and the picture while watching. Pinned, because the chat scrolls.
    fn render_screen_panel(&mut self, cx: &mut Context<Self>) -> Option<Div> {
        use crate::screen_share::Stage;
        fn left(ends: Option<std::time::Instant>) -> String {
            let seconds = ends.map_or(0, |ends| ends.saturating_duration_since(std::time::Instant::now()).as_secs());
            format!("{}:{:02} left", seconds / 60, seconds % 60)
        }
        let screen = &self.voice_state.screen;
        let partner = self.active_conv().display_name.clone();
        let (title, detail, button_id, button_label, button_color): (String, String, &'static str, &'static str, u32) =
            if screen.watch != Stage::Idle {
                (
                    format!("Watching {partner}"),
                    if screen.watch == Stage::Live { left(screen.watch_ends) } else { "Connecting…".into() },
                    "screen-stop-watching",
                    "Stop watching",
                    MATERIAL_DARK,
                )
            } else if screen.share != Stage::Idle {
                (
                    format!("Sharing {}", screen.share_title),
                    if screen.share == Stage::Live { left(screen.share_ends) } else { "Starting…".into() },
                    "screen-stop-sharing",
                    "Stop sharing",
                    MATERIAL_RED,
                )
            } else if screen.partner_sharing {
                (
                    format!("{partner} is sharing their screen"),
                    "Viewing stops after 10 minutes.".into(),
                    "screen-watch",
                    "Watch",
                    ACCENT,
                )
            } else {
                return None;
            };
        let watching = screen.watch != Stage::Idle;
        let sharing_row = screen.watch == Stage::Idle && screen.share != Stage::Idle;
        let bar = material_surface(MATERIAL_PANEL, 16., StrokeKind::Neutral)
            .w_full()
            .p(px(12.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0xf5f5f7))
                            .truncate()
                            .child(title),
                    )
                    .child(label(detail)),
            )
            .child(
                material_interactive(button_id, button_color, 14., StrokeKind::Colored)
                    .flex_shrink_0()
                    .h(px(30.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .text_size(px(12.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(0xffffff))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if watching {
                            this.stop_watching_screen(cx);
                        } else if sharing_row {
                            this.voice.control(crate::voice::Control::ScreenStop);
                        } else {
                            this.start_watching_screen(cx);
                        }
                    }))
                    .child(button_label),
            );
        let frame = if watching { self.screen_frame.clone() } else { self.share_frame.clone() };
        let picture = frame.map(|frame| {
            div().w_full().h(px(380.)).child(components::video_frame(frame, 12.))
        });
        Some(
            div()
                .absolute()
                .top(px(72.))
                .left(px(28.))
                .right(px(28.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(bar)
                .when_some(picture, |column, picture| column.child(picture)),
        )
    }

    fn render_output_dropdown(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let partner_vol = self.state.preferences.partner_volume;
        let out_vol = self.state.preferences.output_volume;
        let conv = self.active_conv();
        let partner_slider = self.slider_for(4, partner_vol, 200, 5, 268., cx);
        let out_slider = self.slider_for(5, out_vol, 100, 1, 268., cx);

        let mut dev_list = div().flex().flex_col().gap(px(3.));
        for (idx, dev) in self.audio_devices.outputs.iter().enumerate() {
            let selected = self.state.preferences.output_device_id == dev.id;
            let dev_id = dev.id.clone();
            dev_list = dev_list.child(
                material_interactive(
                    ("out-dev-item", idx),
                    if selected { 0x323238 } else { MATERIAL_SUBTLE },
                    10.,
                    if selected {
                        StrokeKind::AccentFocus
                    } else {
                        StrokeKind::Neutral
                    },
                )
                .px(px(9.))
                .py(px(7.))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.state.preferences.output_device_id = dev_id.clone();
                                this.send_voice_routes();
                    this.persist();
                    cx.notify();
                }))
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(if selected {
                            gpui::FontWeight::SEMIBOLD
                        } else {
                            gpui::FontWeight::NORMAL
                        })
                        .text_color(rgb(if selected { 0xffffff } else { 0xd4d4d8 }))
                        .child(dev.name.clone()),
                )
                .when(selected, |d| {
                    d.child(
                        div()
                            .size(px(8.))
                            .rounded_full()
                            .bg(rgb(0x00d084)),
                    )
                }),
            );
        }

        material_surface(MATERIAL_PANEL, 16., StrokeKind::Neutral)
            .id("output-dropdown-panel")
            .w(px(292.))
            .p(px(11.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // Partner Voice Volume / Boost Slider (0% - 200%)
            .child(components::slider_field(
                format!("{} volume", conv.display_name),
                format!("{partner_vol}%"),
                268.,
                partner_slider,
            ))
            // Master Output Volume Slider (0% - 100%)
            .child(components::slider_field(
                "Output volume",
                format!("{out_vol}%"),
                268.,
                out_slider,
            ))
            .child(
                div()
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(10.5))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0x8e8e98))
                            .child("OUTPUT DEVICE"),
                    )
                    .child(
                        div()
                            .id("rescan-out-devices")
                            .cursor_pointer()
                            .text_size(px(10.5))
                            .text_color(rgb(crate::state::palette().accent_soft))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_audio_devices(cx);
                                cx.notify();
                            }))
                            .child("Refresh"),
                    ),
            )
            .child(dev_list)
    }

    #[inline(never)]
    fn render_message_history(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut stream = div()
            .w_full()
            .pt(px(88.))
            .pb(px(158.))
            .px(px(28.))
            .flex()
            .flex_col()
            .gap(px(14.))
            // Not on the history itself: a focusable element takes focus from the composer on
            // any press, and marks the press handled, which stops a window drag starting there.
            .child(
                div()
                    .absolute()
                    .size_0()
                    .track_focus(&self.history_focus)
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                        let keys = &event.keystroke;
                        if keys.modifiers.control && !keys.modifiers.alt && keys.key == "c" {
                            if let Some(words) = this.text_selection.borrow().as_ref().and_then(|selection| selection.words()) {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(words.to_string()));
                            }
                            return;
                        }
                        // Anything else means they've moved on from the selection. Typing goes
                        // back to the composer, so a selection never swallows what's typed.
                        let typing = keys.key_char.is_some() && !keys.modifiers.control && !keys.modifiers.alt;
                        if typing || keys.key == "escape" {
                            *this.text_selection.borrow_mut() = None;
                            this.chat_input.update(cx, |input, cx| input.focus(window, cx));
                            cx.notify();
                        }
                    })),
            );

        let messages = self.active_conv().messages.clone();
        if self.state.conversations.is_empty() {
            stream = stream.child(alert(
                "HeroInfoIcon",
                ACCENT,
                "No conversations yet",
                "Add a friend from Friends & requests to start chatting.",
                None,
            ));
        } else if !self.active_conv().loaded {
            stream = stream.child(label("Loading messages…"));
        } else if messages.is_empty() {
            stream = stream.child(alert(
                "HeroInfoIcon",
                ACCENT,
                "No messages yet",
                format!("Say hello to {}.", self.active_conv().display_name),
                None,
            ));
        }
        for (start, length) in crate::backend::burst_groups(&messages) {
            let burst = &messages[start..start + length];
            if let Some(marker) = &burst[0].marker_before {
                stream = stream.child(components::system_marker(marker.clone()));
            }
            if burst[0].system {
                stream = stream.child(components::system_marker(format!(
                    "{} · {}",
                    burst[0].text, burst[0].time
                )));
                continue;
            }
            stream = stream.child(self.render_message_card(start, burst, cx));
        }

        div()
            .id("chat-message-history")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .track_scroll(&self.chat_scroll)
            .child({
                let handle = self.chat_scroll.clone();
                let anchor = self.older_anchor.clone();
                let wants_older = self.wants_older.clone();
                let can_load = self.active_conv().has_more
                    && !self.loading_older
                    && self.history_settled_at.elapsed() > Duration::from_millis(800);
                gpui::canvas(
                    move |_, window, _| {
                        let max = f32::from(handle.max_offset().height);
                        let offset = handle.offset();
                        if let Some((old_max, old_y)) = anchor.get() {
                            if max != old_max {
                                handle.set_offset(gpui::point(offset.x, px(old_y - (max - old_max))));
                                anchor.set(None);
                                window.request_animation_frame();
                            }
                        } else if can_load && max > 0. && f32::from(offset.y) > -300. {
                            wants_older.set(true);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size(px(0.))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.open_dropdown.is_some() {
                        this.open_dropdown = None;
                        cx.notify();
                    }
                }),
            )
            .child(stream)
    }

    /// Condenses a burst into the shared Media Grid, with identical files as one counted tile.
    fn render_burst_card(&self, burst: &[ChatMessage], cx: &mut Context<Self>) -> Div {
        let mut distinct: Vec<(&ChatMessage, usize)> = vec![];
        for member in burst {
            let key = crate::backend::media_key(member);
            match distinct
                .iter_mut()
                .find(|(seen, _)| key.is_some() && crate::backend::media_key(seen) == key)
            {
                Some((_, count)) => *count += 1,
                None => distinct.push((member, 1)),
            }
        }
        let count = distinct.len();
        let tiles = distinct
            .iter()
            .enumerate()
            .map(|(index, (member, repeats))| components::MediaTile {
                content: if let ChatAttachment::VideoClip { duration, .. } = &member.attachment {
                    div().size_full().child(self.render_video_tile(
                        format!("video-{}", member.id).into(),
                        member.media_path.clone(),
                        duration.clone(),
                        components::media_tile_width(count.min(4), index),
                        components::media_tile_height(count.min(4), index),
                        true,
                        cx,
                    ))
                } else {
                    self.render_media_image(
                        member.media_path.clone(),
                        components::media_tile_height(count.min(4), index),
                        gpui::ObjectFit::Cover,
                    )
                },
                count: *repeats,
            })
            .collect();
        div()
            .mt(px(8.))
            .child(components::media_grid(tiles))
    }

    #[inline(never)]
    fn render_message_card(
        &self,
        msg_idx: usize,
        burst: &[ChatMessage],
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let msg = burst[0].clone();
        let sender_for_reply = msg.sender.clone();
        let excerpt_for_reply = if msg.text.is_empty() {
            "Attachment".to_string()
        } else {
            msg.text.clone()
        };
        let message_db_id = msg.db_id.clone();
        let show_previews = self.state.preferences.show_media_previews;

        let mut reactions_row = div().flex().flex_wrap().items_center().gap(px(6.)).mt(px(6.));
        // A burst shows the reactions of every message in it, each still tied to its message.
        let reaction_chips = burst.iter().enumerate().flat_map(|(offset, member)| {
            member.reactions.iter().enumerate().map(move |(r_idx, r)| (offset, r_idx, r))
        });
        for (offset, r_idx, r) in reaction_chips {
            let active = r.mine;
            let chip_emoji = r.emoji.clone();
            let chip_target = msg_idx + offset;
            let chip = material_interactive(
                ("msg-react", chip_target * 10 + r_idx),
                if active { 0x163824 } else { MATERIAL_DARK },
                999.,
                if active {
                    StrokeKind::Colored
                } else {
                    StrokeKind::Neutral
                },
            )
            .px(px(10.))
            .py(px(3.5))
            .flex()
            .items_center()
            .gap(px(5.))
            .text_size(px(12.5))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(rgb(if active { 0x9dffb8 } else { 0xe4e4e7 }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_reaction(chip_target, &chip_emoji, cx);
            }))
            .child(r.emoji.clone())
            .child(format!("{}", r.count));
            reactions_row = reactions_row.child(chip);
        }

        // The same actions as main Nitro's tray: react, reply, and for your own, edit and delete.
        let picker_open = self.reaction_picker.as_deref() == Some(message_db_id.as_str());
        let can_change = msg.is_self && !msg.deleted && !msg.pending;
        let can_edit = can_change && burst.len() == 1;
        let burst_ids: Vec<String> = burst.iter().map(|member| member.db_id.clone()).collect();
        fn action(id: (&'static str, usize), icon: &'static str) -> Stateful<Div> {
            div()
                .id(id)
                .cursor_pointer()
                .size(px(26.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .hover(|s| s.bg(rgba(0xffffff18)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(svg().path(icon).size(px(13.)).text_color(rgb(0xe4e4e7)))
        }
        let picker_target = message_db_id.clone();
        let edit_target = message_db_id.clone();
        let edit_text = msg.text.clone();
        let copy_text = if msg.deleted { String::new() } else { msg.text.clone() };
        let highlighted = self.highlighted_message.as_ref().is_some_and(|id| burst.iter().any(|member| member.db_id == *id));
        let action_bar = material_surface(MATERIAL_DARK, 15., StrokeKind::Neutral)
            .relative()
            .h(px(30.))
            .p(px(2.))
            .flex()
            .items_center()
            .gap(px(2.))
            .opacity(if picker_open { 1. } else { 0. })
            .group_hover("msg-row", |s| s.opacity(1.))
            .child(
                action(("act-react", msg_idx), "SmilePlus").on_click(cx.listener(move |this, _, _, cx| {
                    this.reaction_picker = if this.reaction_picker.as_deref() == Some(picker_target.as_str()) {
                        None
                    } else {
                        Some(picker_target.clone())
                    };
                    cx.notify();
                })),
            )
            .child(
                action(("act-reply", msg_idx), "Reply").on_click(cx.listener(move |this, _, _, cx| {
                    this.chat_replying_to =
                        Some((sender_for_reply.clone(), excerpt_for_reply.clone()));
                    this.chat_reply_target = Some(message_db_id.clone());
                    cx.notify();
                })),
            )
            .when(!copy_text.is_empty(), |bar| {
                bar.child(action(("act-copy", msg_idx), "Copy").on_click(cx.listener(move |_, _, _, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_text.clone()));
                })))
            })
            .when(can_edit, |bar| {
                bar.child(
                    action(("act-edit", msg_idx), "Pencil").on_click(cx.listener(move |this, _, window, cx| {
                        this.start_editing(edit_target.clone(), edit_text.clone(), window, cx);
                    })),
                )
            })
            .when(can_change, |bar| {
                bar.child(
                    action(("act-delete", msg_idx), "Trash").on_click(cx.listener(move |this, _, _, cx| {
                        this.delete_messages(burst_ids.clone(), cx);
                    })),
                )
            })
            .when(picker_open, |bar| {
                let mut picker = material_surface(MATERIAL_DARK, 15., StrokeKind::Neutral)
                    .absolute()
                    .top(px(34.))
                    .right(px(0.))
                    .h(px(34.))
                    .p(px(3.))
                    .flex()
                    .items_center()
                    .gap(px(1.));
                // As in main Nitro. The heart needs its variation selector to render filled.
                for (index, emoji) in ["❤️", "😂", "👍", "👎", "💩", "🔥"].into_iter().enumerate() {
                    picker = picker.child(
                        div()
                            .id(("quick-reaction", msg_idx * 10 + index))
                            .cursor_pointer()
                            .size(px(28.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(15.))
                            .hover(|s| s.bg(rgba(0xffffff18)))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.reaction_picker = None;
                                this.toggle_reaction(msg_idx, emoji, cx);
                            }))
                            .child(emoji),
                    );
                }
                // Above the messages after this one, which would otherwise paint over it.
                bar.child(gpui::deferred(picker).with_priority(1))
            });

        let editing = self.editing_message.as_deref() == Some(msg.db_id.as_str());
        let inline_editor = editing.then(|| {
            div()
                .mt(px(2.))
                .max_w(px(720.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    material_surface(MATERIAL_DARK, 12., StrokeKind::AccentFocus)
                        .px(px(10.))
                        .py(px(4.))
                        .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                            match event.keystroke.key.as_str() {
                                "enter" if !event.keystroke.modifiers.shift => {
                                    cx.stop_propagation();
                                    this.save_edit(cx);
                                }
                                "escape" => {
                                    cx.stop_propagation();
                                    this.cancel_editing(cx);
                                }
                                _ => {}
                            }
                        }))
                        .child(Input::new(&self.edit_input).appearance(false)),
                )
                .child(label("esc to cancel · enter to save"))
        });

        let link_cards = if !show_previews || msg.deleted {
            None
        } else {
            self.render_link_cards(msg.id, &msg.text, cx)
        };
        let attachment_el = if !show_previews || msg.deleted {
            None
        } else if burst.len() > 1 {
            Some(self.render_burst_card(burst, cx))
        } else {
            match &msg.attachment {
                ChatAttachment::None => None,
                ChatAttachment::MediaGallery { .. } => {
                    let (width, height) = message_image_size(msg.media_size);
                    Some(
                        div()
                            .mt(px(6.))
                            .w(px(width))
                            .child(self.render_chat_image(msg.media_path.clone(), height)),
                    )
                }
                ChatAttachment::VideoClip { duration, .. } => {
                    let (width, height) = message_image_size(msg.media_size);
                    Some(
                        div().mt(px(6.)).w(px(width)).h(px(height)).child(self.render_video_tile(
                            format!("video-{}", msg.id).into(),
                            msg.media_path.clone(),
                            duration.clone(),
                            width,
                            height,
                            false,
                            cx,
                        )),
                    )
                }
            }
        };

        div()
            .id(("message-card", msg.id))
            .group("msg-row")
            .relative()
            .when(highlighted, |card| card.bg(rgba(0x7ab8ff1f)))
            .child({
                let (tops, ids) = (self.message_tops.clone(), burst.iter().map(|member| member.db_id.clone()).collect::<Vec<_>>());
                gpui::canvas(
                    move |bounds, _, _| {
                        let mut tops = tops.borrow_mut();
                        for id in &ids {
                            tops.insert(id.clone(), f32::from(bounds.top()));
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full()
            })
            .w_full()
            .px(px(14.))
            .py(px(10.))
            .rounded(px(14.))
            .hover(|s| s.bg(rgba(0xffffff07)))
            .flex()
            . flex_col()
            .gap(px(5.))
            .when_some(msg.reply_to.clone(), |d, (r_author, r_text)| {
                d.child(
                    div()
                        .pl(px(52.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(12.))
                        .text_color(rgb(0x9e9ea8))
                        .child(svg().path("Reply").size(px(12.)).text_color(rgb(0x8e8e98)))
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(0xd4d4d8))
                                .child(format!("@{r_author}")),
                        )
                        .child(r_text),
                )
            })
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(12.))
                            .flex_1()
                            .min_w_0()
                            .child(avatar(
                                msg.initials.clone(),
                                msg.avatar_color,
                                40.,
                                msg.photo.clone(),
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(rgb(msg.name_color))
                                                    .font_weight(gpui::FontWeight(msg.name_weight as f32))
                                                    .child(components::name_text(
                                                        if msg.is_self {
                                                            self.state.profile.display_name.clone()
                                                        } else {
                                                            msg.sender.clone()
                                                        },
                                                        msg.name_effect,
                                                        14.,
                                                    )),
                                            )
                                            .when_some(msg.role_badge.clone(), |d, (b_txt, b_col)| {
                                                d.child(badge(b_txt, b_col, 0xffffff))
                                            })
                                            .when(msg.pinned, |d| {
                                                d.child(badge("PINNED", 0x26262b, 0x9cc9ff))
                                            })
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(rgb(0x71717a))
                                                    .child(msg.time.clone()),
                                            ),
                                    )
                                    .when_some(inline_editor, |d, editor| d.child(editor))
                                    .when(!editing && (msg.deleted || !msg.text.is_empty()), |d| {
                                        d.child(
                                            div()
                                                .text_size(px(14.5))
                                                .line_height(px(21.))
                                                .text_color(rgb(if msg.deleted {
                                                    0x71717a
                                                } else {
                                                    0xe4e4e7
                                                }))
                                                .map(|text| {
                                                    if msg.deleted {
                                                        text.child("(deleted)")
                                                    } else {
                                                        text.child(message_text(
                                                            msg.id,
                                                            msg.text.clone(),
                                                            self.text_selection.clone(),
                                                            self.history_focus.clone(),
                                                        ))
                                                    }
                                                })
                                                .when(msg.edited && !msg.deleted, |text| {
                                                    text.child(
                                                        div()
                                                            .text_size(px(11.5))
                                                            .text_color(rgb(0x71717a))
                                                            .child("(edited)"),
                                                    )
                                                }),
                                        )
                                    })
                                    .when_some(attachment_el, |d, el| d.child(el))
                                    .when_some(link_cards, |d, cards| d.child(cards))
                                    .when(burst.iter().any(|member| !member.reactions.is_empty()), |d| d.child(reactions_row)),
                            ),
                    )
                    .child(action_bar),
            )
    }

    #[inline(never)]
    fn render_composer_dock(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        // Commands appear only once the message starts with a slash, narrowed as you type.
        let typed = self.chat_input.read(cx).value().to_string();
        let typed_command = typed.split_whitespace().next().unwrap_or_default().to_string();
        let typing_command = typed.starts_with('/') && !typed.contains(' ');
        // The partner comes first, as there's one line.
        let typing_row = if self.partner_typing_until.is_some() {
            let conv = self.active_conv();
            Some(components::typing_indicator(
                conv.display_name.clone(),
                conv.initials.clone(),
                conv.color,
                conv.photo.clone(),
            ))
        } else if self.self_typing_until.is_some() {
            let me = &self.state.profile;
            Some(components::typing_indicator(
                me.display_name.clone(),
                me.initials.clone(),
                me.avatar_color,
                me.photo.clone(),
            ))
        } else {
            None
        };
        let mut command_chips = div().flex().items_center().gap(px(6.));
        for (cmd_idx, cmd) in ["/shrug", "/ping"].into_iter().enumerate() {
            if !typing_command || !cmd.starts_with(typed_command.as_str()) {
                continue;
            }
            command_chips = command_chips.child(
                material_interactive(("cmd-chip", cmd_idx), MATERIAL_DARK, 999., StrokeKind::Neutral)
                    .h(px(26.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .text_size(px(11.5))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(crate::state::palette().accent_soft))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let input = this.chat_input.clone();
                        input.update(cx, |state, cx| {
                            state.set_value(format!("{cmd} "), window, cx);
                            state.focus(window, cx);
                        });
                        cx.notify();
                    }))
                    .child(cmd),
            );
        }

        // The attachment waiting to send, as in main Nitro.
        let pending_media_row = if self.pending_media.is_some() || self.media_busy.is_some() {
            let thumbnail = self.pending_media.as_ref().map(|media| media.thumbnail.clone());
            let compressing = self.media_progress.as_ref().filter(|progress| progress.compressing.load(std::sync::atomic::Ordering::Acquire));
            let title = compressing
                .map(|progress| format!("Compressing video… {}%", progress.percent.load(std::sync::atomic::Ordering::Acquire)))
                .or_else(|| self.media_busy.map(String::from))
                .or_else(|| self.pending_media.as_ref().map(|media| media.name.clone()));
            let cancellable = compressing.is_some();
            let detail = self.pending_media.as_ref().map(|media| {
                let size = media.bytes.len() as f32 / (1024. * 1024.);
                match media.seconds {
                    Some(seconds) => format!("Video · {}:{:02} · {size:.1} MB", seconds as u32 / 60, seconds as u32 % 60),
                    None => format!("Image · {size:.1} MB"),
                }
            });
            let uploading = self.media_busy.is_some();
            Some(
                material_surface(MATERIAL_DARK, 14., StrokeKind::Neutral)
                    .block_mouse_except_scroll()
                    .p(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .when_some(thumbnail, |row, frame| {
                        row.child(div().size(px(44.)).flex_shrink_0().child(components::video_frame(frame, 8.)))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xf2f2f3))
                                    .truncate()
                                    .child(title.unwrap_or_default()),
                            )
                            .when_some(detail, |column, detail| column.child(label(detail))),
                    )
                    .when(cancellable, |row| {
                        row.child(
                            div()
                                .id("cancel-compressing")
                                .cursor_pointer()
                                .size(px(24.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(rgba(0xffffff14)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.cancel_preparing_media();
                                    cx.notify();
                                }))
                                .child("×"),
                        )
                    })
                    .when(!uploading, |row| {
                        row.child(
                            div()
                                .id("remove-attachment")
                                .cursor_pointer()
                                .size(px(24.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(rgba(0xffffff14)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_pending_media(None);
                                    cx.notify();
                                }))
                                .child("×"),
                        )
                    }),
            )
        } else {
            None
        };

        let composer_pill = material_surface(MATERIAL_DARK, 23., StrokeKind::Neutral)
            .block_mouse_except_scroll()
            .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, _, cx| this.attach_dropped(paths.paths(), cx)))
            .flex_1()
            .h(px(46.))
            .pl(px(8.))
            .pr(px(6.))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .id("composer-attach-btn")
                    .cursor_pointer()
                    .size(px(32.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|s| s.bg(rgba(0xffffff14)))
                    .on_click(cx.listener(|this, _, _, cx| this.choose_attachment(cx)))
                    .child(svg().path("Paperclip").size(px(16.)).text_color(rgb(0xd4d4d8))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&self.chat_input).appearance(false)),
            )
            .child(
                material_interactive("composer-send-btn", ACCENT, 17., StrokeKind::Colored)
                    .h(px(34.))
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(12.5))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(0xffffff))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.send_chat_message(window, cx);
                    }))
                    .child(svg().path("Send").size(px(13.)).text_color(rgb(0xffffff)))
                    .child("Send"),
            );

        div()
            .id("chat-composer-dock")
            .absolute()
            .bottom(px(16.))
            .left(px(0.))
            .right(px(0.))
            .flex()
            .justify_center()
            .px(px(28.))
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, window, cx| {
                if e.keystroke.key == "enter" && !e.keystroke.modifiers.shift {
                    cx.stop_propagation();
                    this.send_chat_message(window, cx);
                }
            }))
            .child(
                div()
                    .w_full()
                    .max_w(px(600.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .when_some(
                        self.chat_replying_to.clone(),
                        |d, (reply_author, reply_excerpt)| {
                            d.child(
                                material_surface(MATERIAL_PANEL, 16., StrokeKind::Neutral)
                                    .block_mouse_except_scroll()
                                    .w_full()
                                    .px(px(12.))
                                    .py(px(8.))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .child(
                                                svg()
                                                    .path("Reply")
                                                    .size(px(14.))
                                                    .text_color(rgb(crate::state::palette().accent_soft)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(rgb(0xf2f2f3))
                                                    .child(format!("Replying to {reply_author}")),
                                            )
                                            .child(label(reply_excerpt)),
                                    )
                                    .child(
                                        div()
                                            .id("cancel-reply-btn")
                                            .cursor_pointer()
                                            .size(px(24.))
                                            .rounded_full()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .hover(|s| s.bg(rgba(0xffffff14)))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.chat_replying_to = None;
                                                this.chat_reply_target = None;
                                                cx.notify();
                                            }))
                                            .child("×"),
                                    ),
                            )
                        },
                    )
                    .when_some(pending_media_row, |column, row| column.child(row))
                    // Only while someone's typing, or while commands apply to what's typed.
                    .when_some(typing_row, |column, row| column.child(div().px(px(6.)).child(row)))
                    .when(typing_command, |column| {
                        column.child(
                            div()
                                .px(px(6.))
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap(px(8.))
                                .child(command_chips),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(composer_pill)
                            .when(self.history_scrolled_up, |row| {
                                row.child(
                                    material_interactive(
                                        "scroll-to-latest",
                                        MATERIAL_DARK,
                                        23.,
                                        StrokeKind::Neutral,
                                    )
                                    .size(px(46.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.chat_scroll.scroll_to_bottom();
                                        this.history_scrolled_up = false;
                                        cx.notify();
                                    }))
                                    .child(svg().path("ArrowDown").size(px(16.)).text_color(rgb(0xf2f2f3))),
                                )
                            }),
                    ),
            )
    }

    /// Right-hand Media Sidebar with a completely transparent background (`bg(gpui::transparent_black())`),
    /// no border, and `padding: 42px 20px 40px` clearing the 32px top-right Windows control strip.
    #[inline(never)]
    fn render_media_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        // A card and the gap below it.
        const MEDIA_CARD_STEP: f32 = 190.5;
        const MEDIA_LOAD_MARGIN: f32 = 500.;
        let conv = self.active_conv().clone();
        let total_count = conv.shared_media.len();

        // Segmented Tabs: All | Photos | Clips
        let tabs_row = components::tabs(
            "media-tab",
            vec![
                format!("All · {total_count}").into(),
                "Photos".into(),
                "Clips".into(),
            ],
            &self.media_tabs,
            90.6,
            window,
            std::rc::Rc::new(cx.listener(|this, index: &usize, _, cx| {
                this.media_tabs.select(*index);
                cx.notify();
            })),
        );

        // Reads like the chat: newest at the bottom, where it rests, with older ones up under the
        // tabs. Short lists sit at the bottom too.
        let mut media_cards = div().min_h_full().flex().flex_col().justify_end().gap(px(12.));
        let mut shown = 0;
        let mut top_id = None;
        for (m_idx, item) in conv.shared_media.iter().enumerate().rev() {
            if self.media_tabs.selected == 1 && item.kind == "clip" {
                continue;
            }
            if self.media_tabs.selected == 2 && item.kind != "clip" {
                continue;
            }
            shown += 1;
            top_id.get_or_insert_with(|| item.id.clone());
            let item_title = item.title.clone();
            let item_sender = item.sender.clone();
            let item_id = item.id.clone();
            let scroll_target = item.id.clone();

            media_cards = media_cards.child(
                material_surface(MATERIAL_PANEL, 12., StrokeKind::Neutral)
                    .id(("media-card", m_idx))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.scroll_to_message(scroll_target.clone(), cx)))
                    .block_mouse_except_scroll()
                    .w_full()
                    .p(px(1.25))
                    .h(px(178.5))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(
                        self.render_media_picture(item.media_path.clone(), 176., gpui::ObjectFit::Cover, false)
                            .rounded_t(px(10.75))
                            .rounded_b(px(10.75)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(1.25))
                            .right(px(1.25))
                            .bottom(px(1.25))
                            .rounded_b(px(10.75))
                            .bg(gpui::linear_gradient(180.,
                                gpui::linear_color_stop(rgba(0x00000000), 0.),
                                gpui::linear_color_stop(rgba(0x000000e6), 1.)))
                            .px(px(12.))
                            .pt(px(30.))
                            .pb(px(10.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xf2f2f3))
                                            .child(item.title.clone()),
                                    )
                                    .child(label(item.meta.clone())),
                            )
                            .child(
                                material_interactive(
                                    ("media-quote-btn", m_idx),
                                    MATERIAL_DARK,
                                    10.,
                                    StrokeKind::Neutral,
                                )
                                .size(px(26.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    // Replying isn't also a press on the card behind it.
                                    cx.stop_propagation();
                                    this.chat_replying_to =
                                        Some((item_sender.clone(), item_title.clone()));
                                    this.chat_reply_target = Some(item_id.clone());
                                    cx.notify();
                                }))
                                .child(
                                    svg().path("Reply").size(px(12.)).text_color(rgb(0xd4d4d8)),
                                ),
                            ),
                    ),
            );
        }

        if shown == 0 {
            let loading = self.sidebar_media.get(&conv.id).is_none_or(|pages| pages.loading);
            media_cards = media_cards.items_center().justify_center().child(label(if loading {
                "Loading media…"
            } else if self.media_tabs.selected == 1 {
                "No photos here yet"
            } else if self.media_tabs.selected == 2 {
                "No clips here yet"
            } else {
                "Photos and videos you share show up here"
            }));
        }

        // A different chat or tab starts at the newest. Something new, or a resize, only follows
        // if you were already there, so it never pulls you away from older media.
        let key = (conv.id.clone(), self.media_tabs.selected, shown, f32::from(window.viewport_size().height) as i32);
        if self.media_scroll_key.as_ref() != Some(&key) {
            let same_list = self.media_scroll_key.as_ref().is_some_and(|last| last.0 == key.0 && last.1 == key.1);
            let from_bottom = f32::from(self.media_scroll.max_offset().height) + f32::from(self.media_scroll.offset().y);
            let grew_by = self.media_scroll_key.as_ref().map_or(0, |last| shown.saturating_sub(last.2));
            if !same_list || from_bottom < 60. {
                self.media_scroll.scroll_to_bottom();
            } else if grew_by > 0 && self.media_top_id != top_id {
                // Older cards arrived above, so the ones being looked at stay where they are.
                let mut offset = self.media_scroll.offset();
                offset.y -= px(grew_by as f32 * MEDIA_CARD_STEP);
                self.media_scroll.set_offset(offset);
            }
            self.media_scroll_key = Some(key);
            self.media_top_id = top_id;
        } else if f32::from(self.media_scroll.offset().y) > -MEDIA_LOAD_MARGIN {
            self.load_media_page(cx);
        }
        // The first page shouldn't wait for the top of a list with nothing in it yet.
        if !self.sidebar_media.contains_key(&conv.id) {
            self.load_media_page(cx);
        }

        // Transparent Media Sidebar container (no solid bg, no border)
        div()
            .relative()
            .w(px(320.))
            .flex_shrink_0()
            .h_full()
            .bg(gpui::transparent_black())
            .flex()
            .flex_col()
            .when(self.call_connected, |d| d.child(call_glow()))
            // The tabs stay put, clear of the window buttons, and the cards run under them.
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("media-sidebar-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.media_scroll)
                            .pt(px(100.))
                            .px(px(20.))
                            .pb(px(20.))
                            .child(media_cards),
                    )
                    .child(div().absolute().top(px(44.)).left(px(20.)).right(px(20.)).child(tabs_row)),
            )
    }
}
