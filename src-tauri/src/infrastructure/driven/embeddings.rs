//! Driven adapter: `Embeddings` over the bot's model server (`model::embed`).

use async_trait::async_trait;

use crate::application::ports::knowledge::Embeddings;
use crate::config::BotConfig;
use crate::model;

pub struct ModelEmbeddings {
    bot: BotConfig,
}

impl ModelEmbeddings {
    pub fn new(bot: BotConfig) -> Self {
        Self { bot }
    }
}

#[async_trait]
impl Embeddings for ModelEmbeddings {
    async fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, String> {
        model::embed(&self.bot, inputs).await
    }

    fn model(&self) -> String {
        self.bot.model.embedding_model.clone()
    }
}
