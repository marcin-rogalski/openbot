//! Usecase: answer a question from the knowledge base. Embeds the question, then
//! retrieves the top-`k` cited passages for the model to synthesise from.

use std::sync::Arc;

use crate::application::ports::knowledge::{Embeddings, KnowledgeIndex};
use crate::domain::knowledge::KnowledgePassage;

pub struct AskKnowledge {
    embeddings: Arc<dyn Embeddings>,
    index: Arc<dyn KnowledgeIndex>,
}

impl AskKnowledge {
    pub fn new(embeddings: Arc<dyn Embeddings>, index: Arc<dyn KnowledgeIndex>) -> Self {
        Self { embeddings, index }
    }

    pub async fn run(&self, question: &str, k: usize) -> Result<Vec<KnowledgePassage>, String> {
        let mut embs = self
            .embeddings
            .embed(std::slice::from_ref(&question.to_string()))
            .await?;
        if embs.is_empty() {
            return Err("no embedding returned".to_string());
        }
        let query = embs.remove(0);
        self.index.search(query, question, k).await
    }
}
