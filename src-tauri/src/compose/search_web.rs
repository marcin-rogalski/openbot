//! Compose the web-search slice: the Keenable driven adapter wired into the
//! `SearchWeb` usecase. Per-instance (credentialed), so built on demand.

use std::sync::Arc;

use crate::application::usecases::search_web::SearchWeb;
use crate::infrastructure::driven::keenable::KeenableSearch;
use crate::infrastructure::shared::http;

pub fn compose_search_web(api_key: &str) -> SearchWeb {
    let adapter = KeenableSearch::new(http::client(), api_key.to_string());
    SearchWeb::new(Arc::new(adapter))
}
