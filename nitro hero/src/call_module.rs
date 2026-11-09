//! All of a call's controls in one module, built from the same pieces as the tabs. Sounds and
//! audio settings open inside it, so it stays one object.
use crate::{
    Preview,
    components::{self, CallTile, StrokeKind, label},
    sound_wheel,
    state::HeaderDropdown,
};
use gpui::{
    Animation, AnimationExt, Context, Div, FontWeight, MouseButton, SharedString, Stateful, Window, canvas, div, prelude::*, px, relative, rgb, rgba, svg,
};
use std::time::Duration;

pub const WIDTH: f32 = 520.;
/// How long the module takes to shrink away when a call ends.
pub const EXIT: Duration = Duration::from_millis(340);
/// About the size of the Voice button it grows out of and shrinks back into.
const SMALLEST: f32 = 92.;
const LEAVE_WIDTH: f32 = 72.;
const COMPACT_WIDTH: f32 = 420.;
pub const HEIGHT: f32 = components::CONTROL_HEIGHT;
const RADIUS: f32 = HEIGHT / 2.;
const SOUNDS_PER_ROW: usize = 4;
/// A little over four rows, so it's clear there are more.
const SOUNDS_HEIGHT: f32 = 262.;

fn call_clock(seconds: u64) -> String {
    let (hours, minutes, rest) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{rest:02}") } else { format!("{minutes}:{rest:02}") }
}

/// The module's surface. Anything sitting beside it uses the same, so they match.
pub fn module_backing() -> Div {
    components::tab_well(RADIUS)
}

impl Preview {
    fn toggle_module_panel(&mut self, panel: HeaderDropdown, cx: &mut Context<Self>) {
        self.open_dropdown = if self.open_dropdown == Some(panel) { None } else { Some(panel) };
        cx.notify();
    }

    fn toggle_favourite_sound(&mut self, id: &str, cx: &mut Context<Self>) {
        let favourites = &mut self.state.preferences.pinned_sound_ids;
        match favourites.iter().position(|saved| saved == id) {
            Some(position) => {
                favourites.remove(position);
            }
            None => favourites.push(id.to_string()),
        }
        self.persist();
        cx.notify();
    }

    /// Every sound in one list that scrolls, favourites at the top.
    fn render_module_sounds(&self, inner_width: f32, cx: &mut Context<Self>) -> Div {
        let favourites = &self.state.preferences.pinned_sound_ids;
        let sounds = sound_wheel::ordered(&self.soundboard_clips, favourites);
        let mut grid = div().flex().flex_col().gap(px(6.));
        for (row_index, row) in sounds.chunks(SOUNDS_PER_ROW).enumerate() {
            let mut cells = div().flex().gap(px(6.));
            for (column, clip) in row.iter().enumerate() {
                let index = row_index * SOUNDS_PER_ROW + column;
                let playing = self.playing_sound_id.as_deref() == Some(clip.id.as_str());
                let starred = favourites.contains(&clip.id);
                let progress = if playing {
                    self.playing_since.map_or(0., |since| {
                        (since.elapsed().as_millis() as f32 / self.playing_ms.max(1) as f32).clamp(0., 1.)
                    })
                } else {
                    0.
                };
                let (play_id, star_id) = (clip.id.clone(), clip.id.clone());
                cells = cells.child(
                    div()
                        .id(("module-sound", index))
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .h(px(52.))
                        .rounded(px(14.))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .pl(px(10.))
                        .pr(px(4.))
                        .when(playing, |pad| pad.child(components::tab_slab(14.).absolute().top_0().left_0().size_full()))
                        .when(!playing, |pad| pad.bg(rgba(0xffffff0f)).hover(|s| s.bg(rgba(0xffffff1c))))
                        .on_click(cx.listener(move |this, _, _, cx| this.play_clip_in_call(play_id.clone(), cx)))
                        .child(
                            div()
                                .relative()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().text_size(px(12.5)).font_weight(FontWeight::MEDIUM).text_color(rgb(0xffffff)).truncate().child(clip.name.clone()))
                                .child(div().text_size(px(10.5)).text_color(rgb(0xb4b4bc)).child(clip.duration_label.clone())),
                        )
                        .child(
                            div()
                                .id(("module-sound-star", index))
                                .relative()
                                .flex_shrink_0()
                                .size(px(26.))
                                .rounded(px(8.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(rgba(0xffffff14)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.toggle_favourite_sound(&star_id, cx);
                                }))
                                .when(starred, |star| {
                                    star.child(svg().path("Star").size(px(13.)).text_color(rgb(0xf5c84b)).with_animation(
                                        ("module-star-pop", index),
                                        Animation::new(Duration::from_millis(320)),
                                        |star, delta| star.size(px(7. + 6. * components::back_out(delta))),
                                    ))
                                })
                                .when(!starred, |star| star.child(svg().path("Star").size(px(13.)).text_color(rgb(0x7c7c86)))),
                        )
                        .when(playing, |pad| {
                            pad.child(
                                div()
                                    .absolute()
                                    .left(px(10.))
                                    .right(px(10.))
                                    .bottom(px(4.))
                                    .h(px(2.))
                                    .child(div().h_full().rounded_full().w(relative(progress)).bg(rgba(0xffffffb3))),
                            )
                        }),
                );
            }
            for _ in row.len()..SOUNDS_PER_ROW {
                cells = cells.child(div().flex_1().min_w_0());
            }
            grid = grid.child(cells);
        }
        let empty: Option<SharedString> = match &self.soundboard_status {
            crate::SoundboardStatus::Failed(message) => Some(message.clone().into()),
            crate::SoundboardStatus::Loaded if sounds.is_empty() => Some("No sounds yet.".into()),
            crate::SoundboardStatus::Loaded => None,
            _ => Some("Loading sounds…".into()),
        };
        let volume = self.state.preferences.soundboard_volume;
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(components::slider_field("Soundboard volume", format!("{volume}%"), inner_width, self.slider_for(2, volume, 100, 5, inner_width, cx)))
            .when_some(empty, |panel, text| panel.child(div().h(px(52.)).flex().items_center().justify_center().child(label(text))))
            .child(div().id("module-sounds-scroll").max_h(px(SOUNDS_HEIGHT)).overflow_y_scroll().child(grid))
    }

    fn render_module_devices(&self, input: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let devices = if input { &self.audio_devices.inputs } else { &self.audio_devices.outputs };
        let current = if input { &self.state.preferences.input_device_id } else { &self.state.preferences.output_device_id };
        let group: &'static str = if input { "module-input" } else { "module-output" };
        let mut list = div().flex().flex_col().gap(px(2.));
        for (index, device) in devices.iter().enumerate() {
            let selected = *current == device.id;
            let id = device.id.clone();
            list = list.child(
                div()
                    .id((group, index))
                    .relative()
                    .h(px(32.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(px(10.))
                    .rounded(px(12.))
                    .cursor_pointer()
                    .when(selected, |row| row.child(components::tab_slab(12.).absolute().top_0().left_0().size_full()))
                    .when(!selected, |row| row.hover(|s| s.bg(rgba(0xffffff14))))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if input {
                            this.state.preferences.input_device_id = id.clone();
                        } else {
                            this.state.preferences.output_device_id = id.clone();
                        }
                        this.send_voice_routes();
                        this.persist();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(rgb(if selected { 0xffffff } else { 0xd4d4d8 }))
                            .truncate()
                            .child(device.name.clone()),
                    ),
            );
        }
        div().id(if input { "module-input-scroll" } else { "module-output-scroll" }).flex_1().min_w_0().max_h(px(170.)).overflow_y_scroll().child(list)
    }

    /// The volumes and device list for the microphone, or for what you hear.
    fn render_module_audio(&self, input: bool, inner_width: f32, cx: &mut Context<Self>) -> Div {
        let prefs = &self.state.preferences;
        let half = (inner_width - 16.) / 2.;
        let partner = self.active_conv().display_name.clone();
        let sliders = if input {
            div().child(components::slider_field("Mic volume", format!("{}%", prefs.input_volume), inner_width, self.slider_for(3, prefs.input_volume, 100, 1, inner_width, cx)))
        } else {
            div()
                .flex()
                .gap(px(16.))
                .child(components::slider_field(format!("{partner}'s volume"), format!("{}%", prefs.partner_volume), half, self.slider_for(4, prefs.partner_volume, 200, 5, half, cx)))
                .child(components::slider_field("Output volume", format!("{}%", prefs.output_volume), half, self.slider_for(5, prefs.output_volume, 100, 1, half, cx)))
        };
        div().flex().flex_col().gap(px(10.)).child(sliders).child(self.render_module_devices(input, cx))
    }

    fn render_module_screen(&self, cx: &mut Context<Self>) -> Div {
        let mut list = div().flex().flex_col().gap(px(2.));
        for (index, source) in self.voice_state.screen.sources.iter().enumerate() {
            let chosen = source.clone();
            list = list.child(
                div()
                    .id(("module-screen-source", index))
                    .h(px(32.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px(px(10.))
                    .rounded(px(12.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0xffffff14)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.start_screen_share(chosen.id.clone(), chosen.kind.clone(), chosen.title.clone(), cx);
                    }))
                    .child(div().min_w_0().text_size(px(12.)).text_color(rgb(0xd4d4d8)).truncate().child(source.title.clone())),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().id("module-screen-scroll").max_h(px(220.)).overflow_y_scroll().child(list))
            .child(div().px(px(4.)).child(label("One viewer at a time. Stops after 10 minutes.")))
    }

    /// One button that's the Voice or Join button at 0 and Leave at 1.
    fn render_morphing_button(&self, id: &'static str, amount: f32, joinable: bool) -> Stateful<Div> {
        let shown = amount.clamp(0., 1.);
        let outgoing = (1. - shown * 2.).clamp(0., 1.);
        let incoming = (shown * 2. - 1.).clamp(0., 1.);
        let surface = |fill: u32, opacity: f32| {
            components::material_surface(fill, components::INNER_HEIGHT / 2., StrokeKind::Colored).absolute().top_0().left_0().size_full().opacity(opacity)
        };
        div()
            .id(id)
            .group(id)
            .absolute()
            .top(px(4.))
            .right(px(4.))
            .h(px(components::INNER_HEIGHT))
            .w(px(SMALLEST - 8. + (LEAVE_WIDTH - (SMALLEST - 8.)) * amount))
            .rounded(px(components::INNER_HEIGHT / 2.))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .capture_any_mouse_down(components::press_sound)
            .when(joinable, |button| button.child(surface(components::MATERIAL_GREEN, 1. - shown)))
            .child(surface(components::MATERIAL_RED, shown))
            .child(components::hover_sheen(id, components::INNER_HEIGHT / 2.))
            .active(|s| s.opacity(0.85))
            .when(outgoing > 0., |button| {
                button.child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .opacity(outgoing)
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(if joinable { 0xffffff } else { 0xd4d4d8 }))
                        .when(!joinable, |label| label.child(svg().path("CallPhone").size(px(18.)).text_color(rgb(0xd4d4d8))))
                        .child(if joinable { "Join" } else { "Voice" }),
                )
            })
            .when(incoming > 0., |button| {
                button.child(div().relative().opacity(incoming).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(0xffffff)).child("Leave"))
            })
    }

    /// What's left of the module for a moment after the call ends, turning back into Voice.
    pub fn render_module_exit(&self, progress: f32) -> Div {
        let amount = 1. - components::tabs_curve(progress);
        div()
            .relative()
            .h(px(HEIGHT))
            .w(px(SMALLEST + (WIDTH - SMALLEST) * amount.max(0.)))
            .child(module_backing().absolute().top_0().left_0().size_full())
            .child(self.render_morphing_button("module-leave-exit", amount, self.voice_joinable))
    }

    pub fn render_call_module(
        &mut self,
        people: Vec<(SharedString, u32, components::AvatarImage)>,
        seconds: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let width = if self.call_module_compact { COMPACT_WIDTH } else { WIDTH };
        // Springs out from about where the Voice button was.
        let opening = self.module_open.step(1., self.module_step);
        let open_amount = self.module_open.value;
        if opening {
            window.request_animation_frame();
        }
        let open = self.open_dropdown;
        let mic_open = open == Some(HeaderDropdown::Microphone);
        let output_open = open == Some(HeaderDropdown::Output);
        let sounds_open = open == Some(HeaderDropdown::Soundboard);
        let sharing = self.voice_state.screen.share != crate::screen_share::Stage::Idle;
        let screen_open = open == Some(HeaderDropdown::Screen);
        let lit = |on: bool| if on { CallTile::Selected } else { CallTile::Idle };
        let alarm = |on: bool| if on { CallTile::Alarm } else { CallTile::Idle };

        let hearing = components::button_group(components::MATERIAL_DARK, StrokeKind::Neutral)
            .flex_shrink_0()
            .child(
                components::call_segment("module-mic", 0, 4, if self.call_muted { "CallMicOff" } else { "CallMic" }, 44., alarm(self.call_muted))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_mute(cx))),
            )
            .child(components::call_segment("module-mic-menu", 1, 4, "CallChevronDown", 30., lit(mic_open)).on_click(cx.listener(|this, _, _, cx| {
                this.refresh_audio_devices(cx);
                this.toggle_module_panel(HeaderDropdown::Microphone, cx);
            })))
            .child(
                components::call_segment("module-headset", 2, 4, if self.call_deafened { "CallHeadphoneOff" } else { "CallHeadphones" }, 44., alarm(self.call_deafened))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_deafen(cx))),
            )
            .child(components::call_segment("module-headset-menu", 3, 4, "CallChevronDown", 30., lit(output_open)).on_click(cx.listener(|this, _, _, cx| {
                this.refresh_audio_devices(cx);
                this.toggle_module_panel(HeaderDropdown::Output, cx);
            })));
        let extras = components::button_group(components::MATERIAL_DARK, StrokeKind::Neutral)
            .flex_shrink_0()
            .child(
                components::call_segment("module-screen", 0, 2, "CallScreenShare", 44., if sharing { CallTile::Live } else { lit(screen_open) })
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.voice_state.screen.share != crate::screen_share::Stage::Idle {
                            this.voice.control(crate::voice::Control::ScreenStop);
                        } else {
                            this.voice.control(crate::voice::Control::ScreenSources);
                            this.toggle_module_panel(HeaderDropdown::Screen, cx);
                        }
                        cx.notify();
                    })),
            )
            .child(components::call_segment("module-sounds", 1, 2, "CallAudioLines", 44., lit(sounds_open)).on_click(cx.listener(|this, _, _, cx| {
                this.load_soundboard(cx);
                this.toggle_module_panel(HeaderDropdown::Soundboard, cx);
            })));
        let bar = div()
            .h(px(components::INNER_HEIGHT))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .pl(px(6.))
                    .when(!self.call_module_compact, |identity| identity.child(components::avatar_stack(people)))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(0xffffff)).child(call_clock(seconds))),
            )
            .child(hearing)
            .child(extras)
            .pr(px(LEAVE_WIDTH + 6.))
            .opacity(((open_amount - 0.35) / 0.65).clamp(0., 1.));

        // Starts out as the Voice button that was just pressed, and becomes Leave.
        let leave = self
            .render_morphing_button("module-leave", open_amount, self.voice_joinable)
            .on_click(cx.listener(|this, _, _, cx| this.leave_voice(cx)));
        let inner_width = width - 8. - 16.;
        let wanted = open.filter(|panel| *panel != HeaderDropdown::Switcher);
        if wanted.is_some() {
            self.module_panel_kind = wanted;
        }
        let natural = self.module_panel_natural.get();
        let moving = self.module_panel.step(if wanted.is_some() { natural } else { 0. }, self.module_step);
        if moving || (wanted.is_some() && natural <= 0.) {
            window.request_animation_frame();
        }
        // Stays drawn as it closes.
        let shown = self.module_panel_kind.filter(|_| wanted.is_some() || self.module_panel.value > 0.5);
        let panel = match shown {
            Some(HeaderDropdown::Soundboard) => Some(self.render_module_sounds(inner_width, cx)),
            Some(HeaderDropdown::Microphone) => Some(self.render_module_audio(true, inner_width, cx)),
            Some(HeaderDropdown::Output) => Some(self.render_module_audio(false, inner_width, cx)),
            Some(HeaderDropdown::Screen) => Some(self.render_module_screen(cx)),
            _ => None,
        };
        let height = self.module_panel.value;
        let measured = self.module_panel_natural.clone();
        let backing = module_backing();
        div()
            .id("call-module")
            .relative()
            .w(px(SMALLEST + (width - SMALLEST) * open_amount))
            .when(opening, |module| module.overflow_hidden())
            .flex_shrink_0()
            .flex()
            .flex_col()
            .p(px(4.))
            // Nothing behind it can be pressed or hovered through it.
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(backing.absolute().top_0().left_0().size_full())
            .child(bar)
            .child(leave)
            .when_some(panel, |module, panel| {
                module.child(
                    div().relative().h(px(height)).overflow_hidden().child(
                        div()
                            .relative()
                            .px(px(8.))
                            .pt(px(10.))
                            .pb(px(8.))
                            .child(
                                canvas(
                                    move |bounds, window, _| {
                                        let natural = f32::from(bounds.size.height);
                                        if (measured.get() - natural).abs() > 0.5 {
                                            measured.set(natural);
                                            window.request_animation_frame();
                                        }
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full(),
                            )
                            .child(panel),
                    ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::call_clock;

    #[test]
    fn the_timer_only_shows_hours_once_there_are_some() {
        assert_eq!(call_clock(42), "0:42");
        assert_eq!(call_clock(768), "12:48");
        assert_eq!(call_clock(3_725), "1:02:05");
    }
}
