//! Driven adapter: `AudioCodec` over the symphonia/audiopus decode path in the
//! `audio` module. Decoding is CPU-bound, so it runs on a blocking thread.

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
}
