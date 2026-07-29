//! Port: the conversational LLM. The driving side (Discord) builds the message
//! list and streams the reply; the adapter owns the provider wire protocol, so
//! swapping model vendors touches only infrastructure.

use async_trait::async_trait;

use crate::domain::conversation::ChatMessage;

/// A completed reply plus best-effort throughput (tokens/s), for the status bar.
pub struct ChatReply {
    pub text: String,
    pub prefill_tps: Option<f64>,
    pub inference_tps: Option<f64>,
}

#[async_trait]
pub trait ChatModel: Send + Sync {
    /// Stream a chat completion. `on_delta` receives the full text so far as
    /// tokens arrive (for live display).
    async fn chat(
        &self,
        messages: Vec<ChatMessage>,
        // Explicit HRTB so async_trait keeps the callback's lifetime higher-ranked.
        on_delta: &(dyn for<'a> Fn(&'a str) + Sync),
    ) -> Result<ChatReply, String>;

    /// Decide whether the bot should reply to a new message in an open thread.
    async fn should_engage(&self, context: &str, message: &str) -> bool;

    /// Fold newer messages into a running conversation summary.
    async fn summarize_conversation(
        &self,
        previous: &str,
        new_messages: &str,
    ) -> Result<String, String>;
}
