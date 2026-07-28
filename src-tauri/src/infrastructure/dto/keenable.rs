//! Keenable search wire format — kept out of the domain.

use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub results: Vec<SearchResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub snippet: Option<String>,
}
