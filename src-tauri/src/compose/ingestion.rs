//! Compose the ingestion slices (attachment gate + shared indexing). Per tool
//! instance (its Drive credentials/folder + the bot's models).

use std::sync::Arc;

use tauri::AppHandle;

use crate::application::ports::ingestion::ArchivePolicy;
use crate::application::usecases::archive_attachment::ArchiveAttachment;
use crate::application::usecases::index_document::IndexDocument;
use crate::compose::drive::compose_drive_storage;
use crate::compose::knowledge::{compose_embeddings, compose_knowledge_index};
use crate::config::BotConfig;
use crate::infrastructure::driven::http_fetcher::HttpFileFetcher;
use crate::infrastructure::driven::ingest_extractor::IngestTextExtractor;
use crate::infrastructure::driven::model_archive_policy::ModelArchivePolicy;

pub fn compose_archive_policy(bot: &BotConfig) -> Arc<dyn ArchivePolicy> {
    Arc::new(ModelArchivePolicy::new(bot.clone()))
}

pub fn compose_index_document(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
) -> IndexDocument {
    IndexDocument::new(
        compose_embeddings(bot),
        compose_knowledge_index(app, instance_id),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn compose_archive_attachment(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
) -> ArchiveAttachment {
    ArchiveAttachment::new(
        compose_archive_policy(bot),
        Arc::new(HttpFileFetcher),
        Arc::new(IngestTextExtractor),
        compose_drive_storage(app, client_id, client_secret, folder_id),
        compose_index_document(app, bot, instance_id),
    )
}
