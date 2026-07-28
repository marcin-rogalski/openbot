//! Usecase: search the web. Validates the query (domain), delegates to the
//! `WebSearch` port. Provider-agnostic — testable against a fake port.

use std::sync::Arc;

use crate::application::ports::websearch::WebSearch;
use crate::domain::search::{SearchHit, SearchQuery};

pub struct SearchWeb {
    web: Arc<dyn WebSearch>,
}

impl SearchWeb {
    pub fn new(web: Arc<dyn WebSearch>) -> Self {
        Self { web }
    }

    pub async fn run(&self, raw_query: &str) -> Result<Vec<SearchHit>, String> {
        let query = SearchQuery::new(raw_query)?;
        self.web.search(&query).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct FakeWeb(Vec<SearchHit>);

    #[async_trait]
    impl WebSearch for FakeWeb {
        async fn search(&self, _q: &SearchQuery) -> Result<Vec<SearchHit>, String> {
            Ok(self.0.clone())
        }
    }

    #[tokio::test]
    async fn rejects_empty_query_before_hitting_the_port() {
        let uc = SearchWeb::new(Arc::new(FakeWeb(vec![])));
        assert!(uc.run("   ").await.is_err());
    }

    #[tokio::test]
    async fn returns_hits_for_a_valid_query() {
        let hit = SearchHit {
            title: "t".into(),
            url: "u".into(),
            snippet: "s".into(),
        };
        let uc = SearchWeb::new(Arc::new(FakeWeb(vec![hit])));
        let out = uc.run("rust").await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url, "u");
    }
}
