//! Driven adapter: `Summarizer` over the bot's model (`model::summarize_transcript`).

use async_trait::async_trait;

use crate::application::ports::transcription::Summarizer;
use crate::config::BotConfig;
use crate::model;

pub struct ModelSummarizer {
    bot: BotConfig,
}

impl ModelSummarizer {
    pub fn new(bot: BotConfig) -> Self {
        Self { bot }
    }
}

#[async_trait]
impl Summarizer for ModelSummarizer {
    async fn summarize(&self, transcript: &str) -> Result<String, String> {
        model::summarize_transcript(&self.bot, transcript).await
    }
}
