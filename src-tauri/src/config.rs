//! Bot configuration, persisted via `tauri-plugin-store`.
//!
//! Stored as a single `config` object in `settings.json` (app data dir) so the
//! Settings UI (`src/lib/config.ts`) and this backend share one serde shape.
//! Read fresh each time the bot starts.

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
