//! Compose the conversational model. Per bot (its model settings).

use std::sync::Arc;

use crate::application::ports::chat_model::ChatModel;
use crate::config::BotConfig;
use crate::infrastructure::driven::model_chat::ModelChat;

pub fn compose_chat_model(bot: &BotConfig) -> Arc<dyn ChatModel> {
    Arc::new(ModelChat::new(bot.clone()))
}
