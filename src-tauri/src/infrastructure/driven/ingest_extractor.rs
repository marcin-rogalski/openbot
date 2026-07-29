//! Driven adapter: `TextExtractor` over the `ingest` module (utf-8 text + PDF).
//! PDF parsing is CPU-bound, so it runs on a blocking thread.

use async_trait::async_trait;

use crate::application::ports::ingestion::TextExtractor;
use crate::ingest;

pub struct IngestTextExtractor;

#[async_trait]
impl TextExtractor for IngestTextExtractor {
    async fn extract(&self, bytes: &[u8], filename: &str, mime: &str) -> Option<String> {
        let bytes = bytes.to_vec();
        let filename = filename.to_string();
        let mime = mime.to_string();
        tokio::task::spawn_blocking(move || ingest::extract_text(&bytes, &filename, &mime))
            .await
            .ok()
            .flatten()
    }
}
