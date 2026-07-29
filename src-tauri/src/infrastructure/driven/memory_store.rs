//! Driven adapter: `MemoryStore` backed by the tauri plugin-store, keyed by bot
//! id. Owns the JSON shape (via `MemoryDto`) and mints ids/timestamps.

use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::application::ports::memory::MemoryStore;
use crate::domain::memory::{Memory, MemoryKind};
use crate::infrastructure::config::{self, STORE_FILE};
use crate::infrastructure::dto::memory::MemoryDto;
use crate::infrastructure::shared::time::now_ms;

const MEMORIES_KEY: &str = "memories";

pub struct TauriMemoryStore {
    app: AppHandle,
    bot_id: String,
}

impl TauriMemoryStore {
    pub fn new(app: AppHandle, bot_id: String) -> Self {
        Self { app, bot_id }
    }
}

impl MemoryStore for TauriMemoryStore {
    fn load(&self) -> Vec<Memory> {
        let Ok(store) = self.app.store(STORE_FILE) else {
            return Vec::new();
        };
        let dtos: Vec<MemoryDto> = store
            .get(MEMORIES_KEY)
            .and_then(|v| v.get(&self.bot_id).cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        dtos.into_iter().map(MemoryDto::into_domain).collect()
    }

    fn store_all(&self, memories: &[Memory]) {
        let Ok(store) = self.app.store(STORE_FILE) else {
            return;
        };
        let dtos: Vec<MemoryDto> = memories.iter().map(MemoryDto::from_domain).collect();
        let mut all = store
            .get(MEMORIES_KEY)
            .unwrap_or_else(|| serde_json::json!({}));
        if let Some(map) = all.as_object_mut() {
            map.insert(self.bot_id.clone(), serde_json::json!(dtos));
        }
        store.set(MEMORIES_KEY, all);
        let _ = store.save();
    }

    fn mint(&self, kind: MemoryKind, text: String) -> Memory {
        Memory {
            id: config::new_id("mem"),
            kind,
            text,
            created: now_ms(),
        }
    }
}
