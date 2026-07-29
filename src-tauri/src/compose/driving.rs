//! Driving-facing composition — wire driven adapters into the usecases that the
//! driving side (tools, discord, voice) invokes. Third phase: depends on
//! `driven`. (Driving adapters that need a bare driven port — e.g. list/delete —
//! call `compose::driven::*` directly.)

use tauri::AppHandle;

use crate::application::usecases::archive_attachment::ArchiveAttachment;
use crate::application::usecases::ask_knowledge::AskKnowledge;
use crate::application::usecases::fetch_page::FetchPage;
use crate::application::usecases::index_document::IndexDocument;
use crate::application::usecases::reindex_knowledge::ReindexKnowledge;
use crate::application::usecases::save_memory::SaveMemory;
use crate::application::usecases::search_web::SearchWeb;
use crate::application::usecases::transcribe_clip::TranscribeClip;
use crate::compose::driven;
use crate::infrastructure::config::BotConfig;

pub fn search_web(api_key: &str) -> SearchWeb {
    SearchWeb::new(driven::web_search(api_key))
}

pub fn fetch_page(api_key: &str) -> FetchPage {
    FetchPage::new(driven::web_fetch(api_key))
}

pub fn ask_knowledge(app: &AppHandle, bot: &BotConfig, instance_id: &str) -> AskKnowledge {
    AskKnowledge::new(
        driven::embeddings(bot),
        driven::knowledge_index(app, instance_id),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn reindex_knowledge(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
) -> ReindexKnowledge {
    ReindexKnowledge::new(
        driven::drive_storage(app, client_id, client_secret, folder_id),
        driven::embeddings(bot),
        driven::knowledge_index(app, instance_id),
    )
}

pub fn index_document(app: &AppHandle, bot: &BotConfig, instance_id: &str) -> IndexDocument {
    IndexDocument::new(
        driven::embeddings(bot),
        driven::knowledge_index(app, instance_id),
    )
}

pub fn save_memory(app: &AppHandle, bot: &BotConfig) -> SaveMemory {
    SaveMemory::new(
        driven::memory_store(app, &bot.id),
        driven::memory_consolidator(bot),
        bot.memory_max_notes,
        bot.memory_char_budget,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn archive_attachment(
    app: &AppHandle,
    bot: &BotConfig,
    instance_id: &str,
    client_id: &str,
    client_secret: &str,
    folder_id: &str,
) -> ArchiveAttachment {
    ArchiveAttachment::new(
        driven::archive_policy(bot),
        driven::file_fetcher(),
        driven::text_extractor(),
        driven::drive_storage(app, client_id, client_secret, folder_id),
        index_document(app, bot, instance_id),
    )
}

pub fn transcribe_clip(bot: &BotConfig) -> TranscribeClip {
    TranscribeClip::new(driven::audio_codec(), driven::transcriber(bot))
}
