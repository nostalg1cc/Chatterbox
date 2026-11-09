//! System-wide call shortcuts. Only registered during a call, so they never take a key
//! from another app the rest of the time.
use std::{sync::mpsc, time::Duration};

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
    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn RegisterHotKey(hwnd: isize, id: i32, modifiers: u32, key: u32) -> i32;
        pub fn UnregisterHotKey(hwnd: isize, id: i32) -> i32;
        pub fn PeekMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32, remove: u32) -> i32;
    }
    pub const WM_HOTKEY: u32 = 0x0312;
    pub const PM_REMOVE: u32 = 1;
    pub const MOD_ALT: u32 = 1;
    pub const MOD_CONTROL: u32 = 2;
    pub const MOD_SHIFT: u32 = 4;
    pub const MOD_WIN: u32 = 8;
    pub const MOD_NOREPEAT: u32 = 0x4000;
}

/// A GPUI keystroke such as `ctrl-shift-m` as Windows modifiers and a virtual key.
/// `None` for keys with no stable virtual key, or with no modifier: a bare letter
/// would stop that letter being typed anywhere on the computer.
fn parse(keystroke: &str) -> Option<(u32, u32)> {
    let (modifiers, code) = parse_any(keystroke)?;
    let function_key = (0x70..=0x87).contains(&code);
    (modifiers != 0 || function_key).then_some((modifiers, code))
}

/// The same, allowing a key on its own, for something that only watches the key.
pub fn parse_any(keystroke: &str) -> Option<(u32, u32)> {
    const CTRL: u32 = 2;
    const ALT: u32 = 1;
    const SHIFT: u32 = 4;
    const WIN: u32 = 8;
    let mut modifiers = 0;
    let mut key = None;
    for part in keystroke.split('-') {
        match part {
            "ctrl" => modifiers |= CTRL,
            "alt" => modifiers |= ALT,
            "shift" => modifiers |= SHIFT,
            "cmd" | "win" | "super" | "platform" => modifiers |= WIN,
            other => key = Some(other),
        }
    }
    let key = key?;
    let code = match key {
        k if k.len() == 1 => {
            let c = k.chars().next()?.to_ascii_uppercase();
            if c.is_ascii_alphanumeric() { c as u32 } else { return None }
        }
        "space" => 0x20,
        "enter" => 0x0d,
        "tab" => 0x09,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "insert" => 0x2d,
        "delete" => 0x2e,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        f if f.starts_with('f') => {
            let number: u32 = f[1..].parse().ok()?;
            if (1..=24).contains(&number) { 0x6f + number } else { return None }
        }
        _ => return None,
    };
    Some((modifiers, code))
}

pub struct Hotkeys {
    commands: mpsc::Sender<Vec<String>>,
    presses: mpsc::Receiver<usize>,
    current: Vec<String>,
}

impl Hotkeys {
    /// Stops, releasing its keys, once this is dropped.
    pub fn start() -> Self {
        let (commands, command_rx) = mpsc::channel();
        let (press_tx, presses) = mpsc::channel();
        std::thread::spawn(move || run(command_rx, press_tx));
        Self { commands, presses, current: Vec::new() }
    }

    pub fn set(&mut self, binds: &[String]) {
        if self.current != binds {
            self.current = binds.to_vec();
            let _ = self.commands.send(self.current.clone());
        }
    }

    /// Indexes of the actions pressed since the last call.
    pub fn take(&self) -> Vec<usize> {
        self.presses.try_iter().collect()
    }
}

#[cfg(windows)]
fn run(commands: mpsc::Receiver<Vec<String>>, presses: mpsc::Sender<usize>) {
    use native::*;
    // Hotkeys belong to the thread registering them, so this thread does all of it.
    let mut registered: Vec<i32> = Vec::new();
    loop {
        match commands.try_recv() {
            Ok(binds) => {
                for id in registered.drain(..) {
                    unsafe { UnregisterHotKey(0, id) };
                }
                for (index, bind) in binds.iter().enumerate() {
                    if let Some((modifiers, key)) = parse(bind) {
                        // Another app may hold it. The shortcut then only works in Nitro.
                        let id = index as i32 + 1;
                        if unsafe { RegisterHotKey(0, id, modifiers | MOD_NOREPEAT, key) } != 0 {
                            registered.push(id);
                        }
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
        let mut message = Msg::default();
        while unsafe { PeekMessageW(&mut message, 0, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) } != 0 {
            if message.message == WM_HOTKEY && presses.send(message.wparam.saturating_sub(1)).is_err() {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    for id in registered {
        unsafe { UnregisterHotKey(0, id) };
    }
    let _ = (MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN);
}

#[cfg(not(windows))]
fn run(_: mpsc::Receiver<Vec<String>>, _: mpsc::Sender<usize>) {}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn keystrokes_become_windows_keys() {
        assert_eq!(parse("ctrl-shift-m"), Some((6, 0x4d)));
        assert_eq!(parse("alt-1"), Some((1, 0x31)));
        assert_eq!(parse("f9"), Some((0, 0x78)));
        assert_eq!(parse("ctrl-pageup"), Some((2, 0x21)));
    }

    #[test]
    fn a_bare_key_is_never_taken_from_the_whole_computer() {
        assert_eq!(parse("m"), None);
        assert_eq!(parse("space"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("ctrl-"), None);
    }
}
