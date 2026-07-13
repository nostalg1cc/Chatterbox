use tauri::Manager;

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
