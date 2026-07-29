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

use crate::infrastructure::config::{BotConfig, GlobalConfig};

// One module per tool — each owns its `Op` enum, metadata, and execution.
mod drive;
mod memory;
mod web;

use drive::DriveOp;
use memory::MemoryOp;
use web::WebOp;

/// A Drive-link transcription background job. Re-exported for `discord.rs`,
/// which runs it with channel context.
pub use drive::run_transcription;

/// Fixed instance id for the per-bot memory tools.
const MEMORY_INSTANCE: &str = "memory";

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
    /// (non-verbose) activity feed. Each tool owns its own wording; this just
    /// dispatches and appends a failure marker.
    pub fn summary(&self, args: &Value, result: &str) -> String {
        let base = match &self.kind {
            ToolKind::Drive { op, .. } => op.summary(args, result),
            ToolKind::Web { op, .. } => op.summary(args, result),
            ToolKind::Memory { op } => op.summary(args),
        };
        base + if result.starts_with("error:") {
            " (failed)"
        } else {
            ""
        }
    }

    /// Present-tense label shown on Discord *while* the tool runs, so the user
    /// sees what the bot is doing (before any progress or the final summary).
    /// Owned per-tool; this dispatches to the active op.
    pub fn active_label(&self, args: &Value) -> String {
        match &self.kind {
            ToolKind::Drive { op, .. } => op.active_label(),
            ToolKind::Web { op, .. } => op.active_label(args),
            ToolKind::Memory { op } => op.active_label(),
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
            drive::KIND if drive::ready(instance) => {
                let slug = unique_slug(&instance.name, drive::SLUG, &mut used_slugs);
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
            web::KIND if web::ready(instance) => {
                let slug = unique_slug(&instance.name, web::SLUG, &mut used_slugs);
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

/// A tool call parsed out of the model's ReAct output. (Tools are the boundary
/// between the model and the app, so their call format lives here, not in the
/// hexagon.)
pub struct ToolCall {
    pub tool: String,
    pub args: Value,
}

/// Find a `TOOL_CALL { … }` directive in the model's text and extract the tool
/// name + args. Tolerates surrounding text and trailing content after the JSON.
pub fn parse_tool_call(text: &str) -> Option<ToolCall> {
    let idx = text.find("TOOL_CALL")?;
    let after = &text[idx + "TOOL_CALL".len()..];
    let brace = after.find('{')?;
    // Read exactly one JSON value starting at the first brace, ignoring the rest.
    let value = serde_json::Deserializer::from_str(&after[brace..])
        .into_iter::<Value>()
        .next()?
        .ok()?;
    let tool = value.get("tool")?.as_str()?.to_string();
    let args = value
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    Some(ToolCall { tool, args })
}

/// Run a resolved tool call; returns a result string (ok or `error: …`).
/// Dispatches to the owning tool module.
pub async fn execute(
    app: &AppHandle,
    bot_id: &str,
    tool: &ResolvedTool,
    args: &Value,
    progress: &Progress,
) -> String {
    match &tool.kind {
        ToolKind::Drive {
            op,
            client_id,
            client_secret,
            folder_id,
            ..
        } => {
            drive::execute(
                app,
                bot_id,
                &tool.instance_id,
                *op,
                client_id,
                client_secret,
                folder_id,
                args,
                progress,
            )
            .await
        }
        ToolKind::Web { op, api_key } => web::execute(*op, api_key, args).await,
        ToolKind::Memory { op } => memory::execute(app, bot_id, *op, args).await,
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
            if instance.kind == drive::KIND && drive::ready(instance) {
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
pub(super) fn quoted(prefix: &str, query: &str, count: usize) -> String {
    let query = query.trim();
    let head = if query.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix} for \"{query}\"")
    };
    format!("{head} — {count} result(s)")
}

/// Count result lines beginning with `prefix` (one per item in our formats).
pub(super) fn count_prefix(result: &str, prefix: &str) -> usize {
    result.lines().filter(|l| l.starts_with(prefix)).count()
}

/// The host of a URL (without scheme or leading `www.`), for a compact summary.
pub(super) fn domain_of(url: &str) -> String {
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
    fn parse_tool_call_extracts_name_and_args() {
        let t =
            "sure\nTOOL_CALL {\"tool\": \"drive_search\", \"args\": {\"query\": \"x\"}} trailing";
        let c = parse_tool_call(t).unwrap();
        assert_eq!(c.tool, "drive_search");
        assert_eq!(c.args["query"], "x");
    }

    #[test]
    fn parse_tool_call_none_when_absent() {
        assert!(parse_tool_call("just a normal reply").is_none());
    }

    #[test]
    fn parse_tool_call_defaults_missing_args() {
        let c = parse_tool_call("TOOL_CALL {\"tool\": \"x\"}").unwrap();
        assert_eq!(c.tool, "x");
        assert!(c.args.is_object());
    }

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
