//! The web-search tool: `search` + `fetch` ops, executed via the web driving
//! adapter (Keenable behind the `WebSearch`/`WebFetch` ports).

use serde_json::Value;

#[derive(Clone, Copy)]
pub enum WebOp {
    Search,
    Fetch,
}

impl WebOp {
    pub const ALL: [WebOp; 2] = [WebOp::Search, WebOp::Fetch];

    pub fn suffix(self) -> &'static str {
        match self {
            WebOp::Search => "search",
            WebOp::Fetch => "fetch",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            WebOp::Search => "Search the web; returns a list of results (title, url, excerpt).",
            WebOp::Fetch => "Fetch a web page by url and return its main text content.",
        }
    }

    pub fn args(self) -> &'static str {
        match self {
            WebOp::Search => "{\"query\": string}",
            WebOp::Fetch => "{\"url\": string}",
        }
    }
}

/// Execute a web op with the instance's API key.
pub(super) async fn execute(op: WebOp, api_key: &str, args: &Value) -> String {
    let arg = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
    match op {
        WebOp::Search => {
            match crate::infrastructure::driving::web::search(api_key, arg("query")).await {
                Ok(results) => results,
                Err(e) => format!("error: {e}"),
            }
        }
        WebOp::Fetch => match crate::infrastructure::driving::web::fetch(api_key, arg("url")).await
        {
            Ok(content) => content,
            Err(e) => format!("error: {e}"),
        },
    }
}
