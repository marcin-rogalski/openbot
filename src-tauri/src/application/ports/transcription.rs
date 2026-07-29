//! Ports for transcription: the speech-to-text engine, the audio codec that
//! normalises clips for it, and the summarizer. Driven adapters implement them.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::domain::transcript::Segment;

/// Speech-to-text for one audio clip (the bot's transcription model).
#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(
        &self,
        audio: Vec<u8>,
        filename: &str,
        mime: &str,
    ) -> Result<Vec<Segment>, String>;
}

/// Normalise arbitrary audio to a WAV the transcription server accepts. `None`
/// when the codec isn't supported (the caller then sends the raw bytes).
#[async_trait]
pub trait AudioCodec: Send + Sync {
    async fn decode_to_wav(&self, bytes: &[u8], filename: &str, mime: &str) -> Option<Vec<u8>>;

    /// Stream-decode a file into fixed-length mono WAV chunk files in `out_dir`
    /// (bounded memory for long recordings). Returns the chunk paths and whether
    /// it was truncated at `max_chunks`.
    async fn split_to_wav_chunks(
        &self,
        source: PathBuf,
        filename: String,
        mime: String,
        chunk_secs: u32,
        max_chunks: usize,
        out_dir: PathBuf,
    ) -> Result<(Vec<PathBuf>, bool), String>;
}

/// Summarise a transcript into concise Markdown.
#[async_trait]
pub trait Summarizer: Send + Sync {
    async fn summarize(&self, transcript: &str) -> Result<String, String>;
}
