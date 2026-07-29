//! Usecase: rebuild the local knowledge index from the Drive folder. Walks the
//! folder, and for each not-yet-indexed file reads → chunks → embeds → upserts.
//! The index is a derived cache, so this can always reconstruct it.

use std::sync::Arc;

use crate::application::ports::drive::DriveStorage;
use crate::application::ports::knowledge::{Embeddings, KnowledgeIndex};
use crate::application::services::chunking;
use crate::domain::knowledge::SourceRef;

/// Progress for one file as reindex walks the folder.
pub struct ReindexProgress<'a> {
    pub seen: usize,
    pub total: usize,
    pub name: &'a str,
}

/// Outcome tallies.
pub struct ReindexCounts {
    pub indexed: usize,
    pub skipped: usize,
    pub failed: usize,
}

pub struct ReindexKnowledge {
    drive: Arc<dyn DriveStorage>,
    embeddings: Arc<dyn Embeddings>,
    index: Arc<dyn KnowledgeIndex>,
}

impl ReindexKnowledge {
    pub fn new(
        drive: Arc<dyn DriveStorage>,
        embeddings: Arc<dyn Embeddings>,
        index: Arc<dyn KnowledgeIndex>,
    ) -> Self {
        Self {
            drive,
            embeddings,
            index,
        }
    }

    pub async fn run(
        &self,
        on_progress: &(dyn Fn(ReindexProgress) + Sync),
    ) -> Result<ReindexCounts, String> {
        let entries = self.drive.search("").await?;
        let files: Vec<_> = entries.into_iter().filter(|e| !e.is_folder()).collect();
        let total = files.len();

        let mut counts = ReindexCounts {
            indexed: 0,
            skipped: 0,
            failed: 0,
        };
        for (i, file) in files.iter().enumerate() {
            on_progress(ReindexProgress {
                seen: i + 1,
                total,
                name: &file.name,
            });
            if self.index.has_source(&file.id).await.unwrap_or(false) {
                counts.skipped += 1;
                continue;
            }
            match self.index_one(&file.id, &file.name, &file.mime_type).await {
                Ok(true) => counts.indexed += 1,
                Ok(false) => counts.skipped += 1,
                Err(_) => counts.failed += 1,
            }
        }
        Ok(counts)
    }

    /// `Ok(false)` = unreadable/empty (skipped, not an error).
    async fn index_one(&self, id: &str, name: &str, mime: &str) -> Result<bool, String> {
        let Ok(text) = self.drive.read(id).await else {
            return Ok(false);
        };
        let chunks = chunking::chunk(&text);
        if chunks.is_empty() {
            return Ok(false);
        }
        let embeddings = self.embeddings.embed(&chunks).await?;
        let paired: Vec<(String, Vec<f32>)> = chunks.into_iter().zip(embeddings).collect();
        self.index
            .upsert(
                SourceRef {
                    drive_id: id.to_string(),
                    name: name.to_string(),
                    mime: mime.to_string(),
                    embed_model: self.embeddings.model(),
                },
                paired,
            )
            .await?;
        Ok(true)
    }
}
