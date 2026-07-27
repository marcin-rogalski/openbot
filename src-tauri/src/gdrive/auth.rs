//! Google OAuth for Drive access: the installed-app (loopback) flow via
//! `yup-oauth2`. On first connect it opens the browser for consent and captures
//! the code on a loopback port; the refresh token is persisted to the app data
//! dir so later runs get a fresh access token silently.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use tauri::{AppHandle, Manager, Runtime};
use yup_oauth2::authenticator_delegate::InstalledFlowDelegate;
use yup_oauth2::{ApplicationSecret, InstalledFlowAuthenticator, InstalledFlowReturnMethod};

/// Full Drive scope — needed to read, write, and delete in an arbitrary folder.
pub const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive";

/// Token cache filename for a given OAuth client id. Keyed by client id so
/// multiple Drive tools using the same Google account share one sign-in.
fn token_cache_name(client_id: &str) -> String {
    let safe: String = client_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(64)
        .collect();
    format!("gdrive-{safe}.json")
}

/// Opens the consent URL in the system browser instead of printing it to stdout
/// (this is a GUI app). No code entry needed — loopback captures it.
struct BrowserDelegate;

impl InstalledFlowDelegate for BrowserDelegate {
    fn present_user_url<'a>(
        &'a self,
        url: &'a str,
        _need_code: bool,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        let url = url.to_string();
        Box::pin(async move {
            let _ = open::that(url);
            Ok(String::new())
        })
    }
}

fn application_secret(client_id: &str, client_secret: &str) -> ApplicationSecret {
    ApplicationSecret {
        client_id: client_id.trim().to_string(),
        client_secret: client_secret.trim().to_string(),
        auth_uri: "https://accounts.google.com/o/oauth2/auth".to_string(),
        token_uri: "https://oauth2.googleapis.com/token".to_string(),
        redirect_uris: vec!["http://127.0.0.1".to_string()],
        project_id: None,
        client_email: None,
        auth_provider_x509_cert_url: Some("https://www.googleapis.com/oauth2/v1/certs".to_string()),
        client_x509_cert_url: None,
    }
}

fn token_cache_path<R: Runtime>(app: &AppHandle<R>, client_id: &str) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(token_cache_name(client_id))
}

/// Whether a persisted token exists for this client id (so we can show
/// "connected" without triggering the browser flow just to check).
pub fn has_cached_token<R: Runtime>(app: &AppHandle<R>, client_id: &str) -> bool {
    token_cache_path(app, client_id).exists()
}

/// Fetch a fresh (auto-refreshed) Drive access token for the given OAuth client,
/// running the browser consent flow if there's no cached token yet. Building the
/// authenticator does no network; only `token()` may trigger the flow.
pub async fn access_token<R: Runtime>(
    app: &AppHandle<R>,
    client_id: &str,
    client_secret: &str,
) -> Result<String, String> {
    let secret = application_secret(client_id, client_secret);
    let auth = InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect)
        .persist_tokens_to_disk(token_cache_path(app, client_id))
        .flow_delegate(Box::new(BrowserDelegate))
        .build()
        .await
        .map_err(|e| format!("failed to build authenticator: {e}"))?;

    let token = auth
        .token(&[DRIVE_SCOPE])
        .await
        .map_err(|e| format!("authorization failed: {e}"))?;
    token
        .token()
        .map(str::to_string)
        .ok_or_else(|| "no access token returned".to_string())
}
