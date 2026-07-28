//! Port: the web-search capability. A driven adapter (e.g. Keenable) implements it.

use async_trait::async_trait;

use crate::domain::search::{SearchHit, SearchQuery};

#[async_trait]
pub trait WebSearch: Send + Sync {
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>, String>;
}
