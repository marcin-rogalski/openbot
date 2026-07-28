//! Port: the web-page-fetch capability. A driven adapter (e.g. Keenable) implements it.

use async_trait::async_trait;

use crate::domain::page::{PageContent, PageUrl};

#[async_trait]
pub trait WebFetch: Send + Sync {
    async fn fetch(&self, url: &PageUrl) -> Result<PageContent, String>;
}
