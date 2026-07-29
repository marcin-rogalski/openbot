//! A process-wide HTTP client, built once (cheap to clone — `Arc` inside).

use std::sync::LazyLock;
use std::time::Duration;

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap_or_default()
});

/// The shared HTTP client. Call `compose::commons::init` once at startup
/// to warm it before first use.
pub fn client() -> reqwest::Client {
    CLIENT.clone()
}
