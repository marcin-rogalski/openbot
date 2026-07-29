//! Driven adapter: `AudioCodec` over the symphonia/audiopus decode path in the
//! `audio` module. Decoding is CPU-bound, so it runs on a blocking thread.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::application::ports::transcription::AudioCodec;
use crate::audio;

pub struct SymphoniaCodec;

#[async_trait]
impl AudioCodec for SymphoniaCodec {
    async fn decode_to_wav(&self, bytes: &[u8], filename: &str, mime: &str) -> Option<Vec<u8>> {
        let bytes = bytes.to_vec();
        let filename = filename.to_string();
        let mime = mime.to_string();
        tokio::task::spawn_blocking(move || audio::decode_to_wav(&bytes, &filename, &mime))
            .await
            .ok()
            .flatten()
    }

    async fn split_to_wav_chunks(
        &self,
        source: PathBuf,
        filename: String,
        mime: String,
        chunk_secs: u32,
        max_chunks: usize,
        out_dir: PathBuf,
    ) -> Result<(Vec<PathBuf>, bool), String> {
        tokio::task::spawn_blocking(move || {
            audio::split_to_wav_chunks(&source, &filename, &mime, chunk_secs, max_chunks, &out_dir)
        })
        .await
        .map_err(|e| format!("decode task failed: {e}"))?
    }
}
