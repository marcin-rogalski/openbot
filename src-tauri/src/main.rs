// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
mod macos;
mod bot;
mod config;
mod discord;
mod model;
mod tray;
mod window;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(bot::BotManager::new())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            macos::intercept_quit_apple_event();
            tray::create_tray(app.handle())?;
            Ok(())
        })
        // Closing the window hides it (and drops the dock icon) instead of
        // quitting, so the app keeps running from the tray.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                window::hide_window(window.app_handle());
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            window::hide_main_window,
            window::show_main_window,
            bot::start_bot,
            bot::stop_bot,
            bot::get_bot_status,
            bot::restart_bot
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
