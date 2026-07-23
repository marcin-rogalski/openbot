use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

/// Label of the window declared in tauri.conf.json. Kept in one place so
/// tray.rs and main.rs never hardcode the string separately.
pub const MAIN_WINDOW_LABEL: &str = "main";

pub fn toggle_visibility<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<()> {
    if window.is_visible().unwrap_or(false) {
        window.hide()
    } else {
        window.show()?;
        window.set_focus()
    }
}

/// Frontend-callable command: hides the main window instead of closing it,
/// so the app keeps running from the tray. Call this from a "close"/"minimize
/// to tray" button in React.
#[tauri::command]
pub fn hide_main_window<R: Runtime>(app: AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

/// Frontend-callable command: shows and focuses the main window.
#[tauri::command]
pub fn show_main_window<R: Runtime>(app: AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
