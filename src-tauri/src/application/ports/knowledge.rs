//! Ports for the knowledge base: the local index it retrieves from / writes to,
//! and the embeddings it turns text into. Driven adapters implement them.

use async_trait::async_trait;

use crate::domain::knowledge::{KnowledgePassage, SourceRef, SourceSummary};

/// The per-instance knowledge index (retrieval + maintenance). Scoped to one
/// tool instance; the SQLite/vector details live in the adapter.
#[async_trait]
pub trait KnowledgeIndex: Send + Sync {
    /// Hybrid retrieval for a pre-embedded question; top-`k` cited passages.
    async fn search(
        &self,
        query_embedding: Vec<f32>,
        query_text: &str,
        k: usize,
    ) -> Result<Vec<KnowledgePassage>, String>;
    /// Whether a source with this Drive id is already indexed.
    async fn has_source(&self, drive_id: &str) -> Result<bool, String>;
    /// Replace a source's chunks (paired with their embeddings).
    async fn upsert(
        &self,
        source: SourceRef,
        chunks: Vec<(String, Vec<f32>)>,
    ) -> Result<(), String>;
    /// List indexed sources with their chunk counts.
    async fn list_sources(&self) -> Result<Vec<SourceSummary>, String>;
}

/// Turn text into embedding vectors (the bot's embedding model).
#[async_trait]
pub trait Embeddings: Send + Sync {
    async fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, String>;
    /// The model name, recorded as a source's embedding provenance.
    fn model(&self) -> String;
}
