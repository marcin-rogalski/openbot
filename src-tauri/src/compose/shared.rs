//! Compose shared infrastructure — wired before everything else.

/// Warm the shared infra (the HTTP client today; logger/fs as they land) so it's
/// ready before any adapter or bot uses it. Call once from the composition root.
pub fn compose_shared() {
    let _ = crate::infrastructure::shared::http::client();
}
