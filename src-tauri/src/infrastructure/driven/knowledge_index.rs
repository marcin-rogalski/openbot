//! Driven adapter: `KnowledgeIndex` over the per-instance SQLite store. Delegates
//! to the `knowledge` module (SQLite + FTS5 + cosine) and maps its rows to the
//! domain types.

use async_trait::async_trait;
use tauri::AppHandle;

use crate::application::ports::knowledge::KnowledgeIndex;
use crate::domain::knowledge::{KnowledgePassage, SourceRef, SourceSummary};
use crate::infrastructure::driven::knowledge::{self, SourceMeta};

pub struct SqliteKnowledgeIndex {
    app: AppHandle,
    instance_id: String,
}

impl SqliteKnowledgeIndex {
    pub fn new(app: AppHandle, instance_id: String) -> Self {
        Self { app, instance_id }
    }
}

#[async_trait]
impl KnowledgeIndex for SqliteKnowledgeIndex {
    async fn search(
        &self,
        query_embedding: Vec<f32>,
        query_text: &str,
        k: usize,
    ) -> Result<Vec<KnowledgePassage>, String> {
        let hits = knowledge::search(
            &self.app,
            &self.instance_id,
            query_embedding,
            query_text.to_string(),
            k,
        )
        .await?;
        Ok(hits
            .into_iter()
            .map(|h| KnowledgePassage {
                name: h.name,
                drive_id: h.drive_id,
                text: h.text,
            })
            .collect())
    }

    async fn has_source(&self, drive_id: &str) -> Result<bool, String> {
        knowledge::has_source(&self.app, &self.instance_id, drive_id).await
    }

    async fn upsert(
        &self,
        source: SourceRef,
        chunks: Vec<(String, Vec<f32>)>,
    ) -> Result<(), String> {
        let meta = SourceMeta {
            drive_id: source.drive_id,
            name: source.name,
            mime: source.mime,
            embed_model: source.embed_model,
        };
        knowledge::upsert_source(&self.app, &self.instance_id, meta, chunks).await
    }

    async fn list_sources(&self) -> Result<Vec<SourceSummary>, String> {
        let rows = knowledge::list_sources(&self.app, &self.instance_id).await?;
        Ok(rows
            .into_iter()
            .map(|(name, drive_id, chunks)| SourceSummary {
                name,
                drive_id,
                chunks,
            })
            .collect())
    }
}
