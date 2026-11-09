//! The small window shown at launch: it installs any update first, then loads the app out of
//! sight and swaps over once there's something to show.
use crate::updater;
use gpui::{
    Animation, AnimationExt, App, AppContext, AsyncApp, Bounds, Context, ImageFormat, IntoElement, Render, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowHandle, WindowKind, WindowOptions, div, img, prelude::*, px, rgb, rgba, size,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

/// Long enough for a slow connection, short enough that being offline barely delays launch.
const CHECK_LIMIT: Duration = Duration::from_secs(5);
/// A download that hasn't moved in this long has stalled.
const STALL_LIMIT: Duration = Duration::from_secs(20);
/// However the app's loading is going, it's shown by now.
const LOAD_LIMIT: Duration = Duration::from_secs(12);
const LOGO_SIZE: f32 = 92.;
const RETRY_INSTALL_AFTER: Duration = Duration::from_secs(6 * 3600);

pub struct Startup {
    status: SharedString,
    /// How much of the download is done, 0 to 1.
    progress: Option<f32>,
    logo: Arc<gpui::Image>,
    background: u32,
}

impl Render for Startup {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let track = div().w(px(168.)).h(px(4.)).rounded_full().bg(rgba(0xffffff1f)).overflow_hidden();
        div()
            .size_full()
            .bg(rgb(self.background))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(26.))
            .font_family("Inter")
            .window_control_area(gpui::WindowControlArea::Drag)
            .child(
                img(self.logo.clone()).size(px(LOGO_SIZE)).with_animation(
                    "startup-logo",
                    Animation::new(Duration::from_millis(1100)).repeat().with_easing(gpui::pulsating_between(0.72, 1.)),
                    |logo, delta| logo.opacity(delta),
                ),
            )
            .child(
                div()
                    .h(px(30.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.))
                    .child(div().text_size(px(13.)).font_weight(gpui::FontWeight::MEDIUM).text_color(rgb(0xd4d4d8)).child(self.status.clone()))
                    .children(self.progress.map(|progress| {
                        track.child(div().h_full().rounded_full().bg(rgb(0xffffff)).w(gpui::relative(progress)))
                    })),
            )
    }
}

/// The logo at exactly the size it's shown, as a larger picture scaled down has jagged edges.
fn sharp_logo(theme: crate::themes::AppTheme, scale: f32) -> Arc<gpui::Image> {
    let mut png = Vec::new();
    if let Some(picture) = crate::app_icon::compose(theme, (LOGO_SIZE * scale).round() as u32) {
        let _ = picture.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png);
    }
    Arc::new(gpui::Image::from_bytes(ImageFormat::Png, png))
}

fn open_window(cx: &mut App) -> Option<WindowHandle<Startup>> {
    let saved = crate::state::load_state();
    let theme = saved.preferences.theme;
    let bounds = Bounds::centered(None, size(px(300.), px(340.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some("Nitro".into()), appears_transparent: true, ..Default::default() }),
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: false,
            is_minimizable: false,
            app_id: Some("io.github.nostalg1cc.dislight".into()),
            ..Default::default()
        },
        |window, cx| {
            let logo = sharp_logo(theme, window.scale_factor());
            cx.new(|_| Startup {
                status: "Starting…".into(),
                progress: None,
                logo,
                background: theme.stage_bg(),
            })
        },
    )
    .ok()
}

fn say(window: WindowHandle<Startup>, status: impl Into<SharedString>, progress: Option<f32>, cx: &mut AsyncApp) {
    let status = status.into();
    let _ = window.update(cx, |startup, _, cx| {
        startup.status = status;
        startup.progress = progress;
        cx.notify();
    });
}

fn attempt_marker(version: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("nitro-update-attempt-{version}"))
}

fn remember_attempt(version: &str) {
    let _ = std::fs::write(attempt_marker(version), b"");
}

/// Whether this version was installed moments ago and Nitro is somehow still the old one.
fn tried_recently(version: &str) -> bool {
    std::fs::metadata(attempt_marker(version))
        .and_then(|file| file.modified())
        .ok()
        .and_then(|at| at.elapsed().ok())
        .is_some_and(|since| since < RETRY_INSTALL_AFTER)
}

/// Installs an update if there is one. True means the installer is running and the app quits.
async fn update_first(window: WindowHandle<Startup>, cx: &mut AsyncApp) -> bool {
    say(window, "Checking for updates…", None, cx);
    let (found_tx, found) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = found_tx.send(updater::check());
    });
    let began = Instant::now();
    let update = loop {
        match found.try_recv() {
            Ok(Ok(Some(update))) => break update,
            Ok(_) | Err(mpsc::TryRecvError::Disconnected) => return false,
            Err(mpsc::TryRecvError::Empty) if began.elapsed() >= CHECK_LIMIT => return false,
            Err(mpsc::TryRecvError::Empty) => cx.background_executor().timer(Duration::from_millis(50)).await,
        }
    };
    // An installer that keeps failing must not keep Nitro from opening.
    if tried_recently(&update.version) {
        return false;
    }
    let progress = Arc::new(AtomicU32::new(0));
    let given_up = Arc::new(AtomicBool::new(false));
    let (done_tx, done) = mpsc::channel();
    let (thread_progress, thread_given_up) = (progress.clone(), given_up.clone());
    let version = update.version.clone();
    let attempt_version = update.version.clone();
    std::thread::spawn(move || {
        let result = updater::download(&update, thread_progress).and_then(|file| {
            // Launch has moved on without it, and an installer would close the running app.
            if thread_given_up.load(Ordering::Acquire) {
                return Err("Stalled".to_string());
            }
            remember_attempt(&attempt_version);
            updater::install(&file)
        });
        let _ = done_tx.send(result);
    });
    let (mut last, mut moved_at) = (0, Instant::now());
    loop {
        match done.try_recv() {
            Ok(Ok(())) => {
                say(window, "Restarting…", Some(1.), cx);
                return true;
            }
            Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => return false,
            Err(mpsc::TryRecvError::Empty) => {}
        }
        let now = progress.load(Ordering::Relaxed);
        if now != last {
            (last, moved_at) = (now, Instant::now());
        }
        if moved_at.elapsed() >= STALL_LIMIT {
            given_up.store(true, Ordering::Release);
            return false;
        }
        say(window, format!("Updating to {version}… {}%", now / 10), Some(now as f32 / 1000.), cx);
        cx.background_executor().timer(Duration::from_millis(80)).await;
    }
}

/// Shows the startup window and carries on from there.
pub fn launch(cx: &mut App) {
    let Some(window) = open_window(cx) else {
        crate::open_main_window(cx, true);
        return;
    };
    cx.activate(true);
    cx.spawn(async move |cx| {
        // Development builds are never replaced by a release.
        if !cfg!(debug_assertions) && update_first(window, cx).await {
            let _ = cx.update(|cx| cx.quit());
            return;
        }
        say(window, "Starting…", None, cx);
        let Ok((main, preview)) = cx.update(|cx| crate::open_main_window(cx, false)) else { return };
        let began = Instant::now();
        while began.elapsed() < LOAD_LIMIT {
            if preview.read_with(cx, |preview, _| preview.startup_ready()).unwrap_or(true) {
                break;
            }
            cx.background_executor().timer(Duration::from_millis(60)).await;
        }
        let _ = main.update(cx, |_, window, _| window.activate_window());
        // A moment for the main window to appear, so the desktop doesn't show between them.
        cx.background_executor().timer(Duration::from_millis(120)).await;
        let _ = window.update(cx, |_, window, _| window.remove_window());
    })
    .detach();
}

/// Keeps a second copy from starting. It would fight over the call, and its auto-update would
/// close the one already running. Brings that one to the front instead.
#[cfg(windows)]
pub fn another_copy_is_running() -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateMutexW(attributes: *const std::ffi::c_void, owner: i32, name: *const u16) -> isize;
        fn GetLastError() -> u32;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> isize;
        fn ShowWindow(hwnd: isize, command: i32) -> i32;
        fn IsIconic(hwnd: isize) -> i32;
        fn SetForegroundWindow(hwnd: isize) -> i32;
    }
    const ALREADY_EXISTS: u32 = 183;
    let name = if cfg!(debug_assertions) { "Local\\NitroHeroDevelopment\0" } else { "Local\\NitroHero\0" };
    let name: Vec<u16> = name.encode_utf16().collect();
    // Never closed: Windows releases it when the app ends, for any reason.
    let held = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if held == 0 || unsafe { GetLastError() } != ALREADY_EXISTS {
        return false;
    }
    let title: Vec<u16> = "Nitro\0".encode_utf16().collect();
    let window = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if window != 0 {
        unsafe {
            if IsIconic(window) != 0 {
                ShowWindow(window, 9);
            }
            SetForegroundWindow(window);
        }
    }
    true
}

#[cfg(not(windows))]
pub fn another_copy_is_running() -> bool {
    false
}
