//! Usecase: transcribe a single audio clip. Normalises to WAV when the codec is
//! supported (falling back to the raw bytes otherwise), then transcribes into a
//! `Transcript`.

use std::sync::Arc;

use crate::application::ports::transcription::{AudioCodec, Transcriber};
use crate::domain::transcript::Transcript;

pub struct TranscribeClip {
    codec: Arc<dyn AudioCodec>,
    transcriber: Arc<dyn Transcriber>,
}

impl TranscribeClip {
    pub fn new(codec: Arc<dyn AudioCodec>, transcriber: Arc<dyn Transcriber>) -> Self {
        Self { codec, transcriber }
    }

    pub async fn run(
        &self,
        bytes: Vec<u8>,
        filename: &str,
        mime: &str,
    ) -> Result<Transcript, String> {
        let (audio, name, mime) = match self.codec.decode_to_wav(&bytes, filename, mime).await {
            Some(wav) => (wav, "audio.wav".to_string(), "audio/wav".to_string()),
            None => (bytes, filename.to_string(), mime.to_string()),
        };
        let segments = self.transcriber.transcribe(audio, &name, &mime).await?;
        let mut transcript = Transcript::default();
        transcript.append(segments, 0.0);
        Ok(transcript)
    }
}
