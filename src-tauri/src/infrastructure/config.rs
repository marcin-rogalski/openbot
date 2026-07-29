//! App configuration, persisted via `tauri-plugin-store` in `settings.json`.
//!
//! Two top-level keys: `global` ([`GlobalConfig`] — the Google sign-in + a
//! registry of reusable tool instances) and `bots` (a list of [`BotConfig`],
//! each with its own model server and a selection of tool instances). The old
//! single `config` object is migrated into one bot on first load.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

pub const STORE_FILE: &str = "settings.json";
const GLOBAL_KEY: &str = "global";
const BOTS_KEY: &str = "bots";
const LEGACY_CONFIG_KEY: &str = "config";

/// A few pleasant, distinct avatar colors to cycle through for new bots.
pub const BOT_COLORS: &[&str] = &[
    "#4c8bf5", "#22a06b", "#e0603b", "#9b5de5", "#f2a900", "#e5487f", "#00b8d9",
];

// --- Global config ----------------------------------------------------------

/// A reusable, globally-configured tool instance. A Google Drive tool is fully
/// self-contained: it carries its own OAuth client (integration) and folder. A
/// Web Search tool carries a single provider API key.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolInstance {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    // Google Drive:
    pub client_id: String,
    pub client_secret: String,
    pub folder_id: String,
    // Web Search (Keenable): a single API key.
    pub api_key: String,
}

impl Default for ToolInstance {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            kind: "google_drive".into(),
            client_id: String::new(),
            client_secret: String::new(),
            folder_id: String::new(),
            api_key: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GlobalConfig {
    pub tools: Vec<ToolInstance>,
    /// MCP servers — data-model placeholder; implementation deferred.
    pub mcp_servers: Vec<serde_json::Value>,
}

impl GlobalConfig {
    pub fn tool(&self, id: &str) -> Option<&ToolInstance> {
        self.tools.iter().find(|t| t.id == id)
    }
}

// --- Per-bot config ---------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelConfig {
    pub base_url: String,
    pub model_name: String,
    pub api_key: String,
    /// Embedding model served by the same `base_url` (`/embeddings`), used for the
    /// local knowledge index.
    pub embedding_model: String,
    /// Transcription model served by the same `base_url` (`/audio/transcriptions`),
    /// used to turn audio attachments into text.
    pub transcription_model: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:8080/v1".into(),
            model_name: String::new(),
            api_key: String::new(),
            embedding_model: "nomic-embed-text".into(),
            transcription_model: "whisper-1".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BotConfig {
    pub id: String,
    pub name: String,
    pub color: String,
    pub discord_token: String,
    pub model: ModelConfig,
    pub system_prompt: String,
    pub followup_window_messages: u32,
    pub followup_window_secs: u64,
    /// Ids of the global tool instances this bot may use.
    pub enabled_tool_ids: Vec<String>,
    /// Per-tool policy keyed by `"<toolInstanceId>/<op>"` → allow/ask/deny.
    pub tool_policies: HashMap<String, String>,
    /// When enabled, the bot gets `memory_save`/`memory_delete` tools and its
    /// stored memories are injected into the system prompt.
    pub memory_enabled: bool,
    /// Consolidate once memories exceed this count …
    pub memory_max_notes: u32,
    /// … or this many total characters, whichever comes first.
    pub memory_char_budget: u32,
    /// When enabled, attachments posted in conversations the bot takes part in
    /// are forwarded to subscribed tools (e.g. Drive archiving). Off = no live
    /// attachment gate (and no per-attachment model calls).
    pub attachments_enabled: bool,
    /// When enabled, audio attachments in conversations the bot takes part in are
    /// transcribed; the bot posts a transcript + summary (and indexes them into
    /// the knowledge base when a Drive tool is enabled).
    pub transcription_enabled: bool,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            id: new_id("bot"),
            name: "New bot".into(),
            color: BOT_COLORS[0].into(),
            discord_token: String::new(),
            model: ModelConfig::default(),
            system_prompt: "You are a helpful assistant in a Discord server. Keep replies \
                            concise."
                .into(),
            followup_window_messages: 5,
            followup_window_secs: 180,
            enabled_tool_ids: Vec::new(),
            tool_policies: HashMap::new(),
            memory_enabled: false,
            memory_max_notes: 40,
            memory_char_budget: 2000,
            attachments_enabled: true,
            transcription_enabled: true,
        }
    }
}

impl BotConfig {
    /// Everything required to actually run this bot is present.
    pub fn is_ready(&self) -> bool {
        !self.discord_token.trim().is_empty()
            && !self.model.base_url.trim().is_empty()
            && !self.model.model_name.trim().is_empty()
    }

    /// OpenAI-compatible chat completions endpoint.
    pub fn chat_url(&self) -> String {
        format!(
            "{}/chat/completions",
            self.model.base_url.trim_end_matches('/')
        )
    }
}

// --- Loading / migration ----------------------------------------------------

pub fn load_global<R: Runtime>(app: &AppHandle<R>) -> GlobalConfig {
    migrate_if_needed(app);
    let Ok(store) = app.store(STORE_FILE) else {
        return GlobalConfig::default();
    };
    store
        .get(GLOBAL_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

pub fn load_bots<R: Runtime>(app: &AppHandle<R>) -> Vec<BotConfig> {
    migrate_if_needed(app);
    let Ok(store) = app.store(STORE_FILE) else {
        return Vec::new();
    };
    store
        .get(BOTS_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

pub fn load_bot<R: Runtime>(app: &AppHandle<R>, bot_id: &str) -> Option<BotConfig> {
    load_bots(app).into_iter().find(|b| b.id == bot_id)
}

/// If the store has no `bots` yet, seed it — migrating the old single `config`
/// if present, otherwise creating one empty default bot — and persist once.
fn migrate_if_needed<R: Runtime>(app: &AppHandle<R>) {
    let Ok(store) = app.store(STORE_FILE) else {
        return;
    };
    if store.get(BOTS_KEY).is_some() {
        return;
    }

    let (global, bots) = match store.get(LEGACY_CONFIG_KEY) {
        Some(legacy) => migrate_legacy(&legacy),
        None => (GlobalConfig::default(), vec![BotConfig::default()]),
    };

    if let Ok(v) = serde_json::to_value(&global) {
        store.set(GLOBAL_KEY, v);
    }
    if let Ok(v) = serde_json::to_value(&bots) {
        store.set(BOTS_KEY, v);
    }
    let _ = store.save();
}

/// Build a global config + one bot from the old flat `config` shape.
fn migrate_legacy(legacy: &serde_json::Value) -> (GlobalConfig, Vec<BotConfig>) {
    let s = |key: &str| {
        legacy
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let u = |key: &str, default: u64| legacy.get(key).and_then(|v| v.as_u64()).unwrap_or(default);

    let mut global = GlobalConfig::default();

    let mut enabled_tool_ids = Vec::new();
    let folder = s("driveFolderId");
    if !folder.trim().is_empty() {
        let tool = ToolInstance {
            id: new_id("tool"),
            name: "Google Drive".into(),
            kind: "google_drive".into(),
            client_id: s("googleClientId"),
            client_secret: s("googleClientSecret"),
            folder_id: folder,
            ..ToolInstance::default()
        };
        enabled_tool_ids.push(tool.id.clone());
        global.tools.push(tool);
    }

    let bot = BotConfig {
        id: new_id("bot"),
        name: "openbot".into(),
        color: BOT_COLORS[0].into(),
        discord_token: s("discordToken"),
        model: ModelConfig {
            base_url: {
                let base = s("modelBaseUrl");
                if base.is_empty() {
                    ModelConfig::default().base_url
                } else {
                    base
                }
            },
            model_name: s("modelName"),
            api_key: s("apiKey"),
            ..ModelConfig::default()
        },
        system_prompt: {
            let p = s("systemPrompt");
            if p.is_empty() {
                BotConfig::default().system_prompt
            } else {
                p
            }
        },
        followup_window_messages: u("followupWindowMessages", 5) as u32,
        followup_window_secs: u("followupWindowSecs", 180),
        enabled_tool_ids,
        ..BotConfig::default()
    };

    (global, vec![bot])
}

/// Persist a single tool policy for one bot, leaving everything else untouched.
pub fn set_tool_policy<R: Runtime>(app: &AppHandle<R>, bot_id: &str, tool: &str, policy: &str) {
    let Ok(store) = app.store(STORE_FILE) else {
        return;
    };
    let Some(mut bots) = store.get(BOTS_KEY) else {
        return;
    };
    if let Some(array) = bots.as_array_mut() {
        for bot in array.iter_mut() {
            if bot.get("id").and_then(|v| v.as_str()) == Some(bot_id) {
                let policies = bot
                    .as_object_mut()
                    .unwrap()
                    .entry("toolPolicies")
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(map) = policies.as_object_mut() {
                    map.insert(tool.to_string(), serde_json::json!(policy));
                }
            }
        }
    }
    store.set(BOTS_KEY, bots);
    let _ = store.save();
}

/// Process-unique id with a readable prefix.
pub fn new_id(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{prefix}-{nanos}-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_config_defaults() {
        let m = ModelConfig::default();
        assert_eq!(m.base_url, "http://127.0.0.1:8080/v1");
        assert_eq!(m.embedding_model, "nomic-embed-text");
        assert_eq!(m.transcription_model, "whisper-1");
    }

    #[test]
    fn bot_defaults() {
        let b = BotConfig::default();
        assert!(b.attachments_enabled);
        assert!(b.transcription_enabled);
        assert!(!b.memory_enabled);
        assert_eq!(b.followup_window_messages, 5);
    }

    #[test]
    fn chat_url_trims_trailing_slash() {
        let mut b = BotConfig::default();
        b.model.base_url = "http://x/v1/".into();
        assert_eq!(b.chat_url(), "http://x/v1/chat/completions");
    }

    #[test]
    fn is_ready_requires_token_and_model() {
        let mut b = BotConfig::default();
        assert!(!b.is_ready());
        b.discord_token = "t".into();
        b.model.model_name = "m".into();
        assert!(b.is_ready());
    }

    #[test]
    fn new_id_unique_and_prefixed() {
        let a = new_id("bot");
        let b = new_id("bot");
        assert!(a.starts_with("bot-"));
        assert_ne!(a, b);
    }
}
