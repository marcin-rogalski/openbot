//! Google Drive v3 REST client, scoped to the configured folder. Uses reqwest
//! with a bearer token from `auth`. Text-oriented: reads text files and exports
//! Google Docs to plain text; creates/updates plain-text files.

pub mod auth;

use serde::Deserialize;
use serde_json::json;
use tauri::AppHandle;

use crate::config::BotConfig;

const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";
const DRIVE_UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3";

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub modified_time: Option<String>,
}

#[derive(Deserialize)]
struct FileList {
    #[serde(default)]
    files: Vec<DriveFile>,
}

async fn token(app: &AppHandle, cfg: &BotConfig) -> Result<String, String> {
    auth::access_token(app, &cfg.google_client_id, &cfg.google_client_secret).await
}

/// Escape single quotes for a Drive query string literal.
fn esc(s: &str) -> String {
    s.replace('\'', "\\'")
}

async fn error_body(resp: reqwest::Response) -> String {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    format!("Drive API {status}: {}", body.trim())
}

/// Connected account's email, for the Settings status line.
pub async fn whoami(app: &AppHandle, cfg: &BotConfig) -> Result<String, String> {
    #[derive(Deserialize)]
    struct About {
        user: Option<User>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct User {
        email_address: Option<String>,
    }

    let tok = token(app, cfg).await?;
    let resp = reqwest::Client::new()
        .get(format!("{DRIVE_API}/about"))
        .query(&[("fields", "user(emailAddress)")])
        .bearer_auth(&tok)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(error_body(resp).await);
    }
    let about: About = resp.json().await.map_err(|e| format!("bad response: {e}"))?;
    about
        .user
        .and_then(|u| u.email_address)
        .ok_or_else(|| "connected, but no email returned".to_string())
}

async fn list_query(app: &AppHandle, cfg: &BotConfig, q: &str) -> Result<Vec<DriveFile>, String> {
    let tok = token(app, cfg).await?;
    let resp = reqwest::Client::new()
        .get(format!("{DRIVE_API}/files"))
        .query(&[
            ("q", q),
            ("fields", "files(id,name,mimeType,modifiedTime)"),
            ("pageSize", "50"),
        ])
        .bearer_auth(&tok)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(error_body(resp).await);
    }
    let list: FileList = resp.json().await.map_err(|e| format!("bad response: {e}"))?;
    Ok(list.files)
}

/// Search within the folder by content (full-text) or file name. Note: Drive
/// indexes newly-uploaded *content* with a delay, so a just-created file may not
/// match on content immediately — name matches are instant.
pub async fn search(app: &AppHandle, cfg: &BotConfig, query: &str) -> Result<Vec<DriveFile>, String> {
    let folder = esc(&cfg.drive_folder_id);
    let kw = esc(query);
    let q = format!(
        "'{folder}' in parents and (fullText contains '{kw}' or name contains '{kw}') \
         and trashed = false"
    );
    list_query(app, cfg, &q).await
}

/// List files directly in the folder.
pub async fn list(app: &AppHandle, cfg: &BotConfig) -> Result<Vec<DriveFile>, String> {
    let q = format!("'{}' in parents and trashed = false", esc(&cfg.drive_folder_id));
    list_query(app, cfg, &q).await
}

/// Read a file's text content. Exports Google Docs; downloads text files;
/// refuses other binary types.
pub async fn read(app: &AppHandle, cfg: &BotConfig, id: &str) -> Result<String, String> {
    let tok = token(app, cfg).await?;
    let client = reqwest::Client::new();

    let meta: DriveFile = {
        let resp = client
            .get(format!("{DRIVE_API}/files/{id}"))
            .query(&[("fields", "id,name,mimeType")])
            .bearer_auth(&tok)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(error_body(resp).await);
        }
        resp.json().await.map_err(|e| format!("bad response: {e}"))?
    };

    let request = if meta.mime_type == "application/vnd.google-apps.document" {
        client
            .get(format!("{DRIVE_API}/files/{id}/export"))
            .query(&[("mimeType", "text/plain")])
    } else if meta.mime_type == "application/vnd.google-apps.spreadsheet" {
        client
            .get(format!("{DRIVE_API}/files/{id}/export"))
            .query(&[("mimeType", "text/csv")])
    } else if meta.mime_type.starts_with("text/")
        || matches!(
            meta.mime_type.as_str(),
            "application/json" | "application/xml" | "application/x-yaml"
        )
    {
        client
            .get(format!("{DRIVE_API}/files/{id}"))
            .query(&[("alt", "media")])
    } else {
        return Err(format!("cannot read binary type '{}'", meta.mime_type));
    };

    let resp = request
        .bearer_auth(&tok)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(error_body(resp).await);
    }
    resp.text().await.map_err(|e| format!("failed to read content: {e}"))
}

/// Create a plain-text file in the folder. Returns the new file id.
pub async fn create(
    app: &AppHandle,
    cfg: &BotConfig,
    name: &str,
    content: &str,
) -> Result<String, String> {
    let tok = token(app, cfg).await?;
    let metadata = json!({ "name": name, "parents": [cfg.drive_folder_id] });
    let boundary = "openbot_related_boundary";
    let body = format!(
        "--{b}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n\
         --{b}\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n{content}\r\n--{b}--",
        b = boundary,
        meta = metadata,
    );

    let resp = reqwest::Client::new()
        .post(format!("{DRIVE_UPLOAD}/files"))
        .query(&[("uploadType", "multipart"), ("fields", "id")])
        .bearer_auth(&tok)
        .header("Content-Type", format!("multipart/related; boundary={boundary}"))
        .body(body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(error_body(resp).await);
    }

    #[derive(Deserialize)]
    struct Created {
        id: String,
    }
    let created: Created = resp.json().await.map_err(|e| format!("bad response: {e}"))?;
    Ok(created.id)
}

/// Replace a file's content with new plain text.
pub async fn update(app: &AppHandle, cfg: &BotConfig, id: &str, content: &str) -> Result<(), String> {
    let tok = token(app, cfg).await?;
    let resp = reqwest::Client::new()
        .patch(format!("{DRIVE_UPLOAD}/files/{id}"))
        .query(&[("uploadType", "media")])
        .bearer_auth(&tok)
        .header("Content-Type", "text/plain; charset=UTF-8")
        .body(content.to_string())
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(error_body(resp).await);
    }
    Ok(())
}

// --- Tauri commands --------------------------------------------------------

/// Run the OAuth flow (browser consent on first use) and return the connected
/// account's email. Surfaced by the Settings "Connect" button.
#[tauri::command]
pub async fn connect_drive(app: AppHandle) -> Result<String, String> {
    let cfg = crate::config::load(&app);
    if !cfg.drive_ready() {
        return Err("Fill in Google client id, client secret, and folder id first.".into());
    }
    whoami(&app, &cfg).await
}

/// Whether Drive already has a cached token (no network / browser).
#[tauri::command]
pub fn drive_status(app: AppHandle) -> bool {
    auth::has_cached_token(&app)
}

/// Move a file to the trash (recoverable) rather than deleting permanently.
pub async fn trash(app: &AppHandle, cfg: &BotConfig, id: &str) -> Result<(), String> {
    let tok = token(app, cfg).await?;
    let resp = reqwest::Client::new()
        .patch(format!("{DRIVE_API}/files/{id}"))
        .query(&[("fields", "id")])
        .bearer_auth(&tok)
        .json(&json!({ "trashed": true }))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(error_body(resp).await);
    }
    Ok(())
}
