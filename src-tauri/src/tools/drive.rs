//! The Google Drive tool: its operations and their execution. CRUD delegates to
//! the Drive driving adapter, the knowledge-base ops (ask/reindex/list_sources)
//! to the knowledge adapter, and save_link/transcribe to ingestion. Backfill and
//! transcribe_link need Discord channel context, so `discord.rs` intercepts them.

use serde_json::Value;
use tauri::AppHandle;

use super::Progress;
use crate::infrastructure::bot;
use crate::infrastructure::config::{self, BotConfig, ToolInstance};
use crate::infrastructure::driven::gdrive;
use crate::infrastructure::driving::drive as drive_ui;

/// The persisted `ToolInstance.type` this module handles.
pub const KIND: &str = "google_drive";
/// Slug fallback when the instance name doesn't yield one.
pub const SLUG: &str = "drive";

/// A Drive tool is usable once it carries its OAuth client and target folder.
pub fn ready(instance: &ToolInstance) -> bool {
    !instance.client_id.trim().is_empty()
        && !instance.client_secret.trim().is_empty()
        && !instance.folder_id.trim().is_empty()
}

#[derive(Clone, Copy)]
pub enum DriveOp {
    Search,
    Ask,
    ListSources,
    List,
    Read,
    Create,
    CreateFolder,
    Update,
    Delete,
    Reindex,
    Backfill,
    SaveLink,
    TranscribeLink,
}

impl DriveOp {
    pub const ALL: [DriveOp; 13] = [
        DriveOp::Search,
        DriveOp::Ask,
        DriveOp::ListSources,
        DriveOp::List,
        DriveOp::Read,
        DriveOp::Create,
        DriveOp::CreateFolder,
        DriveOp::Update,
        DriveOp::Delete,
        DriveOp::Reindex,
        DriveOp::Backfill,
        DriveOp::SaveLink,
        DriveOp::TranscribeLink,
    ];

    pub fn suffix(self) -> &'static str {
        match self {
            DriveOp::Search => "search",
            DriveOp::Ask => "ask",
            DriveOp::ListSources => "list_sources",
            DriveOp::List => "list",
            DriveOp::Read => "read",
            DriveOp::Create => "create",
            DriveOp::CreateFolder => "create_folder",
            DriveOp::Update => "update",
            DriveOp::Delete => "delete",
            DriveOp::Reindex => "reindex",
            DriveOp::Backfill => "backfill_attachments",
            DriveOp::SaveLink => "save_link",
            DriveOp::TranscribeLink => "transcribe_link",
        }
    }

    pub fn write(self) -> bool {
        matches!(
            self,
            DriveOp::Create
                | DriveOp::CreateFolder
                | DriveOp::Update
                | DriveOp::Delete
                | DriveOp::Reindex
                | DriveOp::Backfill
                | DriveOp::SaveLink
                | DriveOp::TranscribeLink
        )
    }

    pub fn description(self, folder_name: &str) -> String {
        match self {
            DriveOp::Search => format!(
                "Search files in the \"{folder_name}\" Drive folder by content or name (Drive \
                 indexes PDF text too). Returns a file list — for questions/summaries prefer `ask`."
            ),
            DriveOp::Ask => format!(
                "Answer a question or summarise a topic from the knowledge base built over \
                 \"{folder_name}\" — hybrid semantic + keyword retrieval across indexed file \
                 contents, returning cited passages. Prefer this for anything spanning multiple \
                 files. If it says the index is empty, run `reindex` first."
            ),
            DriveOp::ListSources => {
                format!("List the files currently in the \"{folder_name}\" knowledge index.")
            }
            DriveOp::List => format!("List the files in \"{folder_name}\"."),
            DriveOp::Read => format!(
                "Read a file's full text by id OR by a pasted Google Drive share link (in \
                 \"{folder_name}\" or anywhere the link is shared with this bot's account). \
                 Handles text files, Google Docs/Sheets, and PDFs (text extracted, OCR included). \
                 Use this to summarise a linked doc without saving it."
            ),
            DriveOp::Create => format!("Create a new plain-text file in \"{folder_name}\"."),
            DriveOp::CreateFolder => format!(
                "Create a new subfolder in \"{folder_name}\" (or inside another folder by id). \
                 Returns the new folder id, usable as a `parent` for create."
            ),
            DriveOp::Update => "Replace a file's content by id.".into(),
            DriveOp::Delete => "Move a file to trash by id.".into(),
            DriveOp::Reindex => format!(
                "Rebuild the local knowledge index from the files in \"{folder_name}\" (parses + \
                 embeds each file). Run once to bootstrap the knowledge base, or after bulk changes."
            ),
            DriveOp::Backfill => format!(
                "Scan recent messages in this channel and archive relevant attachments to \
                 \"{folder_name}\". Use when the user asks to save/archive files posted earlier."
            ),
            DriveOp::SaveLink => format!(
                "Save (copy) a file from a pasted Google Drive link into \"{folder_name}\" and \
                 index it into the knowledge base. The link must be shared with this bot's Google \
                 account (or be public). Use when the user pastes a Drive link and wants it kept."
            ),
            DriveOp::TranscribeLink => format!(
                "Transcribe an audio/video file from a pasted Google Drive link. Runs in the \
                 background: it posts the transcript + summary to this channel when finished (and \
                 saves them into \"{folder_name}\"), so you get an immediate acknowledgement. Link \
                 must be accessible to this bot's Google account."
            ),
        }
    }

    pub fn args(self) -> &'static str {
        match self {
            DriveOp::Search => "{\"query\": string}",
            DriveOp::Ask => {
                "{\"question\": string, \"k\": number (optional, passages to retrieve)}"
            }
            DriveOp::ListSources | DriveOp::Reindex => "{}",
            DriveOp::List => "{}",
            DriveOp::Delete => "{\"id\": string}",
            DriveOp::Read => "{\"id\": string (a Drive file id or a share link/url)}",
            DriveOp::Create => {
                "{\"name\": string, \"content\": string, \"parent\": string (optional folder id)}"
            }
            DriveOp::CreateFolder => "{\"name\": string, \"parent\": string (optional folder id)}",
            DriveOp::Update => "{\"id\": string, \"content\": string}",
            DriveOp::Backfill => "{\"limit\": number (optional, recent messages to scan)}",
            DriveOp::SaveLink | DriveOp::TranscribeLink => {
                "{\"url\": string (a Google Drive link or file id)}"
            }
        }
    }

    /// Present-tense left-footer label shown while the op runs.
    pub(super) fn active_label(self) -> String {
        match self {
            DriveOp::Search => "🔎 Searching Google Drive…".into(),
            DriveOp::Ask => "📚 Consulting the knowledge base…".into(),
            DriveOp::ListSources => "📇 Listing knowledge sources…".into(),
            DriveOp::Reindex => "🔄 Rebuilding the knowledge index…".into(),
            DriveOp::List => "📁 Listing Google Drive…".into(),
            DriveOp::Read => "📄 Reading a file…".into(),
            DriveOp::Create => "📝 Creating a file…".into(),
            DriveOp::CreateFolder => "📁 Creating a folder…".into(),
            DriveOp::Update => "✏️ Updating a file…".into(),
            DriveOp::Delete => "🗑️ Moving a file to trash…".into(),
            DriveOp::Backfill => "📎 Archiving recent attachments…".into(),
            DriveOp::SaveLink => "📥 Saving a linked file…".into(),
            DriveOp::TranscribeLink => "🎙️ Transcribing a linked file…".into(),
        }
    }

    /// Past-tense one-liner for the folded activity feed (no failure suffix —
    /// the caller appends it).
    pub(super) fn summary(self, args: &Value, result: &str) -> String {
        let str_arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
        match self {
            DriveOp::Search => super::quoted(
                "🔎 Searched Google Drive",
                str_arg("query"),
                super::count_prefix(result, "- id="),
            ),
            DriveOp::Ask => super::quoted(
                "📚 Consulted the knowledge base",
                str_arg("question"),
                super::count_prefix(result, "### "),
            ),
            DriveOp::ListSources => "📇 Listed knowledge sources".into(),
            DriveOp::Reindex => "🔄 Rebuilt the knowledge index".into(),
            DriveOp::List => format!(
                "📁 Listed {} file(s) in Google Drive",
                super::count_prefix(result, "- id=")
            ),
            DriveOp::Read => "📄 Read a file from Google Drive".into(),
            DriveOp::Create => "📝 Created a file in Google Drive".into(),
            DriveOp::CreateFolder => "📁 Created a folder in Google Drive".into(),
            DriveOp::Update => "✏️ Updated a file in Google Drive".into(),
            DriveOp::Delete => "🗑️ Moved a Google Drive file to trash".into(),
            DriveOp::Backfill => "📎 Backfilled attachments from recent messages".into(),
            DriveOp::SaveLink => "📥 Saved a linked file to Google Drive".into(),
            DriveOp::TranscribeLink => "🎙️ Transcribed a linked file".into(),
        }
    }
}

/// Execute a Drive op against a tool instance's folder.
#[allow(clippy::too_many_arguments)]
pub(super) async fn execute(
    app: &AppHandle,
    bot_id: &str,
    instance_id: &str,
    op: DriveOp,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
    args: &Value,
    progress: &Progress,
) -> String {
    let arg = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let (cid, secret, folder) = (client_id, client_secret, folder_id);
    let storage = crate::compose::driven::drive_storage(app, cid, secret, folder);
    match op {
        DriveOp::Search => drive_ui::search(&*storage, &arg("query")).await,
        DriveOp::Ask => {
            let Some(bot) = config::load_bot(app, bot_id) else {
                return "error: bot config not found".to_string();
            };
            let k = args
                .get("k")
                .and_then(Value::as_u64)
                .unwrap_or(6)
                .clamp(1, 12) as usize;
            crate::infrastructure::driving::knowledge::ask(
                app,
                &bot,
                instance_id,
                &arg("question"),
                k,
            )
            .await
        }
        DriveOp::ListSources => {
            crate::infrastructure::driving::knowledge::list_sources(app, instance_id).await
        }
        DriveOp::Reindex => {
            let Some(bot) = config::load_bot(app, bot_id) else {
                return "error: bot config not found".to_string();
            };
            crate::infrastructure::driving::knowledge::reindex(
                app,
                &bot,
                instance_id,
                cid,
                secret,
                folder,
                progress,
            )
            .await
        }
        DriveOp::List => drive_ui::list(&*storage).await,
        DriveOp::Read => drive_ui::read(&*storage, &arg("id")).await,
        DriveOp::Create => {
            drive_ui::create(&*storage, &arg("parent"), &arg("name"), &arg("content")).await
        }
        DriveOp::CreateFolder => {
            drive_ui::create_folder(&*storage, &arg("parent"), &arg("name")).await
        }
        DriveOp::Update => drive_ui::update(&*storage, &arg("id"), &arg("content")).await,
        DriveOp::Delete => drive_ui::trash(&*storage, &arg("id")).await,
        DriveOp::Backfill => "error: backfill must run with channel context".to_string(),
        DriveOp::SaveLink => {
            let Some(bot) = config::load_bot(app, bot_id) else {
                return "error: bot config not found".to_string();
            };
            crate::infrastructure::driving::ingestion::save_link(
                app,
                &bot,
                instance_id,
                cid,
                secret,
                folder,
                &arg("url"),
            )
            .await
        }
        DriveOp::TranscribeLink => "error: transcription must run with channel context".to_string(),
    }
}

/// Transcribe an audio/video Drive file (from a link/id): stream it to disk,
/// split into chunks, transcribe each, save a transcript + summary into the
/// folder + index them, and return the generated `.md` files (name + content) to
/// deliver to Discord. Long-running; `discord.rs` runs it as a background job.
#[allow(clippy::too_many_arguments)]
pub async fn run_transcription(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    client_id: &str,
    client_secret: &str,
    folder: &str,
    url: &str,
    progress: &Progress,
) -> Result<Vec<(String, String)>, String> {
    /// Refuse downloads larger than this (~2 GB) to avoid pathological transfers.
    const MAX_BYTES: u64 = 2_000_000_000;
    /// Cap the number of ~5-minute chunks (~8 h) so one call can't run forever.
    const MAX_CHUNKS: usize = 96;

    let id = gdrive::file_id_from_link(url);
    if id.trim().is_empty() {
        return Err("no Google Drive link or id provided".to_string());
    }
    let meta = gdrive::file_meta(app, client_id, client_secret, &id)
        .await
        .map_err(|e| {
            format!(
                "can't access that link ({e}). It must be shared with this bot's Google account."
            )
        })?;
    if !(meta.mime_type.starts_with("audio/") || meta.mime_type.starts_with("video/")) {
        return Err(format!(
            "\"{}\" is {} — not audio/video. Use read or save_link instead.",
            meta.name, meta.mime_type
        ));
    }
    if let Some(bytes) = meta.size.as_deref().and_then(|s| s.parse::<u64>().ok()) {
        if bytes > MAX_BYTES {
            return Err(format!(
                "\"{}\" is {:.1} GB — too large to transcribe (limit ~2 GB).",
                meta.name,
                bytes as f64 / 1e9
            ));
        }
    }

    // Work in a temp dir: stream the download to disk, split into WAV chunks,
    // transcribe each — bounded memory, so long recordings work.
    let work = std::env::temp_dir().join(config::new_id("openbot-tx"));
    std::fs::create_dir_all(&work).map_err(|e| format!("can't create temp dir: {e}"))?;
    let src = work.join("source");

    bot::emit_log(
        app,
        &bot.id,
        format!("transcribe: downloading \"{}\"…", meta.name),
    );
    progress.report(format!("🎙️ Transcribing \"{}\" — downloading…", meta.name));
    if let Err(e) = gdrive::download_to_path(app, client_id, client_secret, &id, &src).await {
        let _ = std::fs::remove_dir_all(&work);
        return Err(format!("download failed: {e}"));
    }
    progress.report(format!(
        "🎙️ Transcribing \"{}\" — decoding audio…",
        meta.name
    ));

    // Split + transcribe each chunk via the transcription engine; progress is
    // reported per chunk with the recording's name.
    let on_chunk = |i: usize, n: usize| {
        progress.report_with(
            format!(
                "🎙️ Transcribing \"{}\" — chunk {i}/{n} (~{} min in)…",
                meta.name,
                (i.saturating_sub(1) as u32 * crate::infrastructure::driven::audio::CHUNK_SECS)
                    / 60
            ),
            format!("{}%", i * 100 / n.max(1)),
        );
    };
    let (transcript_doc, truncated) =
        match crate::infrastructure::driving::transcription::transcribe_recording(
            bot,
            &src,
            &meta.name,
            &meta.mime_type,
            crate::infrastructure::driven::audio::CHUNK_SECS,
            MAX_CHUNKS,
            &on_chunk,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&work);
                return Err(e);
            }
        };
    let _ = std::fs::remove_dir_all(&work);

    if transcript_doc.is_empty() {
        return Err("transcription produced no text".to_string());
    }
    let plain = transcript_doc.plain();
    let mut timestamped =
        crate::infrastructure::driving::transcription::render_timestamped(&transcript_doc);
    if truncated {
        timestamped.push_str("\n\n[transcript truncated — recording exceeded the length cap]");
    }

    let summary = crate::infrastructure::driving::transcription::summarize(bot, &plain).await;

    let stem = meta
        .name
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(&meta.name);
    let files = vec![
        (
            format!("{stem}.transcript.md"),
            format!(
                "# Transcript — {}\n\n- Source: Google Drive link\n\n---\n\n{}\n",
                meta.name, timestamped
            ),
        ),
        (
            format!("{stem}.summary.md"),
            format!("# Summary — {}\n\n{}\n", meta.name, summary),
        ),
    ];

    for (fname, content) in &files {
        match gdrive::create(app, client_id, client_secret, folder, fname, content).await {
            Ok(fid) => {
                let _ = crate::compose::driving::index_document(app, bot, instance_id)
                    .run(&fid, fname, "text/markdown", content)
                    .await;
            }
            Err(e) => bot::emit_log(app, &bot.id, format!("save \"{fname}\": {e}")),
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_needs_client_and_folder() {
        let mut t = ToolInstance::default();
        assert!(!ready(&t));
        t.client_id = "a".into();
        t.client_secret = "b".into();
        t.folder_id = "c".into();
        assert!(ready(&t));
    }
}
