//! Driven adapter: `Transcriber` over the bot's transcription model
//! (`model::transcribe_segments`), mapping its segments to the domain type.

use async_trait::async_trait;

use crate::application::ports::transcription::Transcriber;
use crate::domain::transcript::Segment;
use crate::infrastructure::config::BotConfig;
use crate::infrastructure::driven::model;

pub struct ModelTranscriber {
    bot: BotConfig,
}

impl ModelTranscriber {
    pub fn new(bot: BotConfig) -> Self {
        Self { bot }
    }
}

#[async_trait]
impl Transcriber for ModelTranscriber {
    async fn transcribe(
        &self,
        audio: Vec<u8>,
        filename: &str,
        mime: &str,
    ) -> Result<Vec<Segment>, String> {
        let segments = model::transcribe_segments(&self.bot, audio, filename, mime).await?;
        Ok(segments
            .into_iter()
            .map(|s| Segment {
                start: s.start,
                text: s.text,
            })
            .collect())
    }
}
