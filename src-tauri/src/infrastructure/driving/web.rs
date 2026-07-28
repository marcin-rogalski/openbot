//! Driving adapter: the `web_search` tool. Translates the model's tool call
//! (op + args) into `SearchWeb` / `FetchPage` usecase calls and formats the
//! domain results into the compact, model-friendly strings the tool loop feeds
//! back to the model. A tool instance carries a single Keenable API key; both
//! ops are read-only.

/// Search the web; returns a compact, model-friendly result list.
pub async fn search(api_key: &str, query: &str) -> Result<String, String> {
    let hits = crate::compose::search_web::compose_search_web(api_key)
        .run(query)
        .await?;
    if hits.is_empty() {
        return Ok("no results".to_string());
    }
    Ok(hits
        .iter()
        .map(|h| format!("- {}\n  {}\n  {}", h.title, h.url, h.snippet))
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Fetch a page and return its extracted markdown content (truncated).
pub async fn fetch(api_key: &str, url: &str) -> Result<String, String> {
    let page = crate::compose::fetch_page::compose_fetch_page(api_key)
        .run(url)
        .await?;
    match page.title {
        Some(title) => Ok(format!("# {}\n{}", title, page.markdown)),
        None => Ok(page.markdown),
    }
}
