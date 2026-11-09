//! The sound wheel: press a key anywhere during a call, move the mouse towards a sound, let go
//! to play it. It's drawn in its own overlay window, so it shows over games, and it reads the
//! key and the mouse system-wide, as a game usually holds the cursor still and has the focus.
use crate::{Preview, components, state::SoundClip};
use gpui::{
    App, AppContext, Bounds, Context, FontWeight, IntoElement, PathBuilder, Render, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, canvas, div, point, prelude::*, px, rgb, rgba, size, svg,
};
use std::time::{Duration, Instant};

pub const SLOTS: usize = 8;
/// How far the mouse has to move before it's pointing at anything.
const DEAD_ZONE: f32 = 26.;
/// How far the pointing can build up, so turning to another direction doesn't take long.
const LONGEST_AIM: f32 = 120.;
/// Letting go sooner than this, without pointing at anything, leaves the wheel open.
const TAP: Duration = Duration::from_millis(350);
const PAGE_TURN: Duration = Duration::from_millis(160);
const OUTER: f32 = 236.;
const INNER: f32 = 100.;
const LABEL_RADIUS: f32 = 166.;
const WINDOW_SIZE: f32 = 540.;

pub struct SoundWheel {
    page: usize,
    aim: (f32, f32),
    selected: Option<usize>,
    opened_at: Instant,
    /// Tapped open, so it waits for a click or a second press.
    tapped: bool,
    page_turned_at: Option<Instant>,
    /// Held open by the development tools, which have no key to hold down.
    pub stay_open: bool,
}

/// Which of the eight directions this is, clockwise from straight up.
pub fn sector(dx: f32, dy: f32) -> Option<usize> {
    if dx.hypot(dy) < DEAD_ZONE {
        return None;
    }
    let turn = std::f32::consts::TAU;
    let from_up = dx.atan2(-dy).rem_euclid(turn);
    Some((from_up / (turn / SLOTS as f32)).round() as usize % SLOTS)
}

/// Adds some mouse movement to where the wheel is being pointed.
fn add_aim(aim: (f32, f32), dx: f32, dy: f32) -> (f32, f32) {
    let (x, y) = (aim.0 + dx, aim.1 + dy);
    let length = x.hypot(y);
    if length > LONGEST_AIM { (x / length * LONGEST_AIM, y / length * LONGEST_AIM) } else { (x, y) }
}

/// Favourites first, in the order they were starred, then the rest as they are.
pub fn ordered<'a>(clips: &'a [SoundClip], favourites: &[String]) -> Vec<&'a SoundClip> {
    let mut list: Vec<&SoundClip> = clips.iter().collect();
    list.sort_by_key(|clip| favourites.iter().position(|id| *id == clip.id).unwrap_or(usize::MAX));
    list
}

pub fn page_count(sounds: usize) -> usize {
    sounds.div_ceil(SLOTS).max(1)
}

#[derive(Clone, Debug, PartialEq)]
struct Slot {
    name: String,
    duration: String,
    starred: bool,
}

/// Everything the overlay draws.
#[derive(Clone, Debug, PartialEq)]
pub struct WheelView {
    slots: Vec<Slot>,
    selected: Option<usize>,
    title: String,
    hint: String,
    accent: u32,
}

pub struct WheelOverlay {
    view: Option<WheelView>,
}

impl Render for WheelOverlay {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full().flex().items_center().justify_center();
        match self.view.clone() {
            // The window has nothing behind it to look through, so no glass here.
            Some(view) => root.child(components::with_glass("sound-wheel-disc", false, move || wheel_disc(&view))),
            None => root,
        }
    }
}

fn wheel_disc(view: &WheelView) -> gpui::Div {
    let (selected, filled, accent) = (view.selected, view.slots.len(), view.accent);
    let wedges = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let centre = bounds.center();
            let spot = |radius: f32, angle: f32| centre + point(px(radius * angle.sin()), px(-radius * angle.cos()));
            for slot in 0..SLOTS {
                let middle = slot as f32 * std::f32::consts::TAU / SLOTS as f32;
                let half = std::f32::consts::PI / SLOTS as f32 - 0.022;
                const STEPS: usize = 14;
                let along = |step: usize| middle - half + 2. * half * step as f32 / STEPS as f32;
                let mut path = PathBuilder::fill();
                path.move_to(spot(OUTER - 8., along(0)));
                (1..=STEPS).for_each(|step| path.line_to(spot(OUTER - 8., along(step))));
                (0..=STEPS).rev().for_each(|step| path.line_to(spot(INNER, along(step))));
                path.close();
                let colour = if selected == Some(slot) && slot < filled {
                    rgba((accent << 8) | 0xe6)
                } else if slot < filled {
                    rgba(0xffffff14)
                } else {
                    rgba(0xffffff06)
                };
                if let Ok(path) = path.build() {
                    window.paint_path(path, colour);
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();

    let mut disc = div()
        .relative()
        .size(px(OUTER * 2.))
        .flex_shrink_0()
        .child(
            components::material_surface(components::MATERIAL_PANEL, OUTER, components::StrokeKind::Neutral)
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
        )
        .child(wedges);

    for (index, slot) in view.slots.iter().enumerate() {
        let angle = index as f32 * std::f32::consts::TAU / SLOTS as f32;
        disc = disc.child(
            div()
                .absolute()
                .left(px(OUTER + LABEL_RADIUS * angle.sin() - 56.))
                .top(px(OUTER - LABEL_RADIUS * angle.cos() - 30.))
                .w(px(112.))
                .h(px(60.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(2.))
                .text_color(rgb(0xffffff))
                .when(slot.starred, |label| label.child(svg().path("Star").size(px(11.)).text_color(rgb(0xf5c84b))))
                .child(
                    div()
                        .w_full()
                        .text_center()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(slot.name.clone()),
                )
                .child(div().text_size(px(10.5)).text_color(rgba(0xffffffb0)).child(slot.duration.clone())),
        );
    }

    disc.child(
        components::material_surface(components::MATERIAL_DARK, INNER - 8., components::StrokeKind::Neutral)
            .absolute()
            .left(px(OUTER - INNER + 8.))
            .top(px(OUTER - INNER + 8.))
            .size(px((INNER - 8.) * 2.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .px(px(18.))
            .text_color(rgb(0xffffff))
            .child(div().w_full().text_center().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).truncate().child(view.title.clone()))
            .child(div().text_size(px(11.)).text_color(rgba(0xffffffb0)).child(view.hint.clone())),
    )
}

fn open_overlay(cx: &mut App) -> Option<WindowHandle<WheelOverlay>> {
    let display = cx.primary_display()?;
    let screen = display.bounds();
    let extent = size(px(WINDOW_SIZE), px(WINDOW_SIZE));
    let bounds = Bounds::new(
        point(
            screen.origin.x + (screen.size.width - extent.width) * 0.5,
            screen.origin.y + (screen.size.height - extent.height) * 0.5,
        ),
        extent,
    );
    let handle = cx
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
            |_, cx| cx.new(|_| WheelOverlay { view: None }),
        )
        .ok()?;
    let _ = handle.update(cx, |_, window, cx| {
        crate::hud::make_passive(window);
        crate::hud::position(window, bounds, cx);
    });
    Some(handle)
}

impl Preview {
    fn close_wheel(&mut self) {
        self.sound_wheel = None;
        self.wheel_press = None;
    }

    pub fn open_wheel(&mut self, cx: &mut Context<Self>) {
        self.load_soundboard(cx);
        self.wheel_press = None;
        self.sound_wheel = Some(SoundWheel {
            page: 0,
            aim: (0., 0.),
            selected: None,
            opened_at: Instant::now(),
            tapped: false,
            page_turned_at: None,
            stay_open: false,
        });
    }

    /// Plays what's pointed at, if anything, and closes.
    fn finish_wheel(&mut self, cx: &mut Context<Self>) {
        let Some(wheel) = self.sound_wheel.take() else { return };
        let Some(slot) = wheel.selected else { return };
        let id = ordered(&self.soundboard_clips, &self.state.preferences.pinned_sound_ids)
            .get(wheel.page * SLOTS + slot)
            .map(|clip| clip.id.clone());
        if let Some(id) = id {
            self.play_clip_in_call(id, cx);
        }
    }

    fn wheel_pressed(&mut self, cx: &mut Context<Self>) {
        if let Some(wheel) = &self.sound_wheel {
            if wheel.tapped {
                self.finish_wheel(cx);
            }
            return;
        }
        // A plain letter has to reach a box in Nitro. Other apps can't be told apart like this.
        let bare = !self.state.preferences.wheel_key.contains('-');
        if bare && self.window_active && self.typing_in_nitro {
            return;
        }
        if self.state.preferences.wheel_hold {
            self.wheel_press = Some(Instant::now());
        } else {
            self.open_wheel(cx);
        }
    }

    fn wheel_released(&mut self, cx: &mut Context<Self>) {
        if self.wheel_press.take().is_some() {
            return;
        }
        let hold_mode = self.state.preferences.wheel_hold;
        let Some(wheel) = self.sound_wheel.as_mut() else { return };
        if wheel.tapped {
            return;
        }
        if wheel.selected.is_some() {
            self.finish_wheel(cx);
        } else if !hold_mode && wheel.opened_at.elapsed() < TAP {
            wheel.tapped = true;
        } else {
            self.close_wheel();
        }
    }

    pub fn wheel_aim_by(&mut self, dx: f32, dy: f32) {
        let Some(wheel) = self.sound_wheel.as_mut() else { return };
        wheel.aim = add_aim(wheel.aim, dx, dy);
        wheel.selected = sector(wheel.aim.0, wheel.aim.1);
    }

    pub fn wheel_turn_page(&mut self, forward: bool) {
        let pages = page_count(self.soundboard_clips.len());
        let Some(wheel) = self.sound_wheel.as_mut() else { return };
        if wheel.page_turned_at.is_some_and(|at| at.elapsed() < PAGE_TURN) {
            return;
        }
        wheel.page_turned_at = Some(Instant::now());
        wheel.page = if forward { (wheel.page + 1) % pages } else { (wheel.page + pages - 1) % pages };
    }

    fn wheel_view(&self) -> Option<WheelView> {
        let wheel = self.sound_wheel.as_ref()?;
        let favourites = &self.state.preferences.pinned_sound_ids;
        let list = ordered(&self.soundboard_clips, favourites);
        let pages = page_count(list.len());
        let slots: Vec<Slot> = list
            .iter()
            .skip(wheel.page * SLOTS)
            .take(SLOTS)
            .map(|clip| Slot {
                name: clip.name.clone(),
                duration: clip.duration_label.clone(),
                starred: favourites.contains(&clip.id),
            })
            .collect();
        let title = wheel
            .selected
            .and_then(|slot| slots.get(slot))
            .map(|slot| slot.name.clone())
            .unwrap_or_else(|| match &self.soundboard_status {
                crate::SoundboardStatus::Loaded if list.is_empty() => "No sounds yet".to_string(),
                crate::SoundboardStatus::Loaded => "Move to pick".to_string(),
                crate::SoundboardStatus::Failed(_) => "Sounds didn't load".to_string(),
                _ => "Loading sounds".to_string(),
            });
        let hint = if pages > 1 { format!("Page {} of {pages} · scroll", wheel.page + 1) } else { "Let go to play".to_string() };
        Some(WheelView { slots, selected: wheel.selected, title, hint, accent: crate::state::palette().accent })
    }

    /// Runs the wheel: reads the key and mouse, and keeps the overlay up to date.
    pub fn pump_wheel(&mut self, cx: &mut Context<Self>) {
        let held_open = self.sound_wheel.as_ref().is_some_and(|wheel| wheel.stay_open);
        if !held_open && !(self.call_connected && self.state.preferences.wheel_enabled) {
            self.close_wheel();
            self.wheel_input = None;
            self.wheel_view_shown = None;
            if let Some(overlay) = self.wheel_overlay.take() {
                let _ = overlay.update(cx, |_, window, _| window.remove_window());
            }
            return;
        }
        let input = self.wheel_input.get_or_insert_with(input::WheelInput::start);
        input.set_key(crate::hotkeys::parse_any(&self.state.preferences.wheel_key));
        input.set_armed(self.recording_keybind.is_none());
        let events = input.take();
        for event in events {
            match event {
                input::Event::Pressed => self.wheel_pressed(cx),
                input::Event::Released => self.wheel_released(cx),
                input::Event::Escape => self.close_wheel(),
                input::Event::Click => {
                    if self.sound_wheel.as_ref().is_some_and(|wheel| wheel.tapped) {
                        self.finish_wheel(cx);
                    }
                }
                input::Event::Moved(dx, dy) => self.wheel_aim_by(dx, dy),
                input::Event::Scrolled(up) => self.wheel_turn_page(!up),
            }
        }
        let hold = Duration::from_millis(u64::from(self.state.preferences.wheel_hold_tenths.max(1)) * 100);
        if self.wheel_press.is_some_and(|since| since.elapsed() >= hold) {
            self.open_wheel(cx);
        }
        if let Some(input) = &self.wheel_input {
            input.set_open(self.sound_wheel.is_some());
        }
        if self.wheel_overlay.is_none() {
            self.wheel_overlay = open_overlay(cx);
        }
        let view = self.wheel_view();
        if view == self.wheel_view_shown {
            return;
        }
        let appearing = self.wheel_view_shown.is_none();
        self.wheel_view_shown = view.clone();
        let centre_cursor = self.window_active && !held_open;
        if let Some(overlay) = self.wheel_overlay {
            let _ = overlay.update(cx, |overlay, window, cx| {
                if appearing {
                    input::centre_on_active_screen(window, centre_cursor, cx);
                }
                overlay.view = view;
                cx.notify();
            });
        }
    }
}

mod input {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };

    pub enum Event {
        Pressed,
        Released,
        Escape,
        Click,
        Moved(f32, f32),
        Scrolled(bool),
    }

    #[derive(Default)]
    struct Shared {
        key: Mutex<Option<(u32, u32)>>,
        armed: AtomicBool,
        open: AtomicBool,
        stop: AtomicBool,
    }

    /// A thread watching the key and the mouse. It only looks: games still get all of it.
    pub struct WheelInput {
        shared: Arc<Shared>,
        events: mpsc::Receiver<Event>,
    }

    impl WheelInput {
        pub fn start() -> Self {
            let shared = Arc::new(Shared::default());
            let (sender, events) = mpsc::channel();
            let thread_shared = shared.clone();
            std::thread::spawn(move || run(thread_shared, sender));
            Self { shared, events }
        }

        pub fn set_key(&self, key: Option<(u32, u32)>) {
            if let Ok(mut current) = self.shared.key.lock() {
                *current = key;
            }
        }

        pub fn set_armed(&self, armed: bool) {
            self.shared.armed.store(armed, Ordering::Relaxed);
        }

        pub fn set_open(&self, open: bool) {
            self.shared.open.store(open, Ordering::Relaxed);
        }

        pub fn take(&self) -> Vec<Event> {
            self.events.try_iter().collect()
        }
    }

    impl Drop for WheelInput {
        fn drop(&mut self) {
            self.shared.stop.store(true, Ordering::Relaxed);
        }
    }

    #[cfg(windows)]
    mod native {
        #[repr(C)]
        #[derive(Default)]
        pub struct Msg {
            pub hwnd: isize,
            pub message: u32,
            pub wparam: usize,
            pub lparam: isize,
            pub time: u32,
            pub x: i32,
            pub y: i32,
        }
        #[repr(C)]
        pub struct RawDevice {
            pub usage_page: u16,
            pub usage: u16,
            pub flags: u32,
            pub target: isize,
        }
        #[repr(C)]
        #[derive(Default)]
        pub struct RawHeader {
            pub kind: u32,
            pub size: u32,
            pub device: isize,
            pub wparam: usize,
        }
        #[repr(C)]
        #[derive(Default)]
        pub struct RawMouse {
            pub flags: u16,
            pub padding: u16,
            pub button_flags: u16,
            pub button_data: u16,
            pub raw_buttons: u32,
            pub x: i32,
            pub y: i32,
            pub extra: u32,
        }
        #[repr(C)]
        #[derive(Default)]
        pub struct RawInput {
            pub header: RawHeader,
            pub mouse: RawMouse,
        }
        #[repr(C)]
        #[derive(Default)]
        pub struct MonitorInfo {
            pub size: u32,
            pub monitor: [i32; 4],
            pub work: [i32; 4],
            pub flags: u32,
        }
        #[link(name = "user32")]
        unsafe extern "system" {
            pub fn CreateWindowExW(
                ex_style: u32,
                class: *const u16,
                name: *const u16,
                style: u32,
                x: i32,
                y: i32,
                width: i32,
                height: i32,
                parent: isize,
                menu: isize,
                instance: isize,
                param: *const std::ffi::c_void,
            ) -> isize;
            pub fn DestroyWindow(hwnd: isize) -> i32;
            pub fn RegisterRawInputDevices(devices: *const RawDevice, count: u32, size: u32) -> i32;
            pub fn GetRawInputData(input: isize, command: u32, data: *mut std::ffi::c_void, size: *mut u32, header: u32) -> u32;
            pub fn PeekMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32, remove: u32) -> i32;
            pub fn DispatchMessageW(msg: *const Msg) -> isize;
            pub fn GetAsyncKeyState(key: i32) -> i16;
            pub fn GetForegroundWindow() -> isize;
            pub fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
            pub fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
            pub fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, width: i32, height: i32, flags: u32) -> i32;
            pub fn SetCursorPos(x: i32, y: i32) -> i32;
        }
    }

    #[cfg(windows)]
    fn run(shared: Arc<Shared>, events: mpsc::Sender<Event>) {
        use native::*;
        const HWND_MESSAGE: isize = -3;
        const INPUT_SINK: u32 = 0x100;
        const WM_INPUT: u32 = 0x00ff;
        const RID_INPUT: u32 = 0x1000_0003;
        const WHEEL_MOVED: u16 = 0x400;
        const ABSOLUTE: u16 = 1;
        const ESCAPE: i32 = 0x1b;
        const LEFT_BUTTON: i32 = 0x01;
        let down = |key: i32| unsafe { GetAsyncKeyState(key) } as u16 & 0x8000 != 0;
        let modifiers = || {
            u32::from(down(0x12)) | u32::from(down(0x11)) << 1 | u32::from(down(0x10)) << 2 | u32::from(down(0x5b) || down(0x5c)) << 3
        };
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let window = unsafe {
            CreateWindowExW(0, class.as_ptr(), std::ptr::null(), 0, 0, 0, 0, 0, HWND_MESSAGE, 0, 0, std::ptr::null())
        };
        if window == 0 {
            return;
        }
        // Raw movement, as a game that holds the cursor in place leaves nothing else to read.
        let mouse = RawDevice { usage_page: 1, usage: 2, flags: INPUT_SINK, target: window };
        unsafe { RegisterRawInputDevices(&mouse, 1, size_of::<RawDevice>() as u32) };
        let (mut held, mut escape_down, mut click_down) = (false, false, false);
        while !shared.stop.load(Ordering::Relaxed) {
            let open = shared.open.load(Ordering::Relaxed);
            let (mut dx, mut dy) = (0i32, 0i32);
            let mut message = Msg::default();
            while unsafe { PeekMessageW(&mut message, window, 0, 0, 1) } != 0 {
                if message.message == WM_INPUT && open {
                    let mut data = RawInput::default();
                    let mut bytes = size_of::<RawInput>() as u32;
                    let read = unsafe {
                        GetRawInputData(message.lparam, RID_INPUT, (&mut data as *mut RawInput).cast(), &mut bytes, size_of::<RawHeader>() as u32)
                    };
                    if read != u32::MAX && data.header.kind == 0 {
                        if data.mouse.flags & ABSOLUTE == 0 {
                            dx += data.mouse.x;
                            dy += data.mouse.y;
                        }
                        if data.mouse.button_flags & WHEEL_MOVED != 0 {
                            let _ = events.send(Event::Scrolled(data.mouse.button_data as i16 > 0));
                        }
                    }
                }
                unsafe { DispatchMessageW(&message) };
            }
            if open && (dx != 0 || dy != 0) {
                let _ = events.send(Event::Moved(dx as f32, dy as f32));
            }
            let bind = shared.key.lock().ok().and_then(|key| *key);
            let key_down = bind.is_some_and(|(_, key)| down(key as i32));
            if held && !key_down {
                held = false;
                let _ = events.send(Event::Released);
            } else if !held && key_down && shared.armed.load(Ordering::Relaxed) && bind.is_some_and(|(wanted, _)| modifiers() == wanted) {
                held = true;
                let _ = events.send(Event::Pressed);
            }
            let (escape, click) = (down(ESCAPE), down(LEFT_BUTTON));
            if open && escape && !escape_down {
                let _ = events.send(Event::Escape);
            }
            if open && click && !click_down {
                let _ = events.send(Event::Click);
            }
            (escape_down, click_down) = (escape, click);
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        unsafe { DestroyWindow(window) };
    }

    #[cfg(not(windows))]
    fn run(_: Arc<Shared>, _: mpsc::Sender<Event>) {}

    /// Centres the overlay on the screen being used, which is where the game is.
    #[cfg(windows)]
    pub fn centre_on_active_screen(window: &gpui::Window, centre_cursor: bool, cx: &gpui::App) {
        use native::*;
        let Some(hwnd) = crate::hud::native::hwnd(window) else { return };
        let scale = window.scale_factor();
        let bounds = window.bounds();
        let (width, height) = (f32::from(bounds.size.width) * scale, f32::from(bounds.size.height) * scale);
        cx.foreground_executor()
            .spawn(async move {
                let mut info = MonitorInfo { size: size_of::<MonitorInfo>() as u32, ..Default::default() };
                let monitor = unsafe { MonitorFromWindow(GetForegroundWindow(), 1) };
                if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
                    return;
                }
                let [left, top, right, bottom] = info.monitor;
                let (x, y) = ((left + right) / 2, (top + bottom) / 2);
                unsafe {
                    SetWindowPos(hwnd, -1, x - (width / 2.) as i32, y - (height / 2.) as i32, 0, 0, 0x1 | 0x10 | 0x40);
                    if centre_cursor {
                        SetCursorPos(x, y);
                    }
                }
            })
            .detach();
    }

    #[cfg(not(windows))]
    pub fn centre_on_active_screen(_: &gpui::Window, _: bool, _: &gpui::App) {}
}

pub use input::WheelInput;

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(id: &str) -> SoundClip {
        SoundClip {
            id: id.into(),
            name: id.into(),
            duration_label: String::new(),
            owner_initials: String::new(),
            owner_name: String::new(),
            is_mine: true,
            duration_ms: 0,
            size_bytes: 0,
            owner_color: 0,
            owner_art: false.into(),
        }
    }

    #[test]
    fn each_direction_picks_its_own_slot() {
        assert_eq!(sector(0., -100.), Some(0));
        assert_eq!(sector(100., -100.), Some(1));
        assert_eq!(sector(100., 0.), Some(2));
        assert_eq!(sector(100., 100.), Some(3));
        assert_eq!(sector(0., 100.), Some(4));
        assert_eq!(sector(-100., 100.), Some(5));
        assert_eq!(sector(-100., 0.), Some(6));
        assert_eq!(sector(-100., -100.), Some(7));
        assert_eq!(sector(-5., -100.), Some(0));
        assert_eq!(sector(5., 5.), None);
    }

    #[test]
    fn aim_stops_building_up_so_a_change_of_direction_is_quick() {
        let far = add_aim((0., 0.), 10_000., 0.);
        assert_eq!(far, (LONGEST_AIM, 0.));
        let turned = add_aim(far, -LONGEST_AIM * 2., 0.);
        assert_eq!(sector(turned.0, turned.1), Some(6));
    }

    #[test]
    fn favourites_come_first_and_the_rest_keep_their_order() {
        let clips = [clip("a"), clip("b"), clip("c"), clip("d")];
        let favourites = ["c".to_string(), "b".to_string()];
        let names: Vec<_> = ordered(&clips, &favourites).iter().map(|clip| clip.id.as_str()).collect();
        assert_eq!(names, ["c", "b", "a", "d"]);
    }

    #[test]
    fn pages_hold_eight_and_there_is_always_one() {
        assert_eq!(page_count(0), 1);
        assert_eq!(page_count(8), 1);
        assert_eq!(page_count(9), 2);
    }
}
