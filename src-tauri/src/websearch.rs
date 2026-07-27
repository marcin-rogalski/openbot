//! Keenable web search + fetch client. A `web_search` tool instance carries a
//! single API key; the model gets a `search` op (find pages) and a `fetch` op
//! (read a page as markdown). Both are read-only.
//!
//! API: `POST https://api.keenable.ai/v1/search` and
//! `GET https://api.keenable.ai/v1/fetch?url=…`, authenticated with an
//! `X-API-Key` header.

use serde::Deserialize;
use serde_json::json;

const SEARCH_URL: &str = "https://api.keenable.ai/v1/search";
const FETCH_URL: &str = "https://api.keenable.ai/v1/fetch";
/// Keep tool results small so they don't blow up the model context.
const MAX_RESULTS: usize = 5;
const MAX_FETCH_CHARS: usize = 6000;

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<SearchResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    snippet: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchResponse {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    content: String,
}

/// Search the web; returns a compact, model-friendly result list.
pub async fn search(api_key: &str, query: &str) -> Result<String, String> {
    let resp = reqwest::Client::new()
        .post(SEARCH_URL)
        .header("X-API-Key", api_key)
        .json(&json!({ "query": query }))
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(error_body(resp).await);
    }

    let parsed: SearchResponse = resp
        .json()
        .await
        .map_err(|e| format!("bad response: {e}"))?;
    if parsed.results.is_empty() {
        return Ok("no results".to_string());
    }

    let lines = parsed
        .results
        .iter()
        .take(MAX_RESULTS)
        .map(|r| {
            let excerpt = r.snippet.as_deref().unwrap_or(&r.description).trim();
            format!("- {}\n  {}\n  {}", r.title.trim(), r.url.trim(), excerpt)
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(lines)
}

/// Fetch a page and return its extracted markdown content (truncated).
pub async fn fetch(api_key: &str, url: &str) -> Result<String, String> {
    let resp = reqwest::Client::new()
        .get(FETCH_URL)
        .header("X-API-Key", api_key)
        .query(&[("url", url)])
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(error_body(resp).await);
    }

    let parsed: FetchResponse = resp
        .json()
        .await
        .map_err(|e| format!("bad response: {e}"))?;
    let body = truncate(parsed.content.trim(), MAX_FETCH_CHARS);
    match parsed.title {
        Some(title) if !title.trim().is_empty() => Ok(format!("# {}\n{body}", title.trim())),
        _ => Ok(body),
    }
}

async fn error_body(resp: reqwest::Response) -> String {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    format!("Keenable API {status}: {}", body.trim())
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push_str("…[truncated]");
    out
}
