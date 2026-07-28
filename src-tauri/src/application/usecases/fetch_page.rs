//! Usecase: fetch a web page as readable content. Validates the URL (domain),
//! delegates to the `WebFetch` port. Provider-agnostic — testable against a fake.

use std::sync::Arc;

use crate::application::ports::webfetch::WebFetch;
use crate::domain::page::{PageContent, PageUrl};

pub struct FetchPage {
    web: Arc<dyn WebFetch>,
}

impl FetchPage {
    pub fn new(web: Arc<dyn WebFetch>) -> Self {
        Self { web }
    }

    pub async fn run(&self, raw_url: &str) -> Result<PageContent, String> {
        let url = PageUrl::new(raw_url)?;
        self.web.fetch(&url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct FakeWeb(PageContent);

    #[async_trait]
    impl WebFetch for FakeWeb {
        async fn fetch(&self, _url: &PageUrl) -> Result<PageContent, String> {
            Ok(self.0.clone())
        }
    }

    #[tokio::test]
    async fn rejects_schemeless_url_before_hitting_the_port() {
        let uc = FetchPage::new(Arc::new(FakeWeb(PageContent {
            title: None,
            markdown: String::new(),
        })));
        assert!(uc.run("example.com").await.is_err());
    }

    #[tokio::test]
    async fn returns_content_for_a_valid_url() {
        let uc = FetchPage::new(Arc::new(FakeWeb(PageContent {
            title: Some("Title".into()),
            markdown: "body".into(),
        })));
        let out = uc.run("https://example.com").await.unwrap();
        assert_eq!(out.title.as_deref(), Some("Title"));
        assert_eq!(out.markdown, "body");
    }
}
