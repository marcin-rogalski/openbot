//! Driven adapter: Keenable implements the `WebSearch` and `WebFetch` ports.
//! Owns the wire format and maps the response DTOs to domain types.

use async_trait::async_trait;

use crate::application::ports::webfetch::WebFetch;
use crate::application::ports::websearch::WebSearch;
use crate::domain::page::{PageContent, PageUrl};
use crate::domain::search::{SearchHit, SearchQuery};
use crate::infrastructure::dto::keenable::{FetchResponse, SearchResponse};

const SEARCH_URL: &str = "https://api.keenable.ai/v1/search";
const FETCH_URL: &str = "https://api.keenable.ai/v1/fetch";
/// Keep result lists small so they don't blow up the model context.
const MAX_RESULTS: usize = 5;
/// Cap fetched page text so a huge page can't blow up the model context.
const MAX_FETCH_CHARS: usize = 6000;

pub struct KeenableSearch {
    http: reqwest::Client,
    api_key: String,
}

impl KeenableSearch {
    pub fn new(http: reqwest::Client, api_key: String) -> Self {
        Self { http, api_key }
    }
}

#[async_trait]
impl WebSearch for KeenableSearch {
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchHit>, String> {
        let resp = self
            .http
            .post(SEARCH_URL)
            .header("X-API-Key", &self.api_key)
            .json(&serde_json::json!({ "query": query.as_str() }))
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Keenable API {status}: {}", body.trim()));
        }
        let parsed: SearchResponse = resp
            .json()
            .await
            .map_err(|e| format!("bad response: {e}"))?;
        Ok(parsed
            .results
            .into_iter()
            .take(MAX_RESULTS)
            .map(|r| SearchHit {
                title: r.title.trim().to_string(),
                url: r.url.trim().to_string(),
                snippet: r
                    .snippet
                    .as_deref()
                    .unwrap_or(&r.description)
                    .trim()
                    .to_string(),
            })
            .collect())
    }
}

pub struct KeenableFetch {
    http: reqwest::Client,
    api_key: String,
}

impl KeenableFetch {
    pub fn new(http: reqwest::Client, api_key: String) -> Self {
        Self { http, api_key }
    }
}

#[async_trait]
impl WebFetch for KeenableFetch {
    async fn fetch(&self, url: &PageUrl) -> Result<PageContent, String> {
        let resp = self
            .http
            .get(FETCH_URL)
            .header("X-API-Key", &self.api_key)
            .query(&[("url", url.as_str())])
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Keenable API {status}: {}", body.trim()));
        }
        let parsed: FetchResponse = resp
            .json()
            .await
            .map_err(|e| format!("bad response: {e}"))?;
        let title = parsed
            .title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        Ok(PageContent {
            title,
            markdown: truncate(parsed.content.trim(), MAX_FETCH_CHARS),
        })
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push_str("…[truncated]");
    out
}
