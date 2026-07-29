//! Usecase: the attachment gate. Decide (policy) whether a posted file is worth
//! keeping; if so fetch it, extract its text, pick a subfolder (semantic
//! foldering), upload it to Drive, and index it. Best-effort indexing — a failed
//! index doesn't undo a successful archive.

use std::sync::Arc;

use crate::application::ports::drive::DriveStorage;
use crate::application::ports::ingestion::{ArchivePolicy, FileFetcher, TextExtractor};
use crate::application::services::foldering;
use crate::application::usecases::index_document::IndexDocument;

/// What the gate did.
pub enum ArchiveOutcome {
    /// The policy judged the file not worth keeping.
    Skipped,
    /// Archived to Drive (indexed too when text could be extracted).
    Archived { drive_id: String, indexed: bool },
}

pub struct ArchiveAttachment {
    policy: Arc<dyn ArchivePolicy>,
    fetcher: Arc<dyn FileFetcher>,
    extractor: Arc<dyn TextExtractor>,
    drive: Arc<dyn DriveStorage>,
    indexer: IndexDocument,
}

impl ArchiveAttachment {
    pub fn new(
        policy: Arc<dyn ArchivePolicy>,
        fetcher: Arc<dyn FileFetcher>,
        extractor: Arc<dyn TextExtractor>,
        drive: Arc<dyn DriveStorage>,
        indexer: IndexDocument,
    ) -> Self {
        Self {
            policy,
            fetcher,
            extractor,
            drive,
            indexer,
        }
    }

    pub async fn run(
        &self,
        guidance: &str,
        context: &str,
        filename: &str,
        mime: &str,
        url: &str,
    ) -> Result<ArchiveOutcome, String> {
        if !self
            .policy
            .should_archive(guidance, context, filename, mime)
            .await
        {
            return Ok(ArchiveOutcome::Skipped);
        }

        let bytes = self.fetcher.fetch(url).await?;
        // Extract before the bytes are moved into the upload.
        let extracted = self.extractor.extract(&bytes, filename, mime).await;
        let target =
            foldering::choose_folder(&*self.drive, &*self.policy, guidance, context, filename)
                .await;
        let drive_id = self
            .drive
            .upload_binary(&target, filename, bytes, mime)
            .await?;

        let indexed = match extracted {
            Some(text) => self
                .indexer
                .run(&drive_id, filename, mime, &text)
                .await
                .unwrap_or(false),
            None => false,
        };
        Ok(ArchiveOutcome::Archived { drive_id, indexed })
    }
}
