//! macOS-only glue to keep the app alive in the tray after its window is
//! hidden.
//!
//! When the app's only window is hidden (`orderOut:`), macOS decides the app
//! is a visibly-windowless GUI app and fires a Quit AppleEvent
//! (`kCoreEventClass` / `kAEQuitApplication`). AppKit's default handler turns
//! that into `terminate:` — a path below Tauri's event loop, so neither
//! `prevent_close` nor `RunEvent::ExitRequested` / `prevent_exit` can stop it
//! (Tauri does not yet expose `applicationShouldTerminate:`, see
//! tauri-apps/tauri#12978). The same quit also arrives via macOS "automatic
//! termination" when the app is an accessory.
//!
//! We install our own AppleEvent handler for that event and simply do nothing,
//! which overrides AppKit's default and keeps the process running until the
//! user explicitly chooses Quit from the tray.

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, AnyThread};
use objc2_core_services::{kAEQuitApplication, kCoreEventClass};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager};

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "OpenbotQuitHandler"]
    struct QuitHandler;

    impl QuitHandler {
        #[unsafe(method(handleAppleEvent:withReplyEvent:))]
        fn handle_apple_event(
            &self,
            _event: &NSAppleEventDescriptor,
            _reply: &NSAppleEventDescriptor,
        ) {
            // Swallow the Quit AppleEvent: doing nothing keeps the app alive.
        }
    }
);

/// Register the Quit AppleEvent interceptor. Call once, on the main thread,
/// after Tauri has set up the application.
pub fn intercept_quit_apple_event() {
    let handler: Retained<QuitHandler> = unsafe { msg_send![QuitHandler::alloc(), init] };
    let manager = NSAppleEventManager::sharedAppleEventManager();

    unsafe {
        manager.setEventHandler_andSelector_forEventClass_andEventID(
            &handler,
            sel!(handleAppleEvent:withReplyEvent:),
            kCoreEventClass as _,
            kAEQuitApplication as _,
        );
    }

    // The manager does not retain its handler; keep ours alive for the whole
    // process lifetime so the callback stays valid.
    std::mem::forget(handler);
}
