//! Driven adapter: `ChatModel` over the OpenAI-compatible `model` client.

use async_trait::async_trait;

use crate::application::ports::chat_model::{ChatModel, ChatReply};
use crate::config::BotConfig;
use crate::domain::conversation::ChatMessage;
use crate::model;

pub struct ModelChat {
    bot: BotConfig,
}

impl ModelChat {
    pub fn new(bot: BotConfig) -> Self {
        Self { bot }
    }
}

#[async_trait]
impl ChatModel for ModelChat {
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        on_delta: &(dyn for<'a> Fn(&'a str) + Sync),
    ) -> Result<ChatReply, String> {
        let (text, metrics) = model::chat(&self.bot, messages, on_delta).await?;
        Ok(ChatReply {
            text,
            prefill_tps: metrics.prefill_tps,
            inference_tps: metrics.inference_tps,
        })
    }

    async fn should_engage(&self, context: &str, message: &str) -> bool {
        model::should_engage(&self.bot, context, message).await
    }

    async fn summarize_conversation(
        &self,
        previous: &str,
        new_messages: &str,
    ) -> Result<String, String> {
        model::summarize_conversation(&self.bot, previous, new_messages).await
    }
}
