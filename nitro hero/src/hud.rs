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
    /// The last source frame and its 16:9 crop, so level changes don't crop it again.
    crop: Option<(std::sync::Arc<gpui::RenderImage>, std::sync::Arc<gpui::RenderImage>)>,
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
    let source = stream.as_ref().map(|stream| {
        let size = stream.frame.size(0);
        (size.width.0.max(1) as u32, size.height.0.max(1) as u32)
    });
    let extent = crate::components::overlay_size(people.len(), scale, show_names, source);
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
                position(window, bounds, cx);
            }
            hud.people = people.clone();
            let next = stream.clone().map(|stream| {
                let frame = match &hud.crop {
                    Some((source, cropped)) if std::sync::Arc::ptr_eq(source, &stream.frame) => cropped.clone(),
                    _ => {
                        let cropped = crop_to_widescreen(&stream.frame);
                        hud.crop = Some((stream.frame.clone(), cropped.clone()));
                        cropped
                    }
                };
                OverlayStream { owner: stream.owner, frame }
            });
            if next.is_none() {
                hud.crop = None;
            }
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
                    stream: stream.map(|stream| OverlayStream {
                        frame: crop_to_widescreen(&stream.frame),
                        owner: stream.owner,
                    }),
                    retired: Vec::new(),
                    crop: None,
                })
            },
        )
        .ok();
    // Apply native frame/position changes AFTER GPUI finishes window creation;
    // otherwise its final setup can restore non-client insets for the first frame.
    if let Some(hud) = handle {
        let _ = hud.update(cx, |_, window, cx| {
            make_passive(window);
            position(window, bounds, cx);
        });
    }
}

/// The centre 16:9 of a frame, so the tile can fill with square-free rounded corners.
/// A frame already that shape is passed through without copying.
fn crop_to_widescreen(frame: &std::sync::Arc<gpui::RenderImage>) -> std::sync::Arc<gpui::RenderImage> {
    let size = frame.size(0);
    let (width, height) = (size.width.0.max(0) as u32, size.height.0.max(0) as u32);
    let (kept_width, kept_height) = widescreen_crop(width, height);
    if (kept_width, kept_height) == (width, height) {
        return frame.clone();
    }
    let Some(bytes) = frame.as_bytes(0) else { return frame.clone() };
    let left = ((width - kept_width) / 2) as usize * 4;
    let top = ((height - kept_height) / 2) as usize;
    let (stride, row) = (width as usize * 4, kept_width as usize * 4);
    let mut pixels = Vec::with_capacity(row * kept_height as usize);
    for line in bytes.chunks_exact(stride).skip(top).take(kept_height as usize) {
        pixels.extend_from_slice(&line[left..left + row]);
    }
    match image::RgbaImage::from_raw(kept_width, kept_height, pixels) {
        Some(pixels) => std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)])),
        None => frame.clone(),
    }
}

/// What's kept of a frame at 16:9. Within two pixels counts as already there: the tile's
/// fill covers that, and most screens would otherwise be copied for nothing.
fn widescreen_crop(width: u32, height: u32) -> (u32, u32) {
    let kept = if u64::from(width) * 9 > u64::from(height) * 16 {
        (((u64::from(height) * 16 / 9) as u32).max(1), height)
    } else {
        (width, ((u64::from(width) * 9 / 16) as u32).max(1))
    };
    if width.abs_diff(kept.0) <= 2 && height.abs_diff(kept.1) <= 2 { (width, height) } else { kept }
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

/// Moves and sizes the window, after the current app update has finished. Windows reports a
/// new size straight away, and GPUI can't take that report during an update: it drops it and
/// keeps drawing to the old area, which clips whatever no longer fits.
#[cfg(windows)]
fn position(window: &Window, bounds: Bounds<gpui::Pixels>, cx: &App) {
    use native::*;
    let Some(hwnd) = hwnd(window) else { return };
    let scale = window.scale_factor();
    cx.foreground_executor()
        .spawn(async move {
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
        })
        .detach();
}

#[cfg(not(windows))]
fn position(window: &mut Window, bounds: Bounds<gpui::Pixels>, _: &App) {
    window.resize(bounds.size);
}

/// Debug only: walks the overlay through the sizes a call goes through and prints whether
/// GPUI's drawing area matches the window each time. Uses made-up people, joins nothing.
#[cfg(debug_assertions)]
pub fn check(cx: &mut App) {
    use crate::components::AvatarImage;
    let person = |id: &str, initials: &str, color| OverlayPerson {
        id: id.into(),
        name: id.to_string().into(),
        initials: initials.to_string().into(),
        color,
        art: AvatarImage::default(),
        muted: false,
        level: 0.,
    };
    let (me, them) = (person("me", "A", 0x5865f2), person("them", "B", 0xeb459e));
    let frame = |width: u32, height: u32| {
        let pixels = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 90, 40, 255]));
        std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(pixels)]))
    };
    let stream = |width, height| Some(OverlayStream { owner: "me".into(), frame: frame(width, height) });
    let steps: Vec<(&str, Vec<OverlayPerson>, Option<OverlayStream>)> = vec![
        ("alone", vec![me.clone()], None),
        ("partner joins", vec![me.clone(), them.clone()], None),
        ("share 1920x1080", vec![me.clone(), them.clone()], stream(1920, 1080)),
        ("share 800x600", vec![me.clone(), them.clone()], stream(800, 600)),
        ("share 200x100", vec![me.clone(), them.clone()], stream(200, 100)),
        ("share stops", vec![me.clone(), them.clone()], None),
        ("partner leaves", vec![me], None),
    ];
    cx.spawn(async move |cx| {
        let mut handle = None;
        let mut stale = 0;
        for (name, people, shared) in steps {
            let _ = cx.update(|cx| sync(&mut handle, Some(people), 1., true, shared, cx));
            cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
            let Some(hud) = handle else { continue };
            let seen = hud.update(cx, |_, window, _| (window.viewport_size(), window.bounds().size));
            if let Ok((viewport, client)) = seen {
                let matches = viewport == client;
                stale += usize::from(!matches);
                println!("{name}: drawing area {viewport:?}, window {client:?}, {}", if matches { "ok" } else { "STALE" });
            }
        }
        println!("{}", if stale == 0 { "PASS" } else { "FAIL" });
        let _ = cx.update(|cx| cx.quit());
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::widescreen_crop;

    #[test]
    fn frames_crop_to_the_centre_widescreen_shape() {
        assert_eq!(widescreen_crop(1920, 1080), (1920, 1080));
        assert_eq!(widescreen_crop(1366, 768), (1366, 768));
        assert_eq!(widescreen_crop(800, 600), (800, 450));
        assert_eq!(widescreen_crop(2560, 1080), (1920, 1080));
        assert_eq!(widescreen_crop(100, 300), (100, 56));
    }
}
