//! OS-integration driving adapters: the menu-bar tray, the hide-to-tray window
//! lifecycle, and macOS-only quit interception.

pub mod tray;
pub mod window;

#[cfg(target_os = "macos")]
pub mod macos;
