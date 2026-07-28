//! Web-page domain: the address the business asks to read, and the readable
//! content it gets back. Representation + validation only — no HTTP, no parsing.

/// A validated web-page address to fetch. Trims surrounding whitespace and
/// requires an `http`/`https` scheme so obvious junk is rejected before any IO.
#[derive(Debug, Clone)]
pub struct PageUrl(String);

impl PageUrl {
    pub fn new(raw: impl Into<String>) -> Result<Self, String> {
        let url = raw.into().trim().to_string();
        if url.is_empty() {
            return Err("url is empty".to_string());
        }
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("url must start with http:// or https://".to_string());
        }
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The readable content of a fetched page: an optional title plus the body as
/// markdown.
#[derive(Debug, Clone)]
pub struct PageContent {
    pub title: Option<String>,
    pub markdown: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_and_schemeless() {
        assert!(PageUrl::new("   ").is_err());
        assert!(PageUrl::new("example.com").is_err());
        assert!(PageUrl::new("ftp://example.com").is_err());
    }

    #[test]
    fn accepts_and_trims_http_urls() {
        let u = PageUrl::new("  https://example.com/a  ").unwrap();
        assert_eq!(u.as_str(), "https://example.com/a");
    }
}
