// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;
mod window;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            tray::create_tray(app.handle())?;
            Ok(())
        })
        // Closing the window hides it instead of quitting the whole app,
        // since the app is meant to keep running from the tray.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            window::hide_main_window,
            window::show_main_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
