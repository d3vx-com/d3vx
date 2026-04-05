//! WebFetch Tool
//!
//! Fetch a URL and convert HTML to readable text.

use async_trait::async_trait;
use regex::Regex;
use std::time::Duration;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Maximum body size in bytes (1MB)
const MAX_BODY_SIZE: usize = 1024 * 1024;

/// Fetch timeout in seconds
const FETCH_TIMEOUT_SECS: u64 = 10;

/// WebFetch tool for fetching URLs
pub struct WebFetchTool {
    definition: ToolDefinition,
    client: reqwest::Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
            .user_agent("d3vx/1.0 (terminal coding agent)")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            definition: ToolDefinition {
                name: "WebFetch".to_string(),
                description: concat!(
                    "Fetch a URL and return its content as text. ",
                    "Converts HTML to readable text by stripping tags. ",
                    "Useful for reading documentation or API references."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "format": "uri",
                            "description": "URL to fetch"
                        }
                    },
                    "required": ["url"]
                }),
            },
            client,
        }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let url = input["url"].as_str().unwrap_or("");

        if url.is_empty() {
            return ToolResult::error("url is required");
        }

        // Validate URL
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return ToolResult::error("url must start with http:// or https://");
        }

        // Fetch the URL
        let response = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                if e.is_timeout() {
                    return ToolResult::error(format!(
                        "Request timed out after {} seconds",
                        FETCH_TIMEOUT_SECS
                    ));
                }
                if e.is_connect() {
                    return ToolResult::error(format!("Failed to connect to URL: {}", e));
                }
                return ToolResult::error(format!("Failed to fetch URL: {}", e));
            }
        };

        // Check HTTP status
        if !response.status().is_success() {
            return ToolResult::error(format!(
                "HTTP {} {} for {}",
                response.status().as_u16(),
                response.status().canonical_reason().unwrap_or("Unknown"),
                url
            ));
        }

        // Check content length
        if let Some(content_length) = response.content_length() {
            if content_length as usize > MAX_BODY_SIZE {
                return ToolResult::error(format!(
                    "Response too large: {} bytes (max {})",
                    content_length, MAX_BODY_SIZE
                ));
            }
        }

        // Get content type
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        // Read body
        let body = match response.text().await {
            Ok(b) => b,
            Err(e) => return ToolResult::error(format!("Failed to read response: {}", e)),
        };

        // Capture body length for metadata before processing
        let body_len = body.len();

        // Check actual body size
        if body_len > MAX_BODY_SIZE {
            return ToolResult::error(format!("Response body too large: {} bytes", body_len));
        }

        // Process based on content type
        let content = if content_type.contains("application/json") {
            // Pretty print JSON
            match serde_json::from_str::<serde_json::Value>(&body) {
                Ok(json) => serde_json::to_string_pretty(&json).unwrap_or(body),
                Err(_) => body,
            }
        } else if content_type.contains("text/plain") {
            body
        } else {
            // Convert HTML to text
            html_to_text(&body)
        };

        // Truncate if still too large
        let truncated = content.len() > MAX_BODY_SIZE;
        let final_content = if truncated {
            content[..MAX_BODY_SIZE].to_string()
        } else {
            content
        };

        let mut result = ToolResult::success(final_content)
            .with_metadata("bytesRead", serde_json::json!(body_len));

        if truncated {
            result = result.with_metadata("truncated", serde_json::json!(true));
        }

        result
    }
}

/// Static regex patterns for HTML to text conversion
/// Using lazy_static ensures regexes are compiled once and avoids runtime panics
use once_cell::sync::Lazy;

static SCRIPT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<script\b[^>]*>[\s\S]*?</script>").expect("Invalid script regex"));
static STYLE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<style\b[^>]*>[\s\S]*?</style>").expect("Invalid style regex"));
static NOSCRIPT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"<noscript\b[^>]*>[\s\S]*?</noscript>").expect("Invalid noscript regex")
});
static BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"</(p|div|li|tr|blockquote|pre|section|article)>").expect("Invalid block regex")
});
static BR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<br\s*/?>").expect("Invalid br regex"));
static HR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<hr\s*/?>").expect("Invalid hr regex"));
static HEADER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<h([1-6])\b[^>]*>(.*?)</h[1-6]>").expect("Invalid header regex"));
static LINK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<a\b[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#).expect("Invalid link regex")
});
static LI_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<li\b[^>]*>").expect("Invalid li regex"));
static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").expect("Invalid tag regex"));
static SPACES_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ \t]+").expect("Invalid spaces regex"));
static INDENT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\n[ \t]+").expect("Invalid indent regex"));
static NEWLINES_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\n{3,}").expect("Invalid newlines regex"));

/// Convert HTML to plain text using regex-based stripping
fn html_to_text(html: &str) -> String {
    let mut text = html.to_string();

    // Remove script, style, and noscript blocks
    text = SCRIPT_RE.replace_all(&text, "").to_string();
    text = STYLE_RE.replace_all(&text, "").to_string();
    text = NOSCRIPT_RE.replace_all(&text, "").to_string();

    // Convert block elements to newlines
    text = BLOCK_RE.replace_all(&text, "\n").to_string();

    // Convert br and hr
    text = BR_RE.replace_all(&text, "\n").to_string();
    text = HR_RE.replace_all(&text, "\n---\n").to_string();

    // Convert headers to markdown-style
    text = HEADER_RE
        .replace_all(&text, |caps: &regex::Captures| {
            let level: usize = caps[1].parse().unwrap_or(1);
            let content = &caps[2];
            format!("\n{} {}\n", "#".repeat(level), content)
        })
        .to_string();

    // Convert links to markdown
    text = LINK_RE
        .replace_all(&text, |caps: &regex::Captures| {
            format!("[{}]({})", &caps[2], &caps[1])
        })
        .to_string();

    // Convert list items
    text = LI_RE.replace_all(&text, "- ").to_string();

    // Strip all remaining tags
    text = TAG_RE.replace_all(&text, "").to_string();

    // Decode common entities
    text = text.replace("&amp;", "&");
    text = text.replace("&lt;", "<");
    text = text.replace("&gt;", ">");
    text = text.replace("&quot;", "\"");
    text = text.replace("&#39;", "'");
    text = text.replace("&nbsp;", " ");

    // Clean up whitespace
    text = SPACES_RE.replace_all(&text, " ").to_string();
    text = INDENT_RE.replace_all(&text, "\n").to_string();
    text = NEWLINES_RE.replace_all(&text, "\n\n").to_string();

    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_webfetch_missing_url() {
        let tool = WebFetchTool::new();
        let context = ToolContext::default();

        let result = tool.execute(serde_json::json!({}), &context).await;

        assert!(result.is_error);
        assert!(result.content.contains("required"));
    }

    #[tokio::test]
    async fn test_webfetch_invalid_url() {
        let tool = WebFetchTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(serde_json::json!({"url": "not-a-url"}), &context)
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("http://") || result.content.contains("https://"));
    }

    #[test]
    fn test_html_to_text_basic() {
        let html = "<html><body><h1>Title</h1><p>Paragraph</p></body></html>";
        let text = html_to_text(html);

        assert!(text.contains("# Title"));
        assert!(text.contains("Paragraph"));
    }

    #[test]
    fn test_html_to_text_links() {
        let html = r#"<a href="https://example.com">Example</a>"#;
        let text = html_to_text(html);

        assert!(text.contains("[Example](https://example.com)"));
    }

    #[test]
    fn test_html_to_text_lists() {
        let html = "<ul><li>Item 1</li><li>Item 2</li></ul>";
        let text = html_to_text(html);

        assert!(text.contains("- Item 1"));
        assert!(text.contains("- Item 2"));
    }

    #[test]
    fn test_html_to_text_entities() {
        let html = "<p>&amp; &lt; &gt; &quot;</p>";
        let text = html_to_text(html);

        assert!(text.contains("& < > \""));
    }

    #[test]
    fn test_html_to_text_removes_script() {
        let html = "<script>alert('xss')</script><p>Visible</p>";
        let text = html_to_text(html);

        assert!(!text.contains("alert"));
        assert!(text.contains("Visible"));
    }
}
