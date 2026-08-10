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

#[tauri::command]
fn configure_global_voice_shortcuts(
    app: tauri::AppHandle,
    enabled: bool,
    mute_shortcut: Option<String>,
    deafen_shortcut: Option<String>,
) -> Result<(), String> {
    clear_global_voice_shortcuts(&app)?;
    if !enabled {
        return Ok(());
    }

    let mut registered = Vec::new();
    let requested = [
        (mute_shortcut.filter(|shortcut| !shortcut.trim().is_empty()), "mute"),
        (deafen_shortcut.filter(|shortcut| !shortcut.trim().is_empty()), "deafen"),
    ];
    for (shortcut, action) in requested {
        let Some(shortcut) = shortcut else { continue };
        if registered.iter().any(|existing: &String| existing.eq_ignore_ascii_case(&shortcut)) {
            let _ = app.global_shortcut().unregister_multiple(registered.iter().map(String::as_str));
            return Err("Mute and deafen cannot use the same global shortcut.".to_string());
        }
        if let Err(error) = register_global_voice_shortcut(&app, &shortcut, action) {
            let _ = app.global_shortcut().unregister_multiple(registered.iter().map(String::as_str));
            return Err(format!("Could not register {shortcut}: {error}"));
        }
        registered.push(shortcut);
    }

    let state = app.state::<GlobalVoiceShortcuts>();
    let mut shortcuts = state.0.lock().map_err(|_| "Global shortcut state is unavailable.".to_string())?;
    *shortcuts = registered;
    Ok(())
}

#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    app.restart();
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            app.manage(GlobalVoiceShortcuts(Mutex::new(Vec::new())));
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
                if let Err(error) = window_vibrancy::apply_mica(&window, Some(true)) {
                    log::warn!("Mica not applied: {error}");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![set_window_material, configure_global_voice_shortcuts, restart_app])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
