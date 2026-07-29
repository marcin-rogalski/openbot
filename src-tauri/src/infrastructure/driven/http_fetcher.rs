//! Driven adapter: `FileFetcher` over the shared HTTP client.

use async_trait::async_trait;

use crate::application::ports::ingestion::FileFetcher;
use crate::infrastructure::shared::http;

pub struct HttpFileFetcher;

#[async_trait]
impl FileFetcher for HttpFileFetcher {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = http::client()
            .get(url)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("read failed: {e}"))
    }
}
