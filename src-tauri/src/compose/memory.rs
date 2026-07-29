//! Compose the memory slices. The store is per-bot (keyed by id); `SaveMemory`
//! additionally needs the bot's model (for consolidation) and budget caps.

use std::sync::Arc;

use tauri::AppHandle;

use crate::application::ports::memory::MemoryStore;
use crate::application::usecases::save_memory::SaveMemory;
use crate::infrastructure::config::BotConfig;
use crate::infrastructure::driven::memory_consolidator::ModelConsolidator;
use crate::infrastructure::driven::memory_store::TauriMemoryStore;

pub fn compose_memory_store(app: &AppHandle, bot_id: &str) -> Arc<dyn MemoryStore> {
    Arc::new(TauriMemoryStore::new(app.clone(), bot_id.to_string()))
}

pub fn compose_save_memory(app: &AppHandle, bot: &BotConfig) -> SaveMemory {
    let store = compose_memory_store(app, &bot.id);
    let consolidator = Arc::new(ModelConsolidator::new(bot.clone()));
    SaveMemory::new(
        store,
        consolidator,
        bot.memory_max_notes,
        bot.memory_char_budget,
    )
}
