//! Dynamic tool catalog. A bot's usable tools are resolved from its
//! `enabled_tool_ids` against the global tool registry, plus its own memory
//! capability:
//! - each enabled Google Drive instance contributes
//!   `<slug>_search/read/list/create/update/delete` ops scoped to that folder;
//! - each enabled Web Search instance contributes `<slug>_search/fetch`;
//! - if the bot has memory enabled, `memory_save`/`memory_delete`.
//!
//! Policies key on `<toolInstanceId>/<op>` (memory uses the instance id
//! `"memory"`).

use std::collections::HashSet;

use serde_json::Value;
use tauri::AppHandle;

use crate::config::{self, BotConfig, GlobalConfig};
use crate::gdrive::{self, DriveFile};
use crate::knowledge::{self, SourceMeta};
use crate::{bot, ingest, memory, model, websearch};

/// Fixed instance id for the per-bot memory tools.
const MEMORY_INSTANCE: &str = "memory";

// --- Ops --------------------------------------------------------------------

#[derive(Clone, Copy)]
enum DriveOp {
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
}

impl DriveOp {
    const ALL: [DriveOp; 11] = [
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
    ];

    fn suffix(self) -> &'static str {
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
        }
    }

    fn write(self) -> bool {
        matches!(
            self,
            DriveOp::Create
                | DriveOp::CreateFolder
                | DriveOp::Update
                | DriveOp::Delete
                | DriveOp::Reindex
                | DriveOp::Backfill
        )
    }

    fn description(self, folder_name: &str) -> String {
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
                "Read one file's full text by id (from \"{folder_name}\"). Handles text files, \
                 Google Docs/Sheets, and PDFs (text is extracted, OCR included)."
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
        }
    }

    fn args(self) -> &'static str {
        match self {
            DriveOp::Search => "{\"query\": string}",
            DriveOp::Ask => "{\"question\": string, \"k\": number (optional, passages to retrieve)}",
            DriveOp::ListSources | DriveOp::Reindex => "{}",
            DriveOp::List => "{}",
            DriveOp::Read | DriveOp::Delete => "{\"id\": string}",
            DriveOp::Create => "{\"name\": string, \"content\": string, \"parent\": string (optional folder id)}",
            DriveOp::CreateFolder => "{\"name\": string, \"parent\": string (optional folder id)}",
            DriveOp::Update => "{\"id\": string, \"content\": string}",
            DriveOp::Backfill => "{\"limit\": number (optional, recent messages to scan)}",
        }
    }
}

#[derive(Clone, Copy)]
enum WebOp {
    Search,
    Fetch,
}

impl WebOp {
    const ALL: [WebOp; 2] = [WebOp::Search, WebOp::Fetch];

    fn suffix(self) -> &'static str {
        match self {
            WebOp::Search => "search",
            WebOp::Fetch => "fetch",
        }
    }

    fn description(self) -> &'static str {
        match self {
            WebOp::Search => "Search the web; returns a list of results (title, url, excerpt).",
            WebOp::Fetch => "Fetch a web page by url and return its main text content.",
        }
    }

    fn args(self) -> &'static str {
        match self {
            WebOp::Search => "{\"query\": string}",
            WebOp::Fetch => "{\"url\": string}",
        }
    }
}

#[derive(Clone, Copy)]
enum MemoryOp {
    Save,
    Delete,
}

impl MemoryOp {
    const ALL: [MemoryOp; 2] = [MemoryOp::Save, MemoryOp::Delete];

    fn suffix(self) -> &'static str {
        match self {
            MemoryOp::Save => "save",
            MemoryOp::Delete => "delete",
        }
    }

    fn call_name(self) -> &'static str {
        match self {
            MemoryOp::Save => "memory_save",
            MemoryOp::Delete => "memory_delete",
        }
    }

    fn description(self) -> &'static str {
        match self {
            MemoryOp::Save => "Remember a fact ('note') or a standing instruction ('rule') for \
                               future conversations.",
            MemoryOp::Delete => "Forget a memory by its id.",
        }
    }

    fn args(self) -> &'static str {
        match self {
            MemoryOp::Save => "{\"kind\": \"note\"|\"rule\", \"text\": string}",
            MemoryOp::Delete => "{\"id\": string}",
        }
    }
}

// --- Resolved tools ---------------------------------------------------------

/// The concrete backend a resolved tool dispatches to, with its bound context.
enum ToolKind {
    Drive {
        op: DriveOp,
        instance_name: String,
        client_id: String,
        client_secret: String,
        folder_id: String,
    },
    Web { op: WebOp, api_key: String },
    Memory { op: MemoryOp },
}

/// One callable tool for a specific bot.
pub struct ResolvedTool {
    pub call_name: String,
    pub instance_id: String,
    description: String,
    kind: ToolKind,
}

impl ResolvedTool {
    pub fn is_write(&self) -> bool {
        match &self.kind {
            ToolKind::Drive { op, .. } => op.write(),
            // Web is read-only; memory is the bot managing its own state.
            ToolKind::Web { .. } | ToolKind::Memory { .. } => false,
        }
    }

    fn op_suffix(&self) -> &'static str {
        match &self.kind {
            ToolKind::Drive { op, .. } => op.suffix(),
            ToolKind::Web { op, .. } => op.suffix(),
            ToolKind::Memory { op } => op.suffix(),
        }
    }

    fn args(&self) -> &'static str {
        match &self.kind {
            ToolKind::Drive { op, .. } => op.args(),
            ToolKind::Web { op, .. } => op.args(),
            ToolKind::Memory { op } => op.args(),
        }
    }

    /// Policy lookup key: `<toolInstanceId>/<op>`.
    pub fn policy_key(&self) -> String {
        format!("{}/{}", self.instance_id, self.op_suffix())
    }

    /// True for the Drive `backfill_attachments` op, which `discord.rs` handles
    /// specially (it needs Discord channel history).
    pub fn is_backfill(&self) -> bool {
        matches!(&self.kind, ToolKind::Drive { op: DriveOp::Backfill, .. })
    }

    /// The attachment sink for a Drive tool, so backfill can archive to this
    /// instance's folder.
    pub fn drive_sink(&self) -> Option<AttachmentSink> {
        match &self.kind {
            ToolKind::Drive { instance_name, client_id, client_secret, folder_id, .. } => {
                Some(AttachmentSink::Drive {
                    instance_id: self.instance_id.clone(),
                    instance_name: instance_name.clone(),
                    client_id: client_id.clone(),
                    client_secret: client_secret.clone(),
                    folder_id: folder_id.clone(),
                })
            }
            _ => None,
        }
    }

    /// A friendly one-liner describing what this call did, for the folded
    /// (non-verbose) activity feed.
    pub fn summary(&self, args: &Value, result: &str) -> String {
        let str_arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
        let failed = result.starts_with("error:");
        match &self.kind {
            ToolKind::Drive { op, .. } => match op {
                DriveOp::Search => {
                    quoted("🔎 Searched Google Drive", str_arg("query"), count_prefix(result, "- id="))
                }
                DriveOp::Ask => quoted(
                    "📚 Consulted the knowledge base",
                    str_arg("question"),
                    count_prefix(result, "### "),
                ),
                DriveOp::ListSources => "📇 Listed knowledge sources".into(),
                DriveOp::Reindex => "🔄 Rebuilt the knowledge index".into(),
                DriveOp::List => {
                    format!("📁 Listed {} file(s) in Google Drive", count_prefix(result, "- id="))
                }
                DriveOp::Read => "📄 Read a file from Google Drive".into(),
                DriveOp::Create => "📝 Created a file in Google Drive".into(),
                DriveOp::CreateFolder => "📁 Created a folder in Google Drive".into(),
                DriveOp::Update => "✏️ Updated a file in Google Drive".into(),
                DriveOp::Delete => "🗑️ Moved a Google Drive file to trash".into(),
                DriveOp::Backfill => "📎 Backfilled attachments from recent messages".into(),
            },
            ToolKind::Web { op, .. } => match op {
                WebOp::Search => {
                    quoted("🌐 Searched the web", str_arg("query"), count_prefix(result, "- "))
                }
                WebOp::Fetch => format!("🌐 Read {}", domain_of(str_arg("url"))),
            },
            ToolKind::Memory { op } => match op {
                MemoryOp::Save => {
                    let kind = if str_arg("kind") == "rule" { "rule" } else { "note" };
                    format!("🧠 Remembered a {kind}")
                }
                MemoryOp::Delete => "🧠 Forgot a memory".into(),
            },
        }
        .to_string()
            + if failed { " (failed)" } else { "" }
    }

    /// URLs this call surfaced, for the reply's "Sources" header. A fetch's
    /// source is its `url` arg; a search's are the result URLs. Empty for
    /// non-web tools.
    pub fn source_urls(&self, args: &Value, result: &str) -> Vec<String> {
        match &self.kind {
            ToolKind::Web { op: WebOp::Fetch, .. } => args
                .get("url")
                .and_then(Value::as_str)
                .filter(|u| u.starts_with("http"))
                .map(|u| vec![u.to_string()])
                .unwrap_or_default(),
            ToolKind::Web { op: WebOp::Search, .. } => result
                .lines()
                .map(str::trim)
                .filter(|l| l.starts_with("http"))
                .map(String::from)
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// Build the tool catalog a bot may use.
pub fn catalog(global: &GlobalConfig, bot: &BotConfig) -> Vec<ResolvedTool> {
    let mut tools = Vec::new();
    let mut used_slugs: HashSet<String> = HashSet::new();

    for tool_id in &bot.enabled_tool_ids {
        let Some(instance) = global.tool(tool_id) else {
            continue;
        };

        match instance.kind.as_str() {
            "google_drive" if instance.drive_ready() => {
                let slug = unique_slug(&instance.name, "drive", &mut used_slugs);
                for op in DriveOp::ALL {
                    tools.push(ResolvedTool {
                        call_name: format!("{slug}_{}", op.suffix()),
                        instance_id: instance.id.clone(),
                        description: op.description(&instance.name),
                        kind: ToolKind::Drive {
                            op,
                            instance_name: instance.name.clone(),
                            client_id: instance.client_id.clone(),
                            client_secret: instance.client_secret.clone(),
                            folder_id: instance.folder_id.clone(),
                        },
                    });
                }
            }
            "web_search" if instance.web_ready() => {
                let slug = unique_slug(&instance.name, "web", &mut used_slugs);
                for op in WebOp::ALL {
                    tools.push(ResolvedTool {
                        call_name: format!("{slug}_{}", op.suffix()),
                        instance_id: instance.id.clone(),
                        description: op.description().to_string(),
                        kind: ToolKind::Web { op, api_key: instance.api_key.clone() },
                    });
                }
            }
            _ => {}
        }
    }

    if bot.memory_enabled {
        for op in MemoryOp::ALL {
            tools.push(ResolvedTool {
                call_name: op.call_name().to_string(),
                instance_id: MEMORY_INSTANCE.to_string(),
                description: op.description().to_string(),
                kind: ToolKind::Memory { op },
            });
        }
    }

    tools
}

pub fn find<'a>(catalog: &'a [ResolvedTool], call_name: &str) -> Option<&'a ResolvedTool> {
    catalog.iter().find(|t| t.call_name == call_name)
}

/// The tools section appended to the system prompt.
pub fn prompt_section(catalog: &[ResolvedTool]) -> String {
    let mut section = String::from(
        "\n\nYou have tools. To use one, output EXACTLY one line and nothing else:\n\
         TOOL_CALL {\"tool\": \"<name>\", \"args\": { ... }}\n\
         You'll then receive a line `TOOL_RESULT: <result>`. Call tools as many times as needed; \
         when you have the final answer, reply normally WITHOUT a TOOL_CALL line. \
         Available tools:\n",
    );
    for tool in catalog {
        section.push_str(&format!(
            "- {} — {} args: {}\n",
            tool.call_name,
            tool.description,
            tool.args()
        ));
    }
    section
}

/// Run a resolved tool call; returns a result string (ok or `error: …`).
pub async fn execute(app: &AppHandle, bot_id: &str, tool: &ResolvedTool, args: &Value) -> String {
    let arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("").to_string();

    match &tool.kind {
        ToolKind::Drive { op, client_id, client_secret, folder_id, .. } => {
            let (cid, secret, folder) = (client_id, client_secret, folder_id);
            match op {
                DriveOp::Search => {
                    match gdrive::search(app, cid, secret, folder, &arg("query")).await {
                        Ok(files) => format_files(&files),
                        Err(e) => format!("error: {e}"),
                    }
                }
                DriveOp::Ask => {
                    let Some(bot) = config::load_bot(app, bot_id) else {
                        return "error: bot config not found".to_string();
                    };
                    let question = arg("question");
                    let k = args.get("k").and_then(Value::as_u64).unwrap_or(6).clamp(1, 12) as usize;
                    let emb = match model::embed(&bot, std::slice::from_ref(&question)).await {
                        Ok(mut v) if !v.is_empty() => v.remove(0),
                        Ok(_) => return "error: no embedding returned".to_string(),
                        Err(e) => return format!("error: embeddings failed: {e}"),
                    };
                    match knowledge::search(app, &tool.instance_id, emb, question.clone(), k).await {
                        Ok(hits) if hits.is_empty() => {
                            "the knowledge index is empty — run reindex first, then ask again"
                                .to_string()
                        }
                        Ok(hits) => format_hits(&question, &hits),
                        Err(e) => format!("error: {e}"),
                    }
                }
                DriveOp::ListSources => match knowledge::list_sources(app, &tool.instance_id).await {
                    Ok(list) if list.is_empty() => "the knowledge index is empty".to_string(),
                    Ok(list) => list
                        .iter()
                        .map(|(name, drive_id, n)| {
                            format!("- {name} (drive_id={drive_id}, {n} chunks)")
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    Err(e) => format!("error: {e}"),
                },
                DriveOp::Reindex => {
                    let Some(bot) = config::load_bot(app, bot_id) else {
                        return "error: bot config not found".to_string();
                    };
                    reindex(app, &bot, &tool.instance_id, cid, secret, folder).await
                }
                DriveOp::List => match gdrive::list(app, cid, secret, folder).await {
                    Ok(files) => format_files(&files),
                    Err(e) => format!("error: {e}"),
                },
                DriveOp::Read => match gdrive::read(app, cid, secret, &arg("id")).await {
                    Ok(text) => truncate(&text, 6000),
                    Err(e) => format!("error: {e}"),
                },
                DriveOp::Create => {
                    let parent = parent_or(&arg("parent"), folder);
                    match gdrive::create(app, cid, secret, &parent, &arg("name"), &arg("content"))
                        .await
                    {
                        Ok(id) => format!("created file id={id}"),
                        Err(e) => format!("error: {e}"),
                    }
                }
                DriveOp::CreateFolder => {
                    let parent = parent_or(&arg("parent"), folder);
                    match gdrive::create_folder(app, cid, secret, &parent, &arg("name")).await {
                        Ok(id) => format!("created folder id={id}"),
                        Err(e) => format!("error: {e}"),
                    }
                }
                DriveOp::Update => {
                    match gdrive::update(app, cid, secret, &arg("id"), &arg("content")).await {
                        Ok(()) => "updated".to_string(),
                        Err(e) => format!("error: {e}"),
                    }
                }
                DriveOp::Delete => match gdrive::trash(app, cid, secret, &arg("id")).await {
                    Ok(()) => "moved to trash".to_string(),
                    Err(e) => format!("error: {e}"),
                },
                // Backfill needs Discord history, so it's intercepted in
                // discord.rs `run_tool` before reaching here.
                DriveOp::Backfill => {
                    "error: backfill must run with channel context".to_string()
                }
            }
        }
        ToolKind::Web { op, api_key } => match op {
            WebOp::Search => match websearch::search(api_key, &arg("query")).await {
                Ok(results) => results,
                Err(e) => format!("error: {e}"),
            },
            WebOp::Fetch => match websearch::fetch(api_key, &arg("url")).await {
                Ok(content) => content,
                Err(e) => format!("error: {e}"),
            },
        },
        ToolKind::Memory { op } => match op {
            MemoryOp::Save => memory::save(app, bot_id, &arg("kind"), &arg("text")).await,
            MemoryOp::Delete => {
                memory::delete(app, bot_id, &arg("id"));
                "forgotten".to_string()
            }
        },
    }
}

// --- Attachment gate --------------------------------------------------------

/// A Discord attachment the bot noticed in a conversation. Built by `discord.rs`
/// from serenity's `Attachment`.
#[derive(Clone)]
pub struct AttachmentRef {
    pub filename: String,
    pub content_type: String,
    pub url: String,
}

/// A tool that has subscribed to the attachment gate. Adding a subscriber = add
/// a variant here + a case in `attachment_sinks` and `deliver_attachment`.
pub enum AttachmentSink {
    Drive {
        instance_id: String,
        instance_name: String,
        client_id: String,
        client_secret: String,
        folder_id: String,
    },
}

/// The subscribers for a bot: derived from its enabled tool instances. Currently
/// every ready Google Drive instance archives attachments.
pub fn attachment_sinks(global: &GlobalConfig, bot: &BotConfig) -> Vec<AttachmentSink> {
    let mut sinks = Vec::new();
    for tool_id in &bot.enabled_tool_ids {
        if let Some(instance) = global.tool(tool_id) {
            if instance.kind == "google_drive" && instance.drive_ready() {
                sinks.push(AttachmentSink::Drive {
                    instance_id: instance.id.clone(),
                    instance_name: instance.name.clone(),
                    client_id: instance.client_id.clone(),
                    client_secret: instance.client_secret.clone(),
                    folder_id: instance.folder_id.clone(),
                });
            }
        }
    }
    sinks
}

/// Deliver one attachment to one subscribed sink. The Drive sink runs the
/// intent gate (message context + the bot's standing rules) and, on "yes",
/// downloads the file from Discord and uploads it. Returns whether it archived.
pub async fn deliver_attachment(
    app: &AppHandle,
    bot: &BotConfig,
    sink: &AttachmentSink,
    att: &AttachmentRef,
    context: &str,
) -> bool {
    match sink {
        AttachmentSink::Drive {
            instance_id,
            instance_name,
            client_id,
            client_secret,
            folder_id,
        } => {
            let guidance = if bot.memory_enabled {
                memory::guidance(&memory::load(app, &bot.id))
            } else {
                String::new()
            };
            let grab =
                model::should_archive(bot, &guidance, context, &att.filename, &att.content_type)
                    .await;
            if !grab {
                bot::emit_log(
                    app,
                    &bot.id,
                    format!("attachment \"{}\": skipped (not relevant)", att.filename),
                );
                return false;
            }

            let bytes = match download(&att.url).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    bot::emit_log(
                        app,
                        &bot.id,
                        format!("attachment \"{}\": download failed: {e}", att.filename),
                    );
                    return false;
                }
            };

            // Parse text now (before `bytes` is moved into the upload); PDF
            // parsing is CPU-bound, so keep it off the async runtime.
            let extracted = {
                let extract_bytes = bytes.clone();
                let filename = att.filename.clone();
                let mime = att.content_type.clone();
                tokio::task::spawn_blocking(move || ingest::extract_text(&extract_bytes, &filename, &mime))
                    .await
                    .ok()
                    .flatten()
            };

            // Semantic foldering: pick a subfolder (rule-guided), else the root.
            let target = choose_folder(
                app, bot, &guidance, context, client_id, client_secret, folder_id, &att.filename,
            )
            .await;

            let drive_id = match gdrive::upload_binary(
                app,
                client_id,
                client_secret,
                &target,
                &att.filename,
                bytes,
                &att.content_type,
            )
            .await
            {
                Ok(id) => id,
                Err(e) => {
                    bot::emit_log(
                        app,
                        &bot.id,
                        format!("attachment \"{}\": archive failed: {e}", att.filename),
                    );
                    return false;
                }
            };
            bot::emit_tool_activity(
                app,
                &bot.id,
                format!(
                    "archive_attachment {{name={:?}, to={:?}}} → id={drive_id}",
                    att.filename, instance_name
                ),
                format!("📎 Archived \"{}\" to {}", att.filename, instance_name),
            );

            // Index into the local knowledge base (best-effort).
            if let Some(text) = extracted {
                index_text(
                    app,
                    bot,
                    instance_id,
                    &drive_id,
                    &att.filename,
                    &att.content_type,
                    &text,
                )
                .await;
            }
            true
        }
    }
}

/// Pick the best subfolder for a file (rule-guided model classification), or the
/// root when there are no subfolders / no clear match.
async fn choose_folder(
    app: &AppHandle,
    bot: &BotConfig,
    guidance: &str,
    context: &str,
    client_id: &str,
    client_secret: &str,
    root: &str,
    filename: &str,
) -> String {
    let subfolders = gdrive::list_folders(app, client_id, client_secret, root).await.unwrap_or_default();
    if subfolders.is_empty() {
        return root.to_string();
    }
    let names: Vec<String> = subfolders.iter().map(|f| f.name.clone()).collect();
    match model::pick_folder(bot, guidance, context, filename, &names).await {
        Some(name) => subfolders
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.id.clone())
            .unwrap_or_else(|| root.to_string()),
        None => root.to_string(),
    }
}

/// Chunk + embed + upsert a source into the local index. Best-effort; logs on failure.
async fn index_text(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    drive_id: &str,
    name: &str,
    mime: &str,
    text: &str,
) {
    let chunks = ingest::chunk(text);
    if chunks.is_empty() {
        return;
    }
    let embeddings = match model::embed(bot, &chunks).await {
        Ok(embeddings) => embeddings,
        Err(e) => {
            bot::emit_log(app, &bot.id, format!("index: embed failed for \"{name}\": {e}"));
            return;
        }
    };
    let paired: Vec<(String, Vec<f32>)> = chunks.into_iter().zip(embeddings).collect();
    let meta = SourceMeta {
        drive_id: drive_id.to_string(),
        name: name.to_string(),
        mime: mime.to_string(),
        embed_model: bot.model.embedding_model.clone(),
    };
    match knowledge::upsert_source(app, instance_id, meta, paired).await {
        Ok(()) => bot::emit_log(app, &bot.id, format!("indexed \"{name}\" into the knowledge base")),
        Err(e) => bot::emit_log(app, &bot.id, format!("index failed for \"{name}\": {e}")),
    }
}

async fn download(url: &str) -> Result<Vec<u8>, String> {
    let resp = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.bytes().await.map(|b| b.to_vec()).map_err(|e| format!("read failed: {e}"))
}

// --- Helpers ----------------------------------------------------------------

/// The model-supplied `parent` folder id, or the tool's root folder when absent.
fn parent_or(arg: &str, root: &str) -> String {
    if arg.trim().is_empty() { root.to_string() } else { arg.trim().to_string() }
}

/// Format retrieved knowledge chunks as cited passages for the model to
/// synthesise from.
fn format_hits(question: &str, hits: &[knowledge::Hit]) -> String {
    let mut out = format!(
        "Knowledge for \"{question}\". Synthesise an answer grounded ONLY in these passages and \
         cite files by name.\n\n"
    );
    for h in hits {
        out.push_str(&format!("### {} (drive_id={})\n{}\n\n", h.name, h.drive_id, h.text.trim()));
    }
    out.trim().to_string()
}

/// Rebuild the local index from the Drive folder: read + chunk + embed every
/// supported file not already indexed. The index is a derived cache, so this can
/// always reconstruct it.
async fn reindex(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
) -> String {
    bot::emit_log(app, &bot.id, "reindex: scanning Drive…");
    let files = match gdrive::search(app, client_id, client_secret, folder_id, "").await {
        Ok(files) => files,
        Err(e) => return format!("error: {e}"),
    };

    let (mut indexed, mut skipped, mut failed) = (0usize, 0usize, 0usize);
    for f in &files {
        if f.mime_type == "application/vnd.google-apps.folder" {
            continue;
        }
        if knowledge::has_source(app, instance_id, &f.id).await.unwrap_or(false) {
            skipped += 1;
            continue;
        }
        match index_drive_file(app, bot, instance_id, client_id, client_secret, f).await {
            Ok(true) => indexed += 1,
            Ok(false) => skipped += 1,
            Err(_) => failed += 1,
        }
    }
    format!("indexed {indexed} new file(s), skipped {skipped}, failed {failed}")
}

/// Read a Drive file's text, chunk + embed it, and upsert into the index.
/// `Ok(false)` = unsupported/unreadable (skipped, not an error).
async fn index_drive_file(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    client_id: &str,
    client_secret: &str,
    file: &DriveFile,
) -> Result<bool, String> {
    let Ok(text) = gdrive::read(app, client_id, client_secret, &file.id).await else {
        return Ok(false);
    };
    let chunks = ingest::chunk(&text);
    if chunks.is_empty() {
        return Ok(false);
    }
    let embeddings = model::embed(bot, &chunks).await?;
    let paired: Vec<(String, Vec<f32>)> = chunks.into_iter().zip(embeddings).collect();
    knowledge::upsert_source(
        app,
        instance_id,
        SourceMeta {
            drive_id: file.id.clone(),
            name: file.name.clone(),
            mime: file.mime_type.clone(),
            embed_model: bot.model.embedding_model.clone(),
        },
        paired,
    )
    .await?;
    Ok(true)
}

/// A unique, readable per-instance prefix, falling back to `fallback`.
fn unique_slug(name: &str, fallback: &str, used: &mut HashSet<String>) -> String {
    let base = {
        let s = slugify(name);
        if s.is_empty() { fallback.to_string() } else { s }
    };
    let mut slug = base.clone();
    let mut n = 2;
    while used.contains(&slug) {
        slug = format!("{base}{n}");
        n += 1;
    }
    used.insert(slug.clone());
    slug
}

/// `"{prefix} for \"{query}\" — {n} result(s)"`, tidily handling an empty query.
fn quoted(prefix: &str, query: &str, count: usize) -> String {
    let query = query.trim();
    let head = if query.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix} for \"{query}\"")
    };
    format!("{head} — {count} result(s)")
}

/// Count result lines beginning with `prefix` (one per item in our formats).
fn count_prefix(result: &str, prefix: &str) -> usize {
    result.lines().filter(|l| l.starts_with(prefix)).count()
}

/// The host of a URL (without scheme or leading `www.`), for a compact summary.
fn domain_of(url: &str) -> String {
    let after_scheme = url.rsplit("://").next().unwrap_or(url);
    let host = after_scheme.split('/').next().unwrap_or(after_scheme);
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

fn format_files(files: &[DriveFile]) -> String {
    if files.is_empty() {
        return "no files found".to_string();
    }
    files
        .iter()
        .map(|f| {
            let modified = f.modified_time.as_deref().unwrap_or("");
            format!(
                "- id={} name=\"{}\" type={} modified={}",
                f.id, f.name, f.mime_type, modified
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut prev_us = false;
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_us = false;
        } else if !prev_us && !out.is_empty() {
            out.push('_');
            prev_us = true;
        }
    }
    out.trim_matches('_').to_string()
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push_str("…[truncated]");
    out
}
