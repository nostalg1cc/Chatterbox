use tauri::Manager;

#[cfg(target_os = "windows")]
fn apply_native_corner_preference(window: &tauri::WebviewWindow) {
    use std::ffi::c_void;
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUNDSMALL,
    };

    if let Ok(hwnd) = window.hwnd() {
        let preference = DWMWCP_ROUNDSMALL;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
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
        .invoke_handler(tauri::generate_handler![set_window_material])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
