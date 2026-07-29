//! Compose the knowledge slices (ask / reindex). Per tool instance; reindex also
//! needs the instance's Drive storage.

use std::sync::Arc;

use tauri::AppHandle;

use crate::application::ports::knowledge::{Embeddings, KnowledgeIndex};
use crate::application::usecases::ask_knowledge::AskKnowledge;
use crate::application::usecases::reindex_knowledge::ReindexKnowledge;
use crate::compose::drive::compose_drive_storage;
use crate::infrastructure::config::BotConfig;
use crate::infrastructure::driven::embeddings::ModelEmbeddings;
use crate::infrastructure::driven::knowledge_index::SqliteKnowledgeIndex;

pub fn compose_knowledge_index(app: &AppHandle, instance_id: &str) -> Arc<dyn KnowledgeIndex> {
    Arc::new(SqliteKnowledgeIndex::new(
        app.clone(),
        instance_id.to_string(),
    ))
}

pub fn compose_embeddings(bot: &BotConfig) -> Arc<dyn Embeddings> {
    Arc::new(ModelEmbeddings::new(bot.clone()))
}

pub fn compose_ask_knowledge(app: &AppHandle, bot: &BotConfig, instance_id: &str) -> AskKnowledge {
    AskKnowledge::new(
        compose_embeddings(bot),
        compose_knowledge_index(app, instance_id),
    )
}

pub fn compose_reindex_knowledge(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
) -> ReindexKnowledge {
    ReindexKnowledge::new(
        compose_drive_storage(app, client_id, client_secret, folder_id),
        compose_embeddings(bot),
        compose_knowledge_index(app, instance_id),
    )
}
