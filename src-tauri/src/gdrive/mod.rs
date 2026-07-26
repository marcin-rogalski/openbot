//! Google Drive v3 REST client. Uses reqwest with a bearer token from `auth`.
//! Credentials (OAuth client) and the folder come from a tool instance. Text-
//! oriented: reads text files and exports Google Docs to plain text.

pub mod auth;

use serde::Deserialize;
use serde_json::json;
use tauri::AppHandle;

use crate::config;

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

async fn token(app: &AppHandle, client_id: &str, client_secret: &str) -> Result<String, String> {
    auth::access_token(app, client_id, client_secret).await
}

fn esc(s: &str) -> String {
    s.replace('\'', "\\'")
}

async fn error_body(resp: reqwest::Response) -> String {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    format!("Drive API {status}: {}", body.trim())
}

/// Connected account's email, for the tool's status line.
pub async fn whoami(app: &AppHandle, client_id: &str, client_secret: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct About {
        user: Option<User>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct User {
        email_address: Option<String>,
    }

    let tok = token(app, client_id, client_secret).await?;
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

async fn list_query(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    q: &str,
) -> Result<Vec<DriveFile>, String> {
    let tok = token(app, client_id, client_secret).await?;
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

/// Search the folder — and its subfolders — for files matching `query`. The
/// query is tokenised and OR-ed across content and name, so multi-word queries
/// broaden recall instead of demanding an exact phrase. An empty query lists the
/// whole subtree.
pub async fn search(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
    query: &str,
) -> Result<Vec<DriveFile>, String> {
    let folders = folder_subtree(app, client_id, client_secret, folder_id).await?;
    let parents = folders
        .iter()
        .map(|id| format!("'{}' in parents", esc(id)))
        .collect::<Vec<_>>()
        .join(" or ");

    let terms: Vec<String> = query
        .split_whitespace()
        .filter(|t| t.chars().count() > 1)
        .map(esc)
        .collect();

    let q = if terms.is_empty() {
        format!("({parents}) and trashed = false")
    } else {
        let matches = terms
            .iter()
            .map(|t| format!("fullText contains '{t}' or name contains '{t}'"))
            .collect::<Vec<_>>()
            .join(" or ");
        format!("({parents}) and ({matches}) and trashed = false")
    };
    list_query(app, client_id, client_secret, &q).await
}

/// Collect the folder id plus its descendant folder ids (bounded), so search can
/// span subfolders.
async fn folder_subtree(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    root: &str,
) -> Result<Vec<String>, String> {
    const MAX_FOLDERS: usize = 100;
    let mut ids = vec![root.to_string()];
    let mut queue = vec![root.to_string()];

    while let Some(parent) = queue.pop() {
        if ids.len() >= MAX_FOLDERS {
            break;
        }
        let q = format!(
            "'{}' in parents and mimeType = 'application/vnd.google-apps.folder' and trashed = false",
            esc(&parent)
        );
        let subfolders = list_query(app, client_id, client_secret, &q).await.unwrap_or_default();
        for f in subfolders {
            if ids.len() >= MAX_FOLDERS {
                break;
            }
            if !ids.contains(&f.id) {
                ids.push(f.id.clone());
                queue.push(f.id);
            }
        }
    }
    Ok(ids)
}

pub async fn list(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
) -> Result<Vec<DriveFile>, String> {
    let q = format!("'{}' in parents and trashed = false", esc(folder_id));
    list_query(app, client_id, client_secret, &q).await
}

/// Immediate subfolders of `parent`, for semantic filing.
pub async fn list_folders(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    parent: &str,
) -> Result<Vec<DriveFile>, String> {
    let q = format!(
        "'{}' in parents and mimeType = 'application/vnd.google-apps.folder' and trashed = false",
        esc(parent)
    );
    list_query(app, client_id, client_secret, &q).await
}

pub async fn read(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    id: &str,
) -> Result<String, String> {
    let tok = token(app, client_id, client_secret).await?;
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
    } else if meta.mime_type == "application/pdf" {
        // PDFs aren't directly exportable; let Drive extract the text (incl. OCR)
        // by converting a temp copy to a Google Doc, then clean it up.
        return read_pdf(&client, &tok, id, &meta.name).await;
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

/// Extract a PDF's text via Drive: copy it into a temporary Google Doc (which
/// runs Drive's text extraction / OCR), export that as plain text, then trash
/// the temp copy. No local PDF library needed.
async fn read_pdf(
    client: &reqwest::Client,
    tok: &str,
    id: &str,
    name: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Copied {
        id: String,
    }
    let copy_meta = json!({
        "name": format!("{name} (openbot temp)"),
        "mimeType": "application/vnd.google-apps.document",
    });
    let resp = client
        .post(format!("{DRIVE_API}/files/{id}/copy"))
        .query(&[("fields", "id")])
        .bearer_auth(tok)
        .json(&copy_meta)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(error_body(resp).await);
    }
    let temp: Copied = resp.json().await.map_err(|e| format!("bad response: {e}"))?;

    let export = client
        .get(format!("{DRIVE_API}/files/{}/export", temp.id))
        .query(&[("mimeType", "text/plain")])
        .bearer_auth(tok)
        .send()
        .await;

    let result = match export {
        Ok(resp) if resp.status().is_success() => {
            resp.text().await.map_err(|e| format!("failed to read content: {e}"))
        }
        Ok(resp) => Err(error_body(resp).await),
        Err(e) => Err(format!("request failed: {e}")),
    };

    // Always clean up the temp Doc, even on export failure.
    let _ = set_trashed(client, tok, &temp.id).await;
    result
}

/// Create a subfolder under `parent` and return its id.
pub async fn create_folder(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    parent: &str,
    name: &str,
) -> Result<String, String> {
    let tok = token(app, client_id, client_secret).await?;
    let metadata = json!({
        "name": name,
        "mimeType": "application/vnd.google-apps.folder",
        "parents": [parent],
    });
    let resp = reqwest::Client::new()
        .post(format!("{DRIVE_API}/files"))
        .query(&[("fields", "id")])
        .bearer_auth(&tok)
        .json(&metadata)
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

async fn set_trashed(client: &reqwest::Client, tok: &str, id: &str) -> Result<(), String> {
    let resp = client
        .patch(format!("{DRIVE_API}/files/{id}"))
        .bearer_auth(tok)
        .json(&json!({ "trashed": true }))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(error_body(resp).await);
    }
    Ok(())
}

pub async fn create(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
    name: &str,
    content: &str,
) -> Result<String, String> {
    let tok = token(app, client_id, client_secret).await?;
    let metadata = json!({ "name": name, "parents": [folder_id] });
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

/// Upload arbitrary bytes as a new file (used to archive Discord attachments).
/// Same multipart/related shape as `create`, but a binary media part.
pub async fn upload_binary(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
    name: &str,
    bytes: Vec<u8>,
    mime: &str,
) -> Result<String, String> {
    let tok = token(app, client_id, client_secret).await?;
    let metadata = json!({ "name": name, "parents": [folder_id] });
    let boundary = "openbot_related_boundary";
    let mime = if mime.trim().is_empty() { "application/octet-stream" } else { mime };

    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(
        format!(
            "--{b}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n\
             --{b}\r\nContent-Type: {mime}\r\n\r\n",
            b = boundary,
            meta = metadata,
        )
        .as_bytes(),
    );
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--").as_bytes());

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

pub async fn update(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    id: &str,
    content: &str,
) -> Result<(), String> {
    let tok = token(app, client_id, client_secret).await?;
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

pub async fn trash(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    id: &str,
) -> Result<(), String> {
    let tok = token(app, client_id, client_secret).await?;
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

// --- Tauri commands (per tool instance) ------------------------------------

/// Run the OAuth flow for a Drive tool (browser consent on first use) and
/// return the connected account's email.
#[tauri::command]
pub async fn connect_drive(app: AppHandle, tool_id: String) -> Result<String, String> {
    let global = config::load_global(&app);
    let Some(tool) = global.tool(&tool_id).cloned() else {
        return Err("tool not found".into());
    };
    if !tool.drive_ready() {
        return Err("Fill in the client id, secret, and folder id first.".into());
    }
    whoami(&app, &tool.client_id, &tool.client_secret).await
}

/// Whether a Drive tool already has a cached token (no network / browser).
#[tauri::command]
pub fn drive_status(app: AppHandle, tool_id: String) -> bool {
    let global = config::load_global(&app);
    match global.tool(&tool_id) {
        Some(tool) => auth::has_cached_token(&app, &tool.client_id),
        None => false,
    }
}
