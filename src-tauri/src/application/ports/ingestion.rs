//! Ports for archival ingestion: the AI archival policy (should we keep this
//! file, and where), text extraction from arbitrary files, and fetching an
//! attachment's bytes. Driven adapters implement them.

use async_trait::async_trait;

/// AI decisions about archiving: whether a file is worth keeping, and which
/// subfolder it belongs in. Backed by the bot's model.
#[async_trait]
pub trait ArchivePolicy: Send + Sync {
    async fn should_archive(
        &self,
        guidance: &str,
        context: &str,
        filename: &str,
        mime: &str,
    ) -> bool;
    /// Pick a folder name from `candidates` (empty return = none / use the root).
    async fn pick_folder(
        &self,
        guidance: &str,
        context: &str,
        filename: &str,
        candidates: &[String],
    ) -> Option<String>;
}

/// Extract indexable text from a file's bytes. `None` = unsupported/unreadable.
#[async_trait]
pub trait TextExtractor: Send + Sync {
    async fn extract(&self, bytes: &[u8], filename: &str, mime: &str) -> Option<String>;
}

/// Fetch an attachment's bytes from its URL.
#[async_trait]
pub trait FileFetcher: Send + Sync {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, String>;
}
