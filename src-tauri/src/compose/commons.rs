//! Common composition — the shared singletons wired before anything else.
//! Composition order is **commons → driven → driving → entry point**; this is
//! the first phase, called once from `main.rs`.

/// Warm the shared infrastructure (the HTTP client today; logger/fs as they
/// land) so it's ready before any driven adapter is built.
pub fn init() {
    let _ = crate::infrastructure::shared::http::client();
}
