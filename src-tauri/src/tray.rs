use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

use crate::bot::{self, BotManager};
use crate::window::show_window;

/// Builds the menu-bar tray icon: left-click opens the main window, right-click
/// shows a menu with Start/Stop (kept in sync with the bot's running state) and
/// Quit. The Start/Stop items are handed to [`BotManager`] so it can enable or
/// disable them as the state changes.
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let start = MenuItem::with_id(app, "start", "Start", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "Stop", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&start, &stop, &separator, &quit])?;

    // Register the items with the bot manager; it sets their initial enabled
    // state and keeps them in sync from here on.
    app.state::<BotManager>()
        .set_tray_items(start.clone(), stop.clone());

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        // Not rendered as a template image: template icons can go blank on
        // hover once the app has no dock tile (Accessory activation policy),
        // which is exactly our state whenever the window is hidden.
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "start" => bot::set_running(app, true),
            "stop" => bot::set_running(app, false),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}
