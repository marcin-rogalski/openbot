//! Compose the web-fetch slice: the Keenable driven adapter wired into the
//! `FetchPage` usecase. Per-instance (credentialed), so built on demand.

use std::sync::Arc;

use crate::application::usecases::fetch_page::FetchPage;
use crate::infrastructure::driven::keenable::KeenableFetch;
use crate::infrastructure::shared::http;

pub fn compose_fetch_page(api_key: &str) -> FetchPage {
    let adapter = KeenableFetch::new(http::client(), api_key.to_string());
    FetchPage::new(Arc::new(adapter))
}
