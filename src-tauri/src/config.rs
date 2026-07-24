//! Bot configuration, persisted via `tauri-plugin-store`.
//!
//! Stored as a single `config` object in `settings.json` (app data dir) so the
//! Settings UI (`src/lib/config.ts`) and this backend share one serde shape.
//! Read fresh each time the bot starts.

use std::collections::HashMap;

use serde::Deserialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

pub const STORE_FILE: &str = "settings.json";
pub const CONFIG_KEY: &str = "config";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BotConfig {
    pub discord_token: String,
    pub model_base_url: String,
    pub model_name: String,
    /// Empty means "no API key" (typical for local servers).
    pub api_key: String,
    pub system_prompt: String,
    pub followup_window_messages: u32,
    pub followup_window_secs: u64,

    // Google Drive (OAuth desktop client + target folder).
    pub google_client_id: String,
    pub google_client_secret: String,
    pub drive_folder_id: String,

    /// Per-tool policy: tool name -> "allow" | "ask" | "deny". Missing = the
    /// tool's built-in default (read/search = allow, writes = ask).
    pub tool_policies: HashMap<String, String>,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            discord_token: String::new(),
            model_base_url: "http://127.0.0.1:8080/v1".into(),
            model_name: String::new(),
            api_key: String::new(),
            system_prompt: "You are openbot, a helpful assistant in a Discord server. \
                            Keep replies concise."
                .into(),
            followup_window_messages: 5,
            followup_window_secs: 180,
            google_client_id: String::new(),
            google_client_secret: String::new(),
            drive_folder_id: String::new(),
            tool_policies: HashMap::new(),
        }
    }
}

impl BotConfig {
    /// Everything required to actually run the bot is present.
    pub fn is_ready(&self) -> bool {
        !self.discord_token.trim().is_empty()
            && !self.model_base_url.trim().is_empty()
            && !self.model_name.trim().is_empty()
    }

    /// OpenAI-compatible chat completions endpoint.
    pub fn chat_url(&self) -> String {
        format!(
            "{}/chat/completions",
            self.model_base_url.trim_end_matches('/')
        )
    }

    /// Enough Google Drive config present to attempt a connection.
    pub fn drive_ready(&self) -> bool {
        !self.google_client_id.trim().is_empty()
            && !self.google_client_secret.trim().is_empty()
            && !self.drive_folder_id.trim().is_empty()
    }
}

/// Load the saved config, falling back to defaults if the store or key is
/// missing or malformed.
pub fn load<R: Runtime>(app: &AppHandle<R>) -> BotConfig {
    let Ok(store) = app.store(STORE_FILE) else {
        return BotConfig::default();
    };
    store
        .get(CONFIG_KEY)
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

/// Persist a single tool policy (`allow`/`ask`/`deny`) into the stored config,
/// leaving all other fields untouched. Used by the "always allow/deny" buttons.
pub fn set_tool_policy<R: Runtime>(app: &AppHandle<R>, tool: &str, policy: &str) {
    let Ok(store) = app.store(STORE_FILE) else {
        return;
    };
    let mut config = store.get(CONFIG_KEY).unwrap_or_else(|| serde_json::json!({}));
    if !config.is_object() {
        config = serde_json::json!({});
    }
    let obj = config.as_object_mut().unwrap();
    let policies = obj
        .entry("toolPolicies")
        .or_insert_with(|| serde_json::json!({}));
    if let Some(map) = policies.as_object_mut() {
        map.insert(tool.to_string(), serde_json::json!(policy));
    }
    store.set(CONFIG_KEY, config);
    let _ = store.save();
}
