// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Hexagonal layers (see docs/hexagonal.md). The root holds only the entry point,
// the composition root, the layers, and the out-of-hex tool boundary.
mod application;
mod compose;
mod domain;
mod infrastructure;
mod tools;

use infrastructure::bot;
use infrastructure::config;
use infrastructure::driven::gdrive;
use infrastructure::driving::control_api;
use infrastructure::driving::os;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(bot::BotManager::new())
        .setup(|app| {
            // Shared infrastructure first (HTTP client, later logger/fs).
            compose::commons::init();
            #[cfg(target_os = "macos")]
            os::macos::intercept_quit_apple_event();
            // Migrate the old single config into { global, bots } if needed, so
            // the UI reads the new shape.
            let _ = config::load_bots(app.handle());
            os::tray::create_tray(app.handle())?;
            // Localhost control API (list/start/stop bots).
            control_api::start(app.handle().clone());
            Ok(())
        })
        // Closing the window hides it (and drops the dock icon) instead of
        // quitting, so the app keeps running from the tray.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                os::window::hide_window(window.app_handle());
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            os::window::hide_main_window,
            os::window::show_main_window,
            bot::start_bot,
            bot::stop_bot,
            bot::restart_bot,
            bot::get_running_bots,
            bot::resolve_tool_approval,
            gdrive::connect_drive,
            gdrive::drive_status,
            infrastructure::driving::memory::get_memories,
            infrastructure::driving::memory::delete_memory,
            infrastructure::driving::memory::clear_memories
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
