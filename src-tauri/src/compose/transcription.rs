//! Compose the transcription slices. Per bot (its transcription + summary models).

use std::sync::Arc;

use crate::application::ports::transcription::{AudioCodec, Summarizer, Transcriber};
use crate::application::usecases::transcribe_clip::TranscribeClip;
use crate::infrastructure::config::BotConfig;
use crate::infrastructure::driven::model_summarizer::ModelSummarizer;
use crate::infrastructure::driven::model_transcriber::ModelTranscriber;
use crate::infrastructure::driven::symphonia_codec::SymphoniaCodec;

pub fn compose_transcribe_clip(bot: &BotConfig) -> TranscribeClip {
    TranscribeClip::new(compose_audio_codec(), compose_transcriber(bot))
}

pub fn compose_audio_codec() -> Arc<dyn AudioCodec> {
    Arc::new(SymphoniaCodec)
}

pub fn compose_transcriber(bot: &BotConfig) -> Arc<dyn Transcriber> {
    Arc::new(ModelTranscriber::new(bot.clone()))
}

pub fn compose_summarizer(bot: &BotConfig) -> Arc<dyn Summarizer> {
    Arc::new(ModelSummarizer::new(bot.clone()))
}
