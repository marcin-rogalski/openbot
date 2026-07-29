use tauri::{AppHandle, Manager, Runtime};

/// Label of the window declared in tauri.conf.json. Kept in one place so
/// tray.rs and main.rs never hardcode the string separately.
pub const MAIN_WINDOW_LABEL: &str = "main";

/// Shows and focuses the main window, and (on macOS) puts the app's icon back
/// in the dock so it behaves like a normal foreground app while open.
pub fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
}

/// Hides the main window without quitting, and (on macOS) removes the app's
/// icon from the dock, leaving only the menu bar tray icon behind. The OS
/// would normally reap the now-windowless app; `macos::intercept_quit_apple_event`
/// (installed at startup) is what keeps it running from the tray.
pub fn hide_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.hide();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

/// Frontend-callable command: hides the main window instead of closing it,
/// so the app keeps running from the tray. Call this from a "close"/"minimize
/// to tray" button in React.
#[tauri::command]
pub fn hide_main_window<R: Runtime>(app: AppHandle<R>) {
    hide_window(&app);
}

/// Frontend-callable command: shows and focuses the main window.
#[tauri::command]
pub fn show_main_window<R: Runtime>(app: AppHandle<R>) {
    show_window(&app);
}
