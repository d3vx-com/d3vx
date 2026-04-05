//! WebSearch Tool
//!
//! Search the web using DuckDuckGo HTML scraping and return results.

use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use std::time::Duration;
use tracing::debug;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

const SEARCH_TIMEOUT_SECS: u64 = 15;
const MAX_RESULTS: usize = 20;
const DDG_URL: &str = "https://html.duckduckgo.com/html/";

static RESULT_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"class="result__[^"]*"[^>]*>[\s\S]*?(?=class="result__|<div id="links")"#)
        .expect("Invalid result block regex")
});
static RESULT_URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"class="result__a"[^>]*href="([^"]*)""#).expect("Invalid url regex"));
static RESULT_TITLE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"class="result__a"[^>]*>([\s\S]*?)</a>"#).expect("Invalid title regex")
});
static RESULT_SNIPPET_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"class="result__snippet"[^>]*>([\s\S]*?)</[at]"#).expect("Invalid snippet regex")
});
static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").expect("Invalid tag regex"));

/// Web search tool using DuckDuckGo
#[derive(Clone, Default)]
pub struct WebSearchTool {
    client: reqwest::Client,
}

impl WebSearchTool {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(SEARCH_TIMEOUT_SECS))
            .user_agent("Mozilla/5.0 (compatible; d3vx/1.0)")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "web_search".to_string(),
            description: concat!(
                "Search the web and return results. ",
                "Returns titles, URLs, and snippets as markdown links. ",
                "Optionally filter by allowed or blocked domains."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query string" },
                    "allowed_domains": {
                        "type": "array", "items": { "type": "string" },
                        "description": "Only include results from these domains"
                    },
                    "blocked_domains": {
                        "type": "array", "items": { "type": "string" },
                        "description": "Exclude results from these domains"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let query = match input["query"].as_str() {
            Some(q) if !q.is_empty() => q,
            _ => return ToolResult::error("query is required and must be a non-empty string"),
        };
        let allowed: Vec<String> = input["allowed_domains"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let blocked: Vec<String> = input["blocked_domains"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        debug!(query = %query, "Executing web search");

        let response = match self.client.post(DDG_URL).form(&[("q", query)]).send().await {
            Ok(r) => r,
            Err(e) => {
                if e.is_timeout() {
                    return ToolResult::error(format!(
                        "Search timed out after {}s",
                        SEARCH_TIMEOUT_SECS
                    ));
                }
                return ToolResult::error(format!("Search request failed: {}", e));
            }
        };
        if !response.status().is_success() {
            return ToolResult::error(format!(
                "Search returned HTTP {}",
                response.status().as_u16()
            ));
        }
        let body = match response.text().await {
            Ok(b) => b,
            Err(e) => return ToolResult::error(format!("Failed to read search response: {}", e)),
        };

        let mut results = parse_ddg_results(&body);
        results = filter_domains(results, &allowed, &blocked);
        results.truncate(MAX_RESULTS);

        if results.is_empty() {
            return ToolResult::success("No search results found.")
                .with_metadata("resultCount", serde_json::json!(0))
                .with_metadata("query", serde_json::json!(query));
        }
        let count = results.len();
        ToolResult::success(format_results(query, &results))
            .with_metadata("resultCount", serde_json::json!(count))
            .with_metadata("query", serde_json::json!(query))
    }
}

struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

fn extract_domain(url: &str) -> Option<String> {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .and_then(|rest| rest.split('/').next())
        .and_then(|host| host.split(':').next())
        .map(|d| d.to_lowercase())
}

fn strip_html(html: &str) -> String {
    TAG_RE
        .replace_all(html, "")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

fn parse_ddg_results(html: &str) -> Vec<SearchResult> {
    let mut results = Vec::new();
    for block in RESULT_BLOCK_RE.find_iter(html) {
        let b = block.as_str();
        let url = RESULT_URL_RE
            .captures(b)
            .and_then(|c| c.get(1))
            .map(|m| extract_actual_url(m.as_str()))
            .unwrap_or_default();
        let title = RESULT_TITLE_RE
            .captures(b)
            .and_then(|c| c.get(1))
            .map(|m| strip_html(m.as_str()))
            .unwrap_or_default();
        let snippet = RESULT_SNIPPET_RE
            .captures(b)
            .and_then(|c| c.get(1))
            .map(|m| strip_html(m.as_str()))
            .unwrap_or_default();
        if !url.is_empty() && !title.is_empty() {
            results.push(SearchResult {
                title,
                url,
                snippet,
            });
        }
    }
    results
}

fn extract_actual_url(raw: &str) -> String {
    if let Some(start) = raw.find("uddg=") {
        let encoded = &raw[start + 5..];
        let end = encoded.find('&').unwrap_or(encoded.len());
        if let Ok(decoded) = percent_decode(&encoded[..end]) {
            return decoded;
        }
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return raw.to_string();
    }
    if raw.starts_with("//") {
        return format!("https:{}", raw);
    }
    raw.to_string()
}

fn filter_domains(
    results: Vec<SearchResult>,
    allowed: &[String],
    blocked: &[String],
) -> Vec<SearchResult> {
    results
        .into_iter()
        .filter(|r| {
            let domain = match extract_domain(&r.url) {
                Some(d) => d,
                None => return true,
            };
            if !allowed.is_empty()
                && !allowed
                    .iter()
                    .any(|a| domain == a.to_lowercase() || domain.ends_with(&format!(".{}", a)))
            {
                return false;
            }
            if !blocked.is_empty()
                && blocked
                    .iter()
                    .any(|b| domain == b.to_lowercase() || domain.ends_with(&format!(".{}", b)))
            {
                return false;
            }
            true
        })
        .collect()
}

fn format_results(query: &str, results: &[SearchResult]) -> String {
    let mut out = format!("## Search results for: {}\n\n", query);
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!(
            "{}. [{}]({})\n   {}\n\n",
            i + 1,
            r.title,
            r.url,
            r.snippet
        ));
    }
    out.push_str("## Sources\n\n");
    for r in results {
        out.push_str(&format!("- [{}]({})\n", r.title, r.url));
    }
    out
}

fn percent_decode(input: &str) -> Result<String, ()> {
    let mut result = String::new();
    let mut bytes = input.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let hi = bytes.next().ok_or(())?;
            let lo = bytes.next().ok_or(())?;
            result.push((hex(hi)? << 4 | hex(lo)?) as char);
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    Ok(result)
}

fn hex(b: u8) -> Result<u8, ()> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_websearch_missing_query() {
        let tool = WebSearchTool::new();
        let result = tool
            .execute(serde_json::json!({}), &ToolContext::default())
            .await;
        assert!(result.is_error, "Should return error for missing query");
        assert!(
            result.content.contains("required"),
            "Error should mention 'required': {}",
            result.content
        );
    }

    #[tokio::test]
    async fn test_websearch_empty_query() {
        let tool = WebSearchTool::new();
        let result = tool
            .execute(serde_json::json!({"query": ""}), &ToolContext::default())
            .await;
        assert!(result.is_error, "Should return error for empty query");
    }

    #[tokio::test]
    async fn test_websearch_invalid_query_type() {
        let tool = WebSearchTool::new();
        let result = tool
            .execute(serde_json::json!({"query": 42}), &ToolContext::default())
            .await;
        assert!(result.is_error, "Should return error for non-string query");
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            extract_domain("https://www.example.com/path"),
            Some("www.example.com".to_string())
        );
        assert_eq!(
            extract_domain("http://docs.rust-lang.org/book"),
            Some("docs.rust-lang.org".to_string())
        );
        assert_eq!(extract_domain("not-a-url"), None);
    }

    #[test]
    fn test_filter_domains_allowed() {
        let results = vec![
            SearchResult {
                title: "Rust".into(),
                url: "https://www.rust-lang.org/".into(),
                snippet: "Rust".into(),
            },
            SearchResult {
                title: "Go".into(),
                url: "https://go.dev/".into(),
                snippet: "Go".into(),
            },
        ];
        let filtered = filter_domains(results, &["rust-lang.org".to_string()], &[]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Rust");
    }

    #[test]
    fn test_filter_domains_blocked() {
        let results = vec![
            SearchResult {
                title: "Good".into(),
                url: "https://good.com/".into(),
                snippet: "Good".into(),
            },
            SearchResult {
                title: "Spam".into(),
                url: "https://spam.com/".into(),
                snippet: "Spam".into(),
            },
        ];
        let filtered = filter_domains(results, &[], &["spam.com".to_string()]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Good");
    }

    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html("<b>Hello</b> &amp; world"), "Hello & world");
        assert_eq!(strip_html("no tags"), "no tags");
    }

    #[test]
    fn test_tool_name() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
    }

    #[test]
    fn test_tool_definition_schema() {
        let tool = WebSearchTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "web_search");
        let required = def.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|r| r.as_str() == Some("query")));
    }
}
