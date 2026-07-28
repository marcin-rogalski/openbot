// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod audio;
mod bot;
mod config;
mod discord;
mod gdrive;
mod ingest;
mod knowledge;
#[cfg(target_os = "macos")]
mod macos;
mod memory;
mod model;
mod tools;
mod tray;
mod voice;
mod websearch;
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
            // Migrate the old single config into { global, bots } if needed, so
            // the UI reads the new shape.
            let _ = config::load_bots(app.handle());
            tray::create_tray(app.handle())?;
            // Localhost control API (list/start/stop bots) — see api.rs.
            api::start(app.handle().clone());
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
            bot::restart_bot,
            bot::get_running_bots,
            bot::resolve_tool_approval,
            gdrive::connect_drive,
            gdrive::drive_status,
            memory::get_memories,
            memory::delete_memory,
            memory::clear_memories
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
