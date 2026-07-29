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
use crate::gdrive;
use crate::knowledge::{self, SourceMeta};
use crate::{bot, ingest, model};

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
    SaveLink,
    TranscribeLink,
}

impl DriveOp {
    const ALL: [DriveOp; 13] = [
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
            DriveOp::SaveLink => "save_link",
            DriveOp::TranscribeLink => "transcribe_link",
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
                | DriveOp::SaveLink
                | DriveOp::TranscribeLink
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

    fn args(self) -> &'static str {
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
            MemoryOp::Save => {
                "Remember a fact ('note') or a standing instruction ('rule') for \
                               future conversations."
            }
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
    Web {
        op: WebOp,
        api_key: String,
    },
    Memory {
        op: MemoryOp,
    },
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
        matches!(
            &self.kind,
            ToolKind::Drive {
                op: DriveOp::Backfill,
                ..
            }
        )
    }

    /// True for the Drive `transcribe_link` op, which `discord.rs` runs as a
    /// background job (it posts the transcript to the channel when done).
    pub fn is_transcribe_link(&self) -> bool {
        matches!(
            &self.kind,
            ToolKind::Drive {
                op: DriveOp::TranscribeLink,
                ..
            }
        )
    }

    /// The attachment sink for a Drive tool, so backfill can archive to this
    /// instance's folder.
    pub fn drive_sink(&self) -> Option<AttachmentSink> {
        match &self.kind {
            ToolKind::Drive {
                instance_name,
                client_id,
                client_secret,
                folder_id,
                ..
            } => Some(AttachmentSink::Drive {
                instance_id: self.instance_id.clone(),
                instance_name: instance_name.clone(),
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
                folder_id: folder_id.clone(),
            }),
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
                DriveOp::Search => quoted(
                    "🔎 Searched Google Drive",
                    str_arg("query"),
                    count_prefix(result, "- id="),
                ),
                DriveOp::Ask => quoted(
                    "📚 Consulted the knowledge base",
                    str_arg("question"),
                    count_prefix(result, "### "),
                ),
                DriveOp::ListSources => "📇 Listed knowledge sources".into(),
                DriveOp::Reindex => "🔄 Rebuilt the knowledge index".into(),
                DriveOp::List => {
                    format!(
                        "📁 Listed {} file(s) in Google Drive",
                        count_prefix(result, "- id=")
                    )
                }
                DriveOp::Read => "📄 Read a file from Google Drive".into(),
                DriveOp::Create => "📝 Created a file in Google Drive".into(),
                DriveOp::CreateFolder => "📁 Created a folder in Google Drive".into(),
                DriveOp::Update => "✏️ Updated a file in Google Drive".into(),
                DriveOp::Delete => "🗑️ Moved a Google Drive file to trash".into(),
                DriveOp::Backfill => "📎 Backfilled attachments from recent messages".into(),
                DriveOp::SaveLink => "📥 Saved a linked file to Google Drive".into(),
                DriveOp::TranscribeLink => "🎙️ Transcribed a linked file".into(),
            },
            ToolKind::Web { op, .. } => match op {
                WebOp::Search => quoted(
                    "🌐 Searched the web",
                    str_arg("query"),
                    count_prefix(result, "- "),
                ),
                WebOp::Fetch => format!("🌐 Read {}", domain_of(str_arg("url"))),
            },
            ToolKind::Memory { op } => match op {
                MemoryOp::Save => {
                    let kind = if str_arg("kind") == "rule" {
                        "rule"
                    } else {
                        "note"
                    };
                    format!("🧠 Remembered a {kind}")
                }
                MemoryOp::Delete => "🧠 Forgot a memory".into(),
            },
        }
        .to_string()
            + if failed { " (failed)" } else { "" }
    }

    /// Present-tense label shown on Discord *while* the tool runs, so the user
    /// sees what the bot is doing (before any progress or the final summary).
    pub fn active_label(&self, args: &Value) -> String {
        let str_arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
        match &self.kind {
            ToolKind::Drive { op, .. } => match op {
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
            },
            ToolKind::Web { op, .. } => match op {
                WebOp::Search => "🌐 Searching the web…".into(),
                WebOp::Fetch => format!("🌐 Reading {}…", domain_of(str_arg("url"))),
            },
            ToolKind::Memory { op } => match op {
                MemoryOp::Save => "🧠 Saving a memory…".into(),
                MemoryOp::Delete => "🧠 Forgetting a memory…".into(),
            },
        }
    }

    /// URLs this call surfaced, for the reply's "Sources" header. A fetch's
    /// source is its `url` arg; a search's are the result URLs. Empty for
    /// non-web tools.
    pub fn source_urls(&self, args: &Value, result: &str) -> Vec<String> {
        match &self.kind {
            ToolKind::Web {
                op: WebOp::Fetch, ..
            } => args
                .get("url")
                .and_then(Value::as_str)
                .filter(|u| u.starts_with("http"))
                .map(|u| vec![u.to_string()])
                .unwrap_or_default(),
            ToolKind::Web {
                op: WebOp::Search, ..
            } => result
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
                        kind: ToolKind::Web {
                            op,
                            api_key: instance.api_key.clone(),
                        },
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

/// One progress report from a long-running tool: a human-readable `label` (the
/// left-side status / Discord message) plus an optional short `detail` — a
/// quantitative note like "42%" shown on the right side of the app footer.
pub struct ProgressUpdate {
    pub label: String,
    pub detail: Option<String>,
}

/// Run a resolved tool call; returns a result string (ok or `error: …`).
/// A sink for progress reports from a long-running tool, shown live on Discord
/// and in the app footer. `report`/`report_with` are no-ops when there's no
/// receiver.
#[derive(Clone)]
pub struct Progress(Option<tokio::sync::mpsc::UnboundedSender<ProgressUpdate>>);

impl Progress {
    /// A reporter plus the receiver `discord.rs` drains to edit the status message.
    pub fn channel() -> (Self, tokio::sync::mpsc::UnboundedReceiver<ProgressUpdate>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (Progress(Some(tx)), rx)
    }

    /// Report a status label with no quantitative detail.
    pub fn report(&self, label: impl Into<String>) {
        self.send(label.into(), None);
    }

    /// Report a status label plus a short right-side detail (e.g. "42%").
    pub fn report_with(&self, label: impl Into<String>, detail: impl Into<String>) {
        self.send(label.into(), Some(detail.into()));
    }

    fn send(&self, label: String, detail: Option<String>) {
        if let Some(tx) = &self.0 {
            let _ = tx.send(ProgressUpdate { label, detail });
        }
    }
}

pub async fn execute(
    app: &AppHandle,
    bot_id: &str,
    tool: &ResolvedTool,
    args: &Value,
    progress: &Progress,
) -> String {
    let arg = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };

    match &tool.kind {
        ToolKind::Drive {
            op,
            client_id,
            client_secret,
            folder_id,
            ..
        } => {
            let (cid, secret, folder) = (client_id, client_secret, folder_id);
            let storage = crate::compose::drive::compose_drive_storage(app, cid, secret, folder);
            use crate::infrastructure::driving::drive as drive_ui;
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
                        &tool.instance_id,
                        &arg("question"),
                        k,
                    )
                    .await
                }
                DriveOp::ListSources => {
                    crate::infrastructure::driving::knowledge::list_sources(app, &tool.instance_id)
                        .await
                }
                DriveOp::Reindex => {
                    let Some(bot) = config::load_bot(app, bot_id) else {
                        return "error: bot config not found".to_string();
                    };
                    crate::infrastructure::driving::knowledge::reindex(
                        app,
                        &bot,
                        &tool.instance_id,
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
                // Backfill needs Discord history, so it's intercepted in
                // discord.rs `run_tool` before reaching here.
                DriveOp::Backfill => "error: backfill must run with channel context".to_string(),
                DriveOp::SaveLink => {
                    let Some(bot) = config::load_bot(app, bot_id) else {
                        return "error: bot config not found".to_string();
                    };
                    save_link(
                        app,
                        &bot,
                        &tool.instance_id,
                        cid,
                        secret,
                        folder,
                        &arg("url"),
                    )
                    .await
                }
                // Transcription runs as a background job with channel context, so
                // it's intercepted in discord.rs `run_tool` before reaching here.
                DriveOp::TranscribeLink => {
                    "error: transcription must run with channel context".to_string()
                }
            }
        }
        ToolKind::Web { op, api_key } => match op {
            WebOp::Search => {
                match crate::infrastructure::driving::web::search(api_key, &arg("query")).await {
                    Ok(results) => results,
                    Err(e) => format!("error: {e}"),
                }
            }
            WebOp::Fetch => {
                match crate::infrastructure::driving::web::fetch(api_key, &arg("url")).await {
                    Ok(content) => content,
                    Err(e) => format!("error: {e}"),
                }
            }
        },
        ToolKind::Memory { op } => match op {
            MemoryOp::Save => {
                crate::infrastructure::driving::memory::save(
                    app,
                    bot_id,
                    &arg("kind"),
                    &arg("text"),
                )
                .await
            }
            MemoryOp::Delete => {
                crate::infrastructure::driving::memory::delete(app, bot_id, &arg("id"));
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
                crate::infrastructure::driving::memory::guidance(
                    &crate::infrastructure::driving::memory::load(app, &bot.id),
                )
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
                tokio::task::spawn_blocking(move || {
                    ingest::extract_text(&extract_bytes, &filename, &mime)
                })
                .await
                .ok()
                .flatten()
            };

            // Semantic foldering: pick a subfolder (rule-guided), else the root.
            let target = choose_folder(
                app,
                bot,
                &guidance,
                context,
                client_id,
                client_secret,
                folder_id,
                &att.filename,
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

/// Store a bot-generated text file (e.g. a transcript or its summary) into a
/// Drive sink's folder and index it into the knowledge base. Returns the new
/// Drive file id. Best-effort; logs and returns `None` on failure.
pub async fn store_text_artifact(
    app: &AppHandle,
    bot: &BotConfig,
    sink: &AttachmentSink,
    filename: &str,
    content: &str,
    context: &str,
) -> Option<String> {
    let AttachmentSink::Drive {
        instance_id,
        instance_name,
        client_id,
        client_secret,
        folder_id,
    } = sink;

    let guidance = if bot.memory_enabled {
        crate::infrastructure::driving::memory::guidance(
            &crate::infrastructure::driving::memory::load(app, &bot.id),
        )
    } else {
        String::new()
    };
    let target = choose_folder(
        app,
        bot,
        &guidance,
        context,
        client_id,
        client_secret,
        folder_id,
        filename,
    )
    .await;

    let drive_id =
        match gdrive::create(app, client_id, client_secret, &target, filename, content).await {
            Ok(id) => id,
            Err(e) => {
                bot::emit_log(app, &bot.id, format!("store \"{filename}\": failed: {e}"));
                return None;
            }
        };
    bot::emit_tool_activity(
        app,
        &bot.id,
        format!("store_artifact {{name={filename:?}, to={instance_name:?}}} → id={drive_id}"),
        format!("💾 Saved \"{filename}\" to {instance_name}"),
    );
    index_text(
        app,
        bot,
        instance_id,
        &drive_id,
        filename,
        "text/markdown",
        content,
    )
    .await;
    Some(drive_id)
}

/// Copy a Drive file (from a link/id) into the tool's folder and index it.
#[allow(clippy::too_many_arguments)]
async fn save_link(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    client_id: &str,
    client_secret: &str,
    folder: &str,
    url: &str,
) -> String {
    let id = gdrive::file_id_from_link(url);
    if id.trim().is_empty() {
        return "error: no Google Drive link or id provided".to_string();
    }
    let meta = match gdrive::file_meta(app, client_id, client_secret, &id).await {
        Ok(m) => m,
        Err(e) => {
            return format!(
                "error: can't access that link ({e}). It must be shared with this bot's Google \
                 account (or set to 'anyone with the link')."
            )
        }
    };
    let new_id = match gdrive::copy_to(app, client_id, client_secret, &id, folder, None).await {
        Ok(nid) => nid,
        Err(e) => return format!("error: couldn't copy the file: {e}"),
    };
    let mut note = format!("saved \"{}\" to the folder (id={new_id})", meta.name);
    if let Ok(text) = gdrive::read(app, client_id, client_secret, &new_id).await {
        index_text(
            app,
            bot,
            instance_id,
            &new_id,
            &meta.name,
            &meta.mime_type,
            &text,
        )
        .await;
        note.push_str(" and indexed it into the knowledge base");
    }
    note
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
                (i.saturating_sub(1) as u32 * crate::audio::CHUNK_SECS) / 60
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
            crate::audio::CHUNK_SECS,
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
                index_text(app, bot, instance_id, &fid, fname, "text/markdown", content).await;
            }
            Err(e) => bot::emit_log(app, &bot.id, format!("save \"{fname}\": {e}")),
        }
    }
    Ok(files)
}

/// Pick the best subfolder for a file (rule-guided model classification), or the
/// root when there are no subfolders / no clear match.
#[allow(clippy::too_many_arguments)]
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
    let subfolders = gdrive::list_folders(app, client_id, client_secret, root)
        .await
        .unwrap_or_default();
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
            bot::emit_log(
                app,
                &bot.id,
                format!("index: embed failed for \"{name}\": {e}"),
            );
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
        Ok(()) => bot::emit_log(
            app,
            &bot.id,
            format!("indexed \"{name}\" into the knowledge base"),
        ),
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
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("read failed: {e}"))
}

// --- Helpers ----------------------------------------------------------------

/// A unique, readable per-instance prefix, falling back to `fallback`.
fn unique_slug(name: &str, fallback: &str, used: &mut HashSet<String>) -> String {
    let base = {
        let s = slugify(name);
        if s.is_empty() {
            fallback.to_string()
        } else {
            s
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_normalizes() {
        assert_eq!(slugify("Case Files!"), "case_files");
        assert_eq!(slugify("  a--b  "), "a_b");
        assert_eq!(slugify("!!!"), "");
    }

    #[test]
    fn domain_strips_scheme_and_www() {
        assert_eq!(domain_of("https://www.example.com/path"), "example.com");
        assert_eq!(domain_of("http://sub.host.org"), "sub.host.org");
    }

    #[test]
    fn count_prefix_counts_matching_lines() {
        assert_eq!(count_prefix("- a\n- b\nx", "- "), 2);
    }

    #[test]
    fn quoted_formats_with_and_without_query() {
        assert_eq!(
            quoted("Searched", "cats", 3),
            "Searched for \"cats\" — 3 result(s)"
        );
        assert_eq!(quoted("Searched", "  ", 0), "Searched — 0 result(s)");
    }

    #[test]
    fn unique_slug_dedupes() {
        let mut used = std::collections::HashSet::new();
        assert_eq!(unique_slug("Drive", "d", &mut used), "drive");
        assert_eq!(unique_slug("Drive", "d", &mut used), "drive2");
    }

    #[test]
    fn drive_op_flags_and_suffix() {
        assert!(DriveOp::Create.write());
        assert!(!DriveOp::Search.write());
        assert_eq!(DriveOp::Ask.suffix(), "ask");
    }
}
