use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[cfg(target_os = "windows")]
fn apply_native_corner_preference(window: &tauri::WebviewWindow) {
    use std::ffi::c_void;
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };

    if let Ok(hwnd) = window.hwnd() {
        // DWMWCP_ROUND matches the standard ~10px radius native Windows 11
        // apps get; DWMWCP_ROUNDSMALL (the previous value) is a much subtler
        // radius that reads as sharp-cornered at a glance.
        let preference = DWMWCP_ROUND;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd.0 as _,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &preference as *const _ as *const c_void,
                size_of_val(&preference) as u32,
            );
        }
    }
}

#[tauri::command]
fn set_window_material(
    app: tauri::AppHandle,
    material: String,
    acrylic_dim: u8,
) -> Result<bool, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window is unavailable".to_string())?;

    #[cfg(target_os = "windows")]
    {
        let _ = window_vibrancy::clear_mica(&window);
        let _ = window_vibrancy::clear_acrylic(&window);
        let result = if material == "acrylic" {
            window_vibrancy::apply_acrylic(
                &window,
                Some((
                    0,
                    0,
                    0,
                    ((u16::from(acrylic_dim.min(100)) * 255) / 100) as u8,
                )),
            )
        } else {
            window_vibrancy::apply_mica(&window, Some(true))
        };
        return result.map(|_| true).map_err(|error| error.to_string());
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = material;
        Ok(false)
    }
}

struct GlobalVoiceShortcuts(Mutex<Vec<String>>);

fn clear_global_voice_shortcuts(app: &tauri::AppHandle) -> Result<(), String> {
    let previous = {
        let state = app.state::<GlobalVoiceShortcuts>();
        let mut shortcuts = state.0.lock().map_err(|_| "Global shortcut state is unavailable.".to_string())?;
        std::mem::take(&mut *shortcuts)
    };
    if !previous.is_empty() {
        app.global_shortcut()
            .unregister_multiple(previous.iter().map(String::as_str))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn register_global_voice_shortcut(app: &tauri::AppHandle, shortcut: &str, action: &str) -> Result<(), String> {
    let action = action.to_string();
    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = app.emit("dislight:global-voice-shortcut", action.clone());
            }
        })
        .map_err(|error| error.to_string())
}

// Returns the action names ("mute"/"deafen") that could NOT be registered
// (e.g. already claimed by another app) - anything not in that list is live.
// A single conflicting shortcut must not take the other one down with it:
// this used to abort and unregister everything on the first failure, so one
// bad binding silently killed every global shortcut, including ones that had
// already registered fine.
#[tauri::command]
fn configure_global_voice_shortcuts(
    app: tauri::AppHandle,
    enabled: bool,
    mute_shortcut: Option<String>,
    deafen_shortcut: Option<String>,
) -> Result<Vec<String>, String> {
    clear_global_voice_shortcuts(&app)?;
    if !enabled {
        return Ok(Vec::new());
    }

    let mut registered = Vec::new();
    let mut failed = Vec::new();
    let requested = [
        (mute_shortcut.filter(|shortcut| !shortcut.trim().is_empty()), "mute"),
        (deafen_shortcut.filter(|shortcut| !shortcut.trim().is_empty()), "deafen"),
    ];
    for (shortcut, action) in requested {
        let Some(shortcut) = shortcut else { continue };
        if registered.iter().any(|existing: &String| existing.eq_ignore_ascii_case(&shortcut)) {
            failed.push(action.to_string());
            continue;
        }
        if register_global_voice_shortcut(&app, &shortcut, action).is_err() {
            failed.push(action.to_string());
            continue;
        }
        registered.push(shortcut);
    }

    let state = app.state::<GlobalVoiceShortcuts>();
    let mut shortcuts = state.0.lock().map_err(|_| "Global shortcut state is unavailable.".to_string())?;
    *shortcuts = registered;
    Ok(failed)
}

// Guards the restart carried out from RunEvent::Exit below - see restart_app.
struct RestartRequested(AtomicBool);

#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    // NOT app.restart() - that's a known race (tauri-apps/tauri#11392):
    // it fires RunEvent::ExitRequested and spawns the replacement process
    // on a separate path from the one tearing the event loop down, and on
    // some machines the teardown wins, killing the process before the new
    // one finishes spawning. From the outside that's indistinguishable
    // from the app just closing and never coming back - exactly the "have
    // to relaunch it myself two or three times" symptom this was hit for.
    // Flagging intent and doing the actual restart from RunEvent::Exit
    // instead (after the event loop has fully wound down, so there's
    // nothing left to race against) is the fix multiple people confirmed
    // in that same thread.
    let state = app.state::<RestartRequested>();
    state.0.store(true, Ordering::SeqCst);
    app.exit(0);
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // WebView2 shares Chromium's GPU hardware blocklist, and some
    // GPU/driver combinations get silently forced into software rendering
    // as a result - which breaks CSS backdrop-filter (it needs GPU
    // compositing) with no visible error or crash, and can differ between
    // a `cargo run` dev process and a freshly-installed exe hitting the
    // blocklist differently on the same machine. This is Microsoft's own
    // documented workaround for it (WebView2Feedback#1469). Must be set
    // before the webview environment is created, so this runs first thing.
    #[cfg(target_os = "windows")]
    unsafe {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--ignore-gpu-blocklist --ignore-gpu-blacklist",
        );
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            app.manage(GlobalVoiceShortcuts(Mutex::new(Vec::new())));
            app.manage(RestartRequested(AtomicBool::new(false)));
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                apply_native_corner_preference(&window);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![set_window_material, configure_global_voice_shortcuts, restart_app])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            let state = app_handle.state::<RestartRequested>();
            if state.0.load(Ordering::SeqCst) {
                app_handle.cleanup_before_exit();
                tauri::process::restart(&app_handle.env());
            }
        }
    });
}
