//! The full-window previews. An image fits the window and can be zoomed and dragged, as in main
//! Nitro. A video fits the window and keeps its controls. Whatever's left over is black.
use crate::{
    Preview,
    components::{self, MATERIAL_DARK, StrokeKind, badge, material_surface, rgb},
};
use gpui::{
    AnyElement, Context, CursorStyle, Div, MouseButton, SharedString, Stateful, Window, deferred, div, prelude::*, px,
    rgba, svg,
};
use std::path::PathBuf;

const MOST_ZOOM: f32 = 5.;
/// A press that moves further than this was a drag, not a click.
const CLICK_DRAG_THRESHOLD: f32 = 4.;
const SEEK_STEPS: u16 = 1000;
const KEYBOARD_SEEK_SECONDS: f64 = 5.;
const CONTROLS_LINGER_SECONDS: f32 = 2.5;

type Pair = (f32, f32);

struct ImageDrag {
    start: Pair,
    origin: Pair,
    moved: bool,
}

pub struct ImageViewer {
    path: PathBuf,
    /// The file's own size, if it could be read.
    natural: Option<Pair>,
    scale: f32,
    /// From the centre of the window, in pixels.
    offset: Pair,
    drag: Option<ImageDrag>,
}

/// The size at 100%: filling the window by whichever of its height or width comes first.
fn contained(view: Pair, natural: Pair) -> Pair {
    if natural.0 <= 0. || natural.1 <= 0. || view.1 <= 0. {
        return view;
    }
    if natural.0 / natural.1 > view.0 / view.1 {
        (view.0, view.0 * natural.1 / natural.0)
    } else {
        (view.1 * natural.0 / natural.1, view.1)
    }
}

/// Keeps the picture's edge from pulling in from the window's edge. Along a side where it's
/// still smaller than the window, it stays centred.
fn clamp_offset(offset: Pair, scale: f32, view: Pair, natural: Pair) -> Pair {
    let (width, height) = contained(view, natural);
    let (most_x, most_y) = (((width * scale - view.0) / 2.).max(0.), ((height * scale - view.1) / 2.).max(0.));
    (offset.0.clamp(-most_x, most_x), offset.1.clamp(-most_y, most_y))
}

/// The scale and offset after zooming, with the point under the cursor staying under it.
fn zoom_at(scale: f32, offset: Pair, factor: f32, cursor: Pair, view: Pair, natural: Pair) -> (f32, Pair) {
    let next = (scale * factor).clamp(1., MOST_ZOOM);
    let change = next / scale;
    let (from_centre_x, from_centre_y) = (cursor.0 - view.0 / 2., cursor.1 - view.1 / 2.);
    let moved = (
        from_centre_x - (from_centre_x - offset.0) * change,
        from_centre_y - (from_centre_y - offset.1) * change,
    );
    (next, clamp_offset(moved, next, view, natural))
}

impl ImageViewer {
    pub fn open(path: PathBuf) -> Self {
        // Media in the cache has no file extension, so its type comes from its content.
        let natural = image::ImageReader::open(&path)
            .ok()
            .and_then(|reader| reader.with_guessed_format().ok())
            .and_then(|reader| reader.into_dimensions().ok())
            .map(|(width, height)| (width as f32, height as f32));
        Self { path, natural, scale: 1., offset: (0., 0.), drag: None }
    }

    fn natural_or(&self, view: Pair) -> Pair {
        self.natural.unwrap_or(view)
    }
}

fn clock(seconds: f64) -> String {
    let whole = seconds.max(0.) as u64;
    format!("{}:{:02}", whole / 60, whole % 60)
}

fn icon_button(id: impl Into<SharedString>, icon: &'static str, size: f32) -> Stateful<Div> {
    let id: SharedString = id.into();
    div()
        .id(id)
        .cursor_pointer()
        .flex_shrink_0()
        .size(px(size))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .hover(|style| style.bg(rgba(0xffffff24)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(svg().path(icon).size(px(size * 0.55)).text_color(rgb(0xf2f2f3)))
}

impl Preview {
    /// Play, position, time, sound and full size. `width` decides what fits. Only for the video
    /// that's currently going.
    pub fn render_video_controls(&self, width: f32, expanded: bool, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let playing = self.playing_video.as_ref()?;
        let player = playing.player.as_ref()?;
        let (position, length, paused) = (player.position(), player.length(), player.paused());
        let full = length > 0.;
        let at = if full { ((position / length) * f64::from(SEEK_STEPS)) as u16 } else { 0 };
        let time = format!("{} / {}", clock(position), clock(length));
        let show_time = width >= 300.;
        let show_volume = expanded;
        let muted = self.video_muted;
        let mut taken = 8. + 8. + 28. + 6. + 28. + 6. + 28.;
        if show_time {
            taken += 92.;
        }
        if show_volume {
            taken += 116.;
        }
        let seek_width = (width - taken).max(40.);
        let seek = cx.listener(|this, value: &u16, _, cx| {
            if let Some(player) = this.playing_video.as_ref().and_then(|playing| playing.player.as_ref()) {
                player.seek(player.length() * f64::from(*value) / f64::from(SEEK_STEPS));
            }
            cx.notify();
        });
        let volume = cx.listener(|this, value: &u16, _, cx| {
            this.video_volume = (*value).min(100) as u8;
            this.video_muted = false;
            cx.notify();
        });
        Some(
            div()
                .id("video-controls")
                .w_full()
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .py(px(6.))
                .bg(gpui::linear_gradient(
                    0.,
                    gpui::linear_color_stop(rgba(0x000000c4), 0.),
                    gpui::linear_color_stop(rgba(0x00000000), 1.),
                ))
                // Presses on the bar are about the bar, not the picture behind it.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    icon_button("video-pause", if paused { "Play" } else { "Pause" }, 28.).on_click(cx.listener(
                        |this, _, _, cx| {
                            cx.stop_propagation();
                            this.toggle_video_pause();
                            cx.notify();
                        },
                    )),
                )
                .child(
                    components::slider(10, &self.sliders[10], at, SEEK_STEPS, 1, seek_width, std::rc::Rc::new(seek))
                        .flex_shrink_0(),
                )
                .when(show_time, |bar| {
                    bar.child(
                        div()
                            .flex_shrink_0()
                            .w(px(86.))
                            .text_size(px(12.))
                            .text_color(rgb(0xf2f2f3))
                            .text_center()
                            .child(time),
                    )
                })
                .child(
                    icon_button("video-mute", if muted { "VolumeOff" } else { "Volume" }, 28.).on_click(cx.listener(
                        |this, _, _, cx| {
                            cx.stop_propagation();
                            this.video_muted = !this.video_muted;
                            cx.notify();
                        },
                    )),
                )
                .when(show_volume, |bar| {
                    let shown = if muted { 0 } else { u16::from(self.video_volume) };
                    bar.child(
                        components::slider(11, &self.sliders[11], shown, 100, 1, 110., std::rc::Rc::new(volume))
                            .flex_shrink_0(),
                    )
                })
                .child(
                    icon_button("video-expand", if expanded { "Shrink" } else { "Expand" }, 28.).on_click(cx.listener(
                        |this, _, _, cx| {
                            cx.stop_propagation();
                            if let Some(playing) = &mut this.playing_video {
                                playing.expanded = !playing.expanded;
                            }
                            this.video_controls_shown_at = std::time::Instant::now();
                            cx.notify();
                        },
                    )),
                ),
        )
    }

    fn render_image_viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let viewer = self.lightbox.as_ref()?;
        let size = window.viewport_size();
        let view = (f32::from(size.width), f32::from(size.height));
        let natural = viewer.natural_or(view);
        let (width, height) = contained(view, natural);
        let (shown_width, shown_height) = (width * viewer.scale, height * viewer.scale);
        let left = view.0 / 2. + viewer.offset.0 - shown_width / 2.;
        let top = view.1 / 2. + viewer.offset.1 - shown_height / 2.;
        let cursor = if viewer.scale > 1. {
            if viewer.drag.is_some() { CursorStyle::ClosedHand } else { CursorStyle::OpenHand }
        } else {
            CursorStyle::PointingHand
        };
        let zoomed = viewer.scale > 1.001;
        let percent = (viewer.scale * 100.).round() as u32;
        let path = viewer.path.clone();
        Some(
            deferred(
                div()
                    .id("lightbox")
                    .track_focus(&self.viewer_focus)
                    .occlude()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .overflow_hidden()
                    .bg(gpui::rgba(0x000000f5))
                    .cursor(cursor)
                    .on_scroll_wheel(cx.listener(|this, event: &gpui::ScrollWheelEvent, window, cx| {
                        let Some(viewer) = &mut this.lightbox else { return };
                        let size = window.viewport_size();
                        let view = (f32::from(size.width), f32::from(size.height));
                        let change = f32::from(event.delta.pixel_delta(px(20.)).y);
                        let cursor = (f32::from(event.position.x), f32::from(event.position.y));
                        let natural = viewer.natural_or(view);
                        (viewer.scale, viewer.offset) =
                            zoom_at(viewer.scale, viewer.offset, (change * 0.005).exp(), cursor, view, natural);
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if let Some(viewer) = &mut this.lightbox {
                            viewer.drag = Some(ImageDrag {
                                start: (f32::from(event.position.x), f32::from(event.position.y)),
                                origin: viewer.offset,
                                moved: false,
                            });
                            cx.notify();
                        }
                    }))
                    .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                        let Some(viewer) = &mut this.lightbox else { return };
                        let natural_view = window.viewport_size();
                        let view = (f32::from(natural_view.width), f32::from(natural_view.height));
                        let natural = viewer.natural_or(view);
                        let (scale, offset) = (viewer.scale, viewer.offset);
                        let Some(drag) = &mut viewer.drag else { return };
                        // The button came up outside the window.
                        if event.pressed_button != Some(MouseButton::Left) {
                            viewer.drag = None;
                            cx.notify();
                            return;
                        }
                        let (x, y) = (
                            f32::from(event.position.x) - drag.start.0,
                            f32::from(event.position.y) - drag.start.1,
                        );
                        if x.hypot(y) > CLICK_DRAG_THRESHOLD {
                            drag.moved = true;
                        }
                        if scale > 1. {
                            let next = clamp_offset((drag.origin.0 + x, drag.origin.1 + y), scale, view, natural);
                            if next != offset {
                                viewer.offset = next;
                                cx.notify();
                            }
                        }
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                        let Some(viewer) = &mut this.lightbox else { return };
                        // A drag that moved it mustn't also close it.
                        match viewer.drag.take() {
                            Some(drag) if !drag.moved => this.lightbox = None,
                            _ => {}
                        }
                        cx.notify();
                    }))
                    .child(
                        gpui::img(path)
                            .absolute()
                            .left(px(left))
                            .top(px(top))
                            .w(px(shown_width))
                            .h(px(shown_height))
                            .object_fit(gpui::ObjectFit::Fill),
                    )
                    // The window can still be moved by its top edge, as in main Nitro.
                    .child(
                        div()
                            .id("lightbox-drag")
                            .absolute()
                            .top_0()
                            .left_0()
                            .right(px(60.))
                            .h(px(36.))
                            .block_mouse_except_scroll()
                            .window_control_area(gpui::WindowControlArea::Drag),
                    )
                    .child(div().absolute().top(px(8.)).right(px(12.)).child(
                        close_button("lightbox-close").on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.lightbox = None;
                            cx.notify();
                        })),
                    ))
                    .when(zoomed, |viewer| {
                        viewer.child(
                            div()
                                .absolute()
                                .bottom(px(16.))
                                .left_0()
                                .right_0()
                                .flex()
                                .justify_center()
                                .child(badge(format!("{percent}%"), MATERIAL_DARK, 0xf2f2f3)),
                        )
                    }),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    fn render_video_viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let playing = self.playing_video.as_ref().filter(|playing| playing.expanded)?;
        let frame = playing.frame.clone();
        let paused = playing.player.as_ref().is_some_and(|player| player.paused());
        let width = f32::from(window.viewport_size().width);
        let controls = self.render_video_controls(width, true, cx);
        let shown = paused || self.video_controls_shown_at.elapsed().as_secs_f32() < CONTROLS_LINGER_SECONDS;
        Some(
            deferred(
                div()
                    .id("video-viewer")
                    .track_focus(&self.viewer_focus)
                    .occlude()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .overflow_hidden()
                    .bg(rgb(0x000000))
                    .cursor_pointer()
                    .on_mouse_move(cx.listener(|this, _: &gpui::MouseMoveEvent, _, cx| {
                        this.video_controls_shown_at = std::time::Instant::now();
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_video_pause();
                        cx.notify();
                    }))
                    .when_some(frame, |viewer, frame| viewer.child(components::video_frame(frame, 0.)))
                    .when(paused, |viewer| {
                        viewer.child(
                            div().absolute().inset_0().flex().items_center().justify_center().child(
                                material_surface(MATERIAL_DARK, 32., StrokeKind::Neutral)
                                    .size(px(64.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(svg().path("Play").size(px(26.)).text_color(rgb(0xffffff))),
                            ),
                        )
                    })
                    .child(
                        div()
                            .id("video-viewer-drag")
                            .absolute()
                            .top_0()
                            .left_0()
                            .right(px(60.))
                            .h(px(36.))
                            .block_mouse_except_scroll()
                            .window_control_area(gpui::WindowControlArea::Drag),
                    )
                    .when(shown, |viewer| {
                        viewer
                            .child(div().absolute().top(px(8.)).right(px(12.)).child(
                                close_button("video-viewer-close").on_click(cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(playing) = &mut this.playing_video {
                                        playing.expanded = false;
                                    }
                                    cx.notify();
                                })),
                            ))
                            .when_some(controls, |viewer, controls| {
                                viewer.child(div().absolute().bottom_0().left_0().right_0().child(controls))
                            })
                    }),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    pub fn render_viewers(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.lightbox.is_some() || self.playing_video.as_ref().is_some_and(|playing| playing.expanded);
        // Keys only arrive with something focused, and Esc has to work however it was opened.
        // A slider inside the preview can hold focus without losing that.
        if open && !self.viewer_focus.contains_focused(window, cx) {
            window.focus(&self.viewer_focus);
        }
        // Closing hands typing back to the composer.
        if self.viewer_was_open && !open {
            let input = self.chat_input.clone();
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        self.viewer_was_open = open;
        self.render_image_viewer(window, cx).or_else(|| self.render_video_viewer(window, cx))
    }

    /// Keys while a preview is open. True if the key was for it.
    pub fn viewer_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.lightbox.is_some() {
            if key == "escape" {
                self.lightbox = None;
                cx.notify();
                return true;
            }
            return false;
        }
        if !self.playing_video.as_ref().is_some_and(|playing| playing.expanded) {
            return false;
        }
        match key {
            "escape" => {
                if let Some(playing) = &mut self.playing_video {
                    playing.expanded = false;
                }
            }
            "space" => self.toggle_video_pause(),
            "left" | "right" => {
                if let Some(player) = self.playing_video.as_ref().and_then(|playing| playing.player.as_ref()) {
                    let step = if key == "left" { -KEYBOARD_SEEK_SECONDS } else { KEYBOARD_SEEK_SECONDS };
                    player.seek(player.position() + step);
                }
            }
            _ => return false,
        }
        self.video_controls_shown_at = std::time::Instant::now();
        cx.notify();
        true
    }
}

fn close_button(id: &'static str) -> Stateful<Div> {
    material_surface(MATERIAL_DARK, 16., StrokeKind::Neutral)
        .id(id)
        .cursor_pointer()
        .size(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(18.))
        .text_color(rgb(0xf2f2f3))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child("×")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_100_percent_it_fills_by_height_or_width_and_never_overflows() {
        let view = (1000., 600.);
        assert_eq!(contained(view, (2000., 1000.)), (1000., 500.));
        assert_eq!(contained(view, (500., 1000.)), (300., 600.));
        // Small pictures come up to fill too.
        assert_eq!(contained(view, (100., 60.)), (1000., 600.));
    }

    #[test]
    fn it_can_only_be_dragged_as_far_as_its_edges() {
        let (view, natural) = ((1000., 600.), (1000., 600.));
        assert_eq!(clamp_offset((50., 50.), 1., view, natural), (0., 0.));
        assert_eq!(clamp_offset((900., -900.), 2., view, natural), (500., -300.));
        // A tall picture zoomed a little: still centred sideways, free up and down.
        assert_eq!(clamp_offset((90., 90.), 1.5, view, (300., 600.)), (0., 90.));
    }

    #[test]
    fn zooming_keeps_the_point_under_the_cursor_where_it_is() {
        let (view, natural) = ((1000., 600.), (1000., 600.));
        let cursor = (800., 450.);
        let (scale, offset) = zoom_at(1., (0., 0.), 2., cursor, view, natural);
        assert_eq!(scale, 2.);
        // The point was 300 right and 150 down of centre: now 600 and 300 of the picture's centre.
        assert_eq!(offset, (-300., -150.));
        let (scale, offset) = zoom_at(scale, offset, 0.5, cursor, view, natural);
        assert_eq!((scale, offset), (1., (0., 0.)));
        assert_eq!(zoom_at(4., (0., 0.), 100., cursor, view, natural).0, MOST_ZOOM);
        assert_eq!(zoom_at(1., (0., 0.), 0.1, cursor, view, natural).0, 1.);
    }

    #[test]
    fn times_read_as_minutes_and_seconds() {
        assert_eq!(clock(0.), "0:00");
        assert_eq!(clock(90.4), "1:30");
        assert_eq!(clock(-1.), "0:00");
    }
}
