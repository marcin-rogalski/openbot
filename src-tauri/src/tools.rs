//! Built-in tool catalog + dispatch. Google Drive is the first provider; the
//! ReAct loop in `discord.rs` builds the prompt from [`prompt_section`] and runs
//! parsed calls through [`execute`]. Generic MCP providers can join later behind
//! the same shape.

use serde_json::Value;
use tauri::AppHandle;

use crate::config::BotConfig;
use crate::gdrive::{self, DriveFile};

pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// Human-readable args schema, shown in the prompt.
    pub args: &'static str,
    /// Write/destructive tools default to the `ask` policy.
    pub write: bool,
}

pub const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "drive_search",
        description: "Full-text search files in the connected Google Drive folder.",
        args: "{\"query\": string}",
        write: false,
    },
    ToolSpec {
        name: "drive_list",
        description: "List the files in the folder.",
        args: "{}",
        write: false,
    },
    ToolSpec {
        name: "drive_read",
        description: "Read a file's text content by id (from search/list results).",
        args: "{\"id\": string}",
        write: false,
    },
    ToolSpec {
        name: "drive_create",
        description: "Create a new plain-text file in the folder.",
        args: "{\"name\": string, \"content\": string}",
        write: true,
    },
    ToolSpec {
        name: "drive_update",
        description: "Replace a file's content by id.",
        args: "{\"id\": string, \"content\": string}",
        write: true,
    },
    ToolSpec {
        name: "drive_delete",
        description: "Move a file to trash by id.",
        args: "{\"id\": string}",
        write: true,
    },
];

pub fn find(name: &str) -> Option<&'static ToolSpec> {
    TOOLS.iter().find(|t| t.name == name)
}

/// Are any tools usable right now? (Drive configured.)
pub fn available(cfg: &BotConfig) -> bool {
    cfg.drive_ready()
}

/// The tools section appended to the system prompt.
pub fn prompt_section() -> String {
    let mut section = String::from(
        "\n\nYou have tools. To use one, output EXACTLY one line and nothing else:\n\
         TOOL_CALL {\"tool\": \"<name>\", \"args\": { ... }}\n\
         You'll then receive a line `TOOL_RESULT: <result>`. You may call tools several times in \
         a row. When you have the final answer, reply normally WITHOUT a TOOL_CALL line. \
         Available tools:\n",
    );
    for tool in TOOLS {
        section.push_str(&format!(
            "- {} — {} args: {}\n",
            tool.name, tool.description, tool.args
        ));
    }
    section
}

/// Run a tool call; returns a result string (ok or `error: …`) for TOOL_RESULT.
pub async fn execute(app: &AppHandle, cfg: &BotConfig, name: &str, args: &Value) -> String {
    let arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    match name {
        "drive_search" => match gdrive::search(app, cfg, &arg("query")).await {
            Ok(files) => format_files(&files),
            Err(e) => format!("error: {e}"),
        },
        "drive_list" => match gdrive::list(app, cfg).await {
            Ok(files) => format_files(&files),
            Err(e) => format!("error: {e}"),
        },
        "drive_read" => match gdrive::read(app, cfg, &arg("id")).await {
            Ok(text) => truncate(&text, 6000),
            Err(e) => format!("error: {e}"),
        },
        "drive_create" => match gdrive::create(app, cfg, &arg("name"), &arg("content")).await {
            Ok(id) => format!("created file id={id}"),
            Err(e) => format!("error: {e}"),
        },
        "drive_update" => match gdrive::update(app, cfg, &arg("id"), &arg("content")).await {
            Ok(()) => "updated".to_string(),
            Err(e) => format!("error: {e}"),
        },
        "drive_delete" => match gdrive::trash(app, cfg, &arg("id")).await {
            Ok(()) => "moved to trash".to_string(),
            Err(e) => format!("error: {e}"),
        },
        _ => format!("error: unknown tool '{name}'"),
    }
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

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push_str("…[truncated]");
    out
}
