//! The web-search tool: `search` + `fetch` ops, executed via the web driving
//! adapter (Keenable behind the `WebSearch`/`WebFetch` ports). This module owns
//! everything that makes a web tool a web tool: its config schema (`KIND`,
//! `ready`), its ops, and the footer states it shows while running.

use serde_json::Value;

use super::manifest::{ManifestField, ManifestOp, ToolManifest};
use crate::infrastructure::config::ToolInstance;

/// The persisted `ToolInstance.type` this module handles.
pub const KIND: &str = "web_search";
/// Slug fallback when the instance name doesn't yield one.
pub const SLUG: &str = "web";

/// A web tool is usable once it carries a provider API key.
pub fn ready(instance: &ToolInstance) -> bool {
    !instance.api_key.trim().is_empty()
}

/// This tool's schema — config field + ops — for the frontend.
pub fn manifest() -> ToolManifest {
    ToolManifest {
        kind: KIND,
        label: "Web Search",
        icon: "🔎",
        oauth: false,
        config_caption: Some("Create an API key at keenable.ai/console. Stored locally."),
        config_fields: vec![ManifestField {
            key: "apiKey",
            label: "API key",
            secret: true,
            number: false,
        }],
        ops: WebOp::ALL
            .iter()
            .map(|o| ManifestOp {
                op: o.suffix(),
                label: o.label(),
                write: false,
            })
            .collect(),
    }
}

#[derive(Clone, Copy)]
pub enum WebOp {
    Search,
    Fetch,
}

impl WebOp {
    pub const ALL: [WebOp; 2] = [WebOp::Search, WebOp::Fetch];

    pub fn suffix(self) -> &'static str {
        match self {
            WebOp::Search => "search",
            WebOp::Fetch => "fetch",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            WebOp::Search => "Search the web; returns a list of results (title, url, excerpt).",
            WebOp::Fetch => "Fetch a web page by url and return its main text content.",
        }
    }

    /// Short label for the approvals UI (mirrors the frontend's old `TOOL_OPS`).
    pub fn label(self) -> &'static str {
        match self {
            WebOp::Search => "Web search",
            WebOp::Fetch => "Fetch a page",
        }
    }

    pub fn args(self) -> &'static str {
        match self {
            WebOp::Search => "{\"query\": string}",
            WebOp::Fetch => "{\"url\": string}",
        }
    }

    /// Present-tense left-footer label shown while the op runs.
    pub(super) fn active_label(self, args: &Value) -> String {
        let str_arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
        match self {
            WebOp::Search => "🌐 Searching the web…".into(),
            WebOp::Fetch => format!("🌐 Reading {}…", super::domain_of(str_arg("url"))),
        }
    }

    /// Past-tense one-liner for the folded activity feed (no failure suffix —
    /// the caller appends it).
    pub(super) fn summary(self, args: &Value, result: &str) -> String {
        let str_arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
        match self {
            WebOp::Search => super::quoted(
                "🌐 Searched the web",
                str_arg("query"),
                super::count_prefix(result, "- "),
            ),
            WebOp::Fetch => format!("🌐 Read {}", super::domain_of(str_arg("url"))),
        }
    }
}

/// Execute a web op with the instance's API key.
pub(super) async fn execute(op: WebOp, api_key: &str, args: &Value) -> String {
    let arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
    match op {
        WebOp::Search => {
            match crate::infrastructure::driving::web::search(api_key, arg("query")).await {
                Ok(results) => results,
                Err(e) => format!("error: {e}"),
            }
        }
        WebOp::Fetch => match crate::infrastructure::driving::web::fetch(api_key, arg("url")).await
        {
            Ok(content) => content,
            Err(e) => format!("error: {e}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_needs_api_key() {
        let mut t = ToolInstance::default();
        assert!(!ready(&t));
        t.api_key = "k".into();
        assert!(ready(&t));
    }
}
