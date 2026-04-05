//! MCP Resource Tools
//!
//! Provides `ListMcpResourcesTool` and `ReadMcpResourceTool` for discovering
//! and reading MCP server resources. Currently stubbed until the MCP manager
//! integration is wired in.

use async_trait::async_trait;
use serde_json::json;
use tracing::debug;

use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Tool to list available MCP resources from connected servers
#[derive(Clone, Default)]
pub struct ListMcpResourcesTool;

impl ListMcpResourcesTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ListMcpResourcesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "list_mcp_resources".to_string(),
            description: "List available MCP resources from connected servers. Optionally filter by server name.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server": {
                        "type": "string",
                        "description": "Optional server name to filter resources"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let server = input.get("server").and_then(|v| v.as_str());
        debug!(server =? server, "Listing MCP resources");
        ToolResult::success(json!({"resources": [], "message": "MCP resource discovery requires active server connections"}).to_string())
    }
}

/// Tool to read a specific MCP resource by URI
#[derive(Clone, Default)]
pub struct ReadMcpResourceTool;

impl ReadMcpResourceTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ReadMcpResourceTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_mcp_resource".to_string(),
            description: "Read a specific MCP resource from a connected server by its URI."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server": { "type": "string", "description": "The MCP server name to read from" },
                    "uri": { "type": "string", "description": "The URI of the resource to read" }
                },
                "required": ["server", "uri"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let server = match input.get("server").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            _ => return ToolResult::error("Missing or empty required field: server"),
        };
        let uri = match input.get("uri").and_then(|v| v.as_str()) {
            Some(u) if !u.is_empty() => u,
            _ => return ToolResult::error("Missing or empty required field: uri"),
        };
        debug!(server = server, uri = uri, "Reading MCP resource");
        ToolResult::success(json!({"server": server, "uri": uri, "message": "MCP resource reading requires active server connections"}).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn exec_list(input: serde_json::Value) -> ToolResult {
        ListMcpResourcesTool
            .execute(input, &ToolContext::default())
            .await
    }

    async fn exec_read(input: serde_json::Value) -> ToolResult {
        ReadMcpResourceTool
            .execute(input, &ToolContext::default())
            .await
    }

    #[tokio::test]
    async fn test_list_no_filter() {
        let r = exec_list(json!({})).await;
        assert!(!r.is_error);
        assert!(r.content.contains("resources"));
        assert!(r.content.contains("active server connections"));
    }

    #[tokio::test]
    async fn test_list_with_server() {
        let r = exec_list(json!({"server": "my-server"})).await;
        assert!(!r.is_error);
    }

    #[tokio::test]
    async fn test_read_valid() {
        let r = exec_read(json!({"server": "my-server", "uri": "file:///data.txt"})).await;
        assert!(!r.is_error);
        assert!(r.content.contains("my-server"));
        assert!(r.content.contains("file:///data.txt"));
    }

    #[tokio::test]
    async fn test_read_missing_server() {
        let r = exec_read(json!({"uri": "file:///data.txt"})).await;
        assert!(r.is_error);
        assert!(r.content.contains("server"));
    }

    #[tokio::test]
    async fn test_read_missing_uri() {
        let r = exec_read(json!({"server": "my-server"})).await;
        assert!(r.is_error);
        assert!(r.content.contains("uri"));
    }

    #[tokio::test]
    async fn test_read_empty_fields() {
        let r = exec_read(json!({"server": "", "uri": ""})).await;
        assert!(r.is_error);
    }
}
