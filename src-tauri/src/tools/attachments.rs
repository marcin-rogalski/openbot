//! The attachments capability, as a tool. Event-driven (no model ops): when
//! bound to a bot, files posted in conversations the bot takes part in are
//! forwarded to subscribed sinks (e.g. a bound Drive folder). Binding = on.

use super::manifest::ToolManifest;
use crate::infrastructure::config;

/// The persisted `ToolInstance.type` this module handles. No readiness gate —
/// binding the instance is the whole switch.
pub const KIND: &str = config::KIND_ATTACHMENTS;

pub fn manifest() -> ToolManifest {
    ToolManifest {
        kind: KIND,
        label: "Attachments",
        icon: "📎",
        oauth: false,
        config_caption: Some(
            "Files posted in conversations are read inline and forwarded to bound tools \
             (e.g. Drive archives relevant ones, guided by memory rules).",
        ),
        config_fields: vec![],
        ops: vec![],
    }
}
