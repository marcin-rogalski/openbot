//! Compose the transcription slices. Per bot (its transcription + summary models).

use std::sync::Arc;

use crate::application::ports::transcription::Summarizer;
use crate::application::usecases::transcribe_clip::TranscribeClip;
use crate::config::BotConfig;
use crate::infrastructure::driven::model_summarizer::ModelSummarizer;
use crate::infrastructure::driven::model_transcriber::ModelTranscriber;
use crate::infrastructure::driven::symphonia_codec::SymphoniaCodec;

pub fn compose_transcribe_clip(bot: &BotConfig) -> TranscribeClip {
    TranscribeClip::new(
        Arc::new(SymphoniaCodec),
        Arc::new(ModelTranscriber::new(bot.clone())),
    )
}

pub fn compose_summarizer(bot: &BotConfig) -> Arc<dyn Summarizer> {
    Arc::new(ModelSummarizer::new(bot.clone()))
}
