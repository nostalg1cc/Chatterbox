use tauri::Manager;

/// Whether Mica was successfully applied to the main window at startup.
/// The frontend queries this to decide between a transparent (Mica) or solid background.
struct MicaState(bool);

#[tauri::command]
fn is_mica_supported(state: tauri::State<'_, MicaState>) -> bool {
    state.0
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let window = app
                .get_webview_window("main")
                .expect("main window should exist");

            #[cfg(target_os = "windows")]
            let mica_applied = {
                // Dark Mica; fails on Windows 10 and earlier, where the frontend
                // falls back to a solid background via the .no-mica class.
                let result = window_vibrancy::apply_mica(&window, Some(true));
                if let Err(ref err) = result {
                    log::warn!("Mica not applied: {err}");
                }
                result.is_ok()
            };
            #[cfg(not(target_os = "windows"))]
            let mica_applied = false;

            let _ = &window;
            app.manage(MicaState(mica_applied));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![is_mica_supported])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
