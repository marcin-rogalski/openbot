//! Usecase: index one document's text into the knowledge base — chunk (service)
//! → embed (port) → upsert (port). Shared by every ingestion path. `Ok(false)`
//! means there was nothing to index (empty text).

use std::sync::Arc;

use crate::application::ports::knowledge::{Embeddings, KnowledgeIndex};
use crate::application::services::chunking;
use crate::domain::knowledge::SourceRef;

pub struct IndexDocument {
    embeddings: Arc<dyn Embeddings>,
    index: Arc<dyn KnowledgeIndex>,
}

impl IndexDocument {
    pub fn new(embeddings: Arc<dyn Embeddings>, index: Arc<dyn KnowledgeIndex>) -> Self {
        Self { embeddings, index }
    }

    pub async fn run(
        &self,
        drive_id: &str,
        name: &str,
        mime: &str,
        text: &str,
    ) -> Result<bool, String> {
        let chunks = chunking::chunk(text);
        if chunks.is_empty() {
            return Ok(false);
        }
        let embeddings = self.embeddings.embed(&chunks).await?;
        let paired: Vec<(String, Vec<f32>)> = chunks.into_iter().zip(embeddings).collect();
        self.index
            .upsert(
                SourceRef {
                    drive_id: drive_id.to_string(),
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
