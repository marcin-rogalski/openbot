//! Web-search domain: a validated query and a result as the *business*
//! representation of a hit (independent of any provider's wire format).

/// A non-empty, trimmed search query. The invariant (non-empty) lives here.
#[derive(Debug, Clone)]
pub struct SearchQuery(String);

impl SearchQuery {
    pub fn new(raw: &str) -> Result<Self, String> {
        let q = raw.trim();
        if q.is_empty() {
            return Err("empty search query".to_string());
        }
        Ok(Self(q.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One search result in business terms — not a provider's JSON shape.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_rejects_blank_and_trims() {
        assert!(SearchQuery::new("   ").is_err());
        assert_eq!(SearchQuery::new("  rust  ").unwrap().as_str(), "rust");
    }
}
