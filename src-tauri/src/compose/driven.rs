//! Driven-adapter composition — build the outbound adapters, each implementing an
//! application port. Second phase (after `commons`): depends only on the concrete
//! adapters + shared infra, and is consumed by `driving`. Adapters are per-bot /
//! per-tool-instance (credentialed at call time), so these build on demand.

use std::sync::Arc;

use tauri::AppHandle;

use crate::application::ports::chat_model::ChatModel;
use crate::application::ports::drive::DriveStorage;
use crate::application::ports::ingestion::{ArchivePolicy, FileFetcher, TextExtractor};
use crate::application::ports::knowledge::{Embeddings, KnowledgeIndex};
use crate::application::ports::memory::{MemoryConsolidator, MemoryStore};
use crate::application::ports::transcription::{AudioCodec, Summarizer, Transcriber};
use crate::application::ports::webfetch::WebFetch;
use crate::application::ports::websearch::WebSearch;
use crate::infrastructure::config::BotConfig;
use crate::infrastructure::driven::embeddings::ModelEmbeddings;
use crate::infrastructure::driven::gdrive_storage::GDriveStorage;
use crate::infrastructure::driven::http_fetcher::HttpFileFetcher;
use crate::infrastructure::driven::ingest_extractor::IngestTextExtractor;
use crate::infrastructure::driven::keenable::{KeenableFetch, KeenableSearch};
use crate::infrastructure::driven::knowledge_index::SqliteKnowledgeIndex;
use crate::infrastructure::driven::memory_consolidator::ModelConsolidator;
use crate::infrastructure::driven::memory_store::TauriMemoryStore;
use crate::infrastructure::driven::model_archive_policy::ModelArchivePolicy;
use crate::infrastructure::driven::model_chat::ModelChat;
use crate::infrastructure::driven::model_summarizer::ModelSummarizer;
use crate::infrastructure::driven::model_transcriber::ModelTranscriber;
use crate::infrastructure::driven::symphonia_codec::SymphoniaCodec;
use crate::infrastructure::shared::http;

// --- Model server -----------------------------------------------------------

pub fn chat_model(bot: &BotConfig) -> Arc<dyn ChatModel> {
    Arc::new(ModelChat::new(bot.clone()))
}

pub fn embeddings(bot: &BotConfig) -> Arc<dyn Embeddings> {
    Arc::new(ModelEmbeddings::new(bot.clone()))
}

pub fn transcriber(bot: &BotConfig) -> Arc<dyn Transcriber> {
    Arc::new(ModelTranscriber::new(bot.clone()))
}

pub fn summarizer(bot: &BotConfig) -> Arc<dyn Summarizer> {
    Arc::new(ModelSummarizer::new(bot.clone()))
}

pub fn archive_policy(bot: &BotConfig) -> Arc<dyn ArchivePolicy> {
    Arc::new(ModelArchivePolicy::new(bot.clone()))
}

pub fn memory_consolidator(bot: &BotConfig) -> Arc<dyn MemoryConsolidator> {
    Arc::new(ModelConsolidator::new(bot.clone()))
}

// --- Web (Keenable) ---------------------------------------------------------

pub fn web_search(api_key: &str) -> Arc<dyn WebSearch> {
    Arc::new(KeenableSearch::new(http::client(), api_key.to_string()))
}

pub fn web_fetch(api_key: &str) -> Arc<dyn WebFetch> {
    Arc::new(KeenableFetch::new(http::client(), api_key.to_string()))
}

// --- Google Drive -----------------------------------------------------------

pub fn drive_storage(
    app: &AppHandle,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
) -> Arc<dyn DriveStorage> {
    Arc::new(GDriveStorage::new(
        app.clone(),
        client_id.to_string(),
        client_secret.to_string(),
        folder_id.to_string(),
    ))
}

// --- Local knowledge index --------------------------------------------------

pub fn knowledge_index(app: &AppHandle, instance_id: &str) -> Arc<dyn KnowledgeIndex> {
    Arc::new(SqliteKnowledgeIndex::new(
        app.clone(),
        instance_id.to_string(),
    ))
}

// --- Memory store -----------------------------------------------------------

pub fn memory_store(app: &AppHandle, store_id: &str) -> Arc<dyn MemoryStore> {
    Arc::new(TauriMemoryStore::new(app.clone(), store_id.to_string()))
}

// --- Audio / ingestion I/O --------------------------------------------------

pub fn audio_codec() -> Arc<dyn AudioCodec> {
    Arc::new(SymphoniaCodec)
}

pub fn text_extractor() -> Arc<dyn TextExtractor> {
    Arc::new(IngestTextExtractor)
}

pub fn file_fetcher() -> Arc<dyn FileFetcher> {
    Arc::new(HttpFileFetcher)
}
