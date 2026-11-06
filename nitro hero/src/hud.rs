//! The desktop call overlay: a small always-on-top window that can't take focus or clicks.
use crate::components::{OverlayPerson, OverlayStream, call_overlay};
use gpui::{
    App, AppContext, Bounds, Context, Render, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, point, prelude::*,
};

pub struct Hud {
    people: Vec<OverlayPerson>,
    scale: f32,
    show_names: bool,
    levels: std::collections::HashMap<String, f32>,
    last_frame: std::time::Instant,
    animating: bool,
    stream: Option<OverlayStream>,
    /// Frames to free from this window's image atlas on the next render.
    retired: Vec<std::sync::Arc<gpui::RenderImage>>,
}

impl Render for Hud {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        for frame in self.retired.drain(..) {
            let _ = window.drop_image(frame);
        }
        let dt = self.last_frame.elapsed().as_secs_f32().min(0.1);
        self.last_frame = std::time::Instant::now();
        let mut visible = self.people.clone();
        self.animating = false;
        self.levels.retain(|id, _| visible.iter().any(|person| &person.id == id));
        for person in &mut visible {
            let target = if person.muted { 0. } else { person.level.clamp(0., 1.) };
            let current = self.levels.entry(person.id.clone()).or_default();
            let tau = if target > *current { 0.075 } else { 0.18 };
            *current += (target - *current) * (1. - (-dt / tau).exp());
            if (target - *current).abs() > 0.002 { self.animating = true; }
            else { *current = target; }
            person.level = *current;
        }
        if self.animating { window.request_animation_frame(); }
        call_overlay(&visible, self.scale, self.show_names, self.stream.as_ref())
    }
}

/// Opens, updates or closes the overlay. `None` closes it.
pub fn sync(
    handle: &mut Option<WindowHandle<Hud>>,
    people: Option<Vec<OverlayPerson>>,
    scale: f32,
    show_names: bool,
    stream: Option<OverlayStream>,
    cx: &mut App,
) {
    let Some(people) = people else {
        if let Some(hud) = handle.take() {
            let _ = hud.update(cx, |_, window, _| window.remove_window());
        }
        return;
    };
    let Some(display) = cx.primary_display() else {
        return;
    };
    let screen = display.bounds();
    let extent = crate::components::overlay_size(people.len(), scale, show_names, stream.is_some());
    let bounds = Bounds::new(
        point(
            screen.origin.x,
            screen.origin.y + (screen.size.height - extent.height) * 0.5,
        ),
        extent,
    );
    if let Some(hud) = handle {
        let updated = hud.update(cx, |hud, window, cx| {
            if window.bounds() != bounds {
                position(window, bounds);
            }
            hud.people = people.clone();
            let next = stream.clone();
            if let Some(old) = std::mem::replace(&mut hud.stream, next) {
                let kept = hud.stream.as_ref().is_some_and(|new| std::sync::Arc::ptr_eq(&new.frame, &old.frame));
                if !kept {
                    hud.retired.push(old.frame);
                }
            }
            if !hud.animating { hud.last_frame = std::time::Instant::now(); }
            hud.scale = scale;
            hud.show_names = show_names;
            cx.notify();
        });
        if updated.is_ok() {
            return;
        }
        *handle = None;
    }
    *handle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id: Some(display.id()),
                kind: WindowKind::PopUp,
                titlebar: None,
                focus: false,
                show: true,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| Hud {
                    people,
                    scale,
                    show_names,
                    levels: Default::default(),
                    last_frame: std::time::Instant::now(),
                    animating: false,
                    stream,
                    retired: Vec::new(),
                })
            },
        )
        .ok();
    // Apply native frame/position changes AFTER GPUI finishes window creation;
    // otherwise its final setup can restore non-client insets for the first frame.
    if let Some(hud) = handle {
        let _ = hud.update(cx, |_, window, _| {
            make_passive(window);
            position(window, bounds);
        });
    }
}

#[cfg(windows)]
mod native {
    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
        pub fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
        pub fn SetWindowPos(
            hwnd: isize,
            after: isize,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> i32;
    }
    #[link(name = "dwmapi")]
    unsafe extern "system" {
        pub fn DwmSetWindowAttribute(hwnd: isize, attribute: u32, value: *const std::ffi::c_void, size: u32) -> i32;
    }
    pub const GWL_EXSTYLE: i32 = -20;
    pub const GWL_STYLE: i32 = -16;
    pub const WS_EX_TRANSPARENT: isize = 0x20;
    pub const WS_EX_TOOLWINDOW: isize = 0x80;
    pub const WS_EX_NOACTIVATE: isize = 0x0800_0000;
    pub const HWND_TOPMOST: isize = -1;
    pub const SWP_NOSIZE: u32 = 0x1;
    pub const SWP_NOMOVE: u32 = 0x2;
    pub const SWP_NOACTIVATE: u32 = 0x10;
    pub const SWP_SHOWWINDOW: u32 = 0x40;
    pub const SWP_FRAMECHANGED: u32 = 0x20;

    pub fn hwnd(window: &gpui::Window) -> Option<isize> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Win32(raw) => Some(raw.hwnd.get()),
            _ => None,
        }
    }
}

/// Clicks pass through it to whatever's underneath, and it never steals focus.
#[cfg(windows)]
fn make_passive(window: &Window) {
    use native::*;
    let Some(hwnd) = hwnd(window) else { return };
    unsafe {
        // A transparent popup still gets a Windows non-client frame unless removed.
        let frame = GetWindowLongPtrW(hwnd, GWL_STYLE);
        SetWindowLongPtrW(hwnd, GWL_STYLE, frame & !(0x00c00000 | 0x00040000 | 0x00800000));
        let disabled: u32 = 1; // DWMNCRP_DISABLED
        DwmSetWindowAttribute(hwnd, 2, (&disabled as *const u32).cast(), 4);
        let no_border: u32 = 0xfffffffe; // DWMWA_COLOR_NONE
        DwmSetWindowAttribute(hwnd, 34, (&no_border as *const u32).cast(), 4);
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            style | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
        );
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        );
    }
}

#[cfg(not(windows))]
fn make_passive(_: &Window) {}

#[cfg(windows)]
fn position(window: &Window, bounds: Bounds<gpui::Pixels>) {
    use native::*;
    let Some(hwnd) = hwnd(window) else { return };
    let scale = window.scale_factor();
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            (f32::from(bounds.origin.x) * scale).round() as i32,
            (f32::from(bounds.origin.y) * scale).round() as i32,
            (f32::from(bounds.size.width) * scale).round() as i32,
            (f32::from(bounds.size.height) * scale).round() as i32,
            SWP_NOACTIVATE,
        );
    }
}

#[cfg(not(windows))]
fn position(window: &mut Window, bounds: Bounds<gpui::Pixels>) {
    window.resize(bounds.size);
}
