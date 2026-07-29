//! The transcription capability, as a tool. Event-driven (no model ops): when
//! bound to a bot, audio posted in conversations is transcribed and the bot
//! posts a transcript + summary (indexed into the knowledge base when a Drive
//! tool is bound). The transcription *model* is model-server config and stays on
//! the bot's Model tab (alongside the embedding model); this tool is the on/off.

use super::manifest::ToolManifest;
use crate::infrastructure::config;

/// The persisted `ToolInstance.type` this module handles. No readiness gate —
/// the model lives on the bot's Model config; binding is the whole switch.
pub const KIND: &str = config::KIND_TRANSCRIPTION;

pub fn manifest() -> ToolManifest {
    ToolManifest {
        kind: KIND,
        label: "Transcription",
        icon: "🎙️",
        oauth: false,
        config_caption: Some(
            "Audio posted in conversations is transcribed (via the bot's transcription model); \
             the bot replies with a transcript + summary, and indexes them when Drive is bound.",
        ),
        config_fields: vec![],
        ops: vec![],
    }
}
