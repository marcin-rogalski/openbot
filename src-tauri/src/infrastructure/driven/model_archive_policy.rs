//! Driven adapter: `ArchivePolicy` over the bot's model (`model::should_archive`
//! / `model::pick_folder`).

use async_trait::async_trait;

use crate::application::ports::ingestion::ArchivePolicy;
use crate::infrastructure::config::BotConfig;
use crate::infrastructure::driven::model;

pub struct ModelArchivePolicy {
    bot: BotConfig,
}

impl ModelArchivePolicy {
    pub fn new(bot: BotConfig) -> Self {
        Self { bot }
    }
}

#[async_trait]
impl ArchivePolicy for ModelArchivePolicy {
    async fn should_archive(
        &self,
        guidance: &str,
        context: &str,
        filename: &str,
        mime: &str,
    ) -> bool {
        model::should_archive(&self.bot, guidance, context, filename, mime).await
    }

    async fn pick_folder(
        &self,
        guidance: &str,
        context: &str,
        filename: &str,
        candidates: &[String],
    ) -> Option<String> {
        model::pick_folder(&self.bot, guidance, context, filename, candidates).await
    }
}
