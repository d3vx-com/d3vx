//! MCP Tool Implementation
//!
//! A proxy tool that forwards tool execution requests to an MCP server.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::mcp::manager::McpManager;
use crate::mcp::protocol::{McpContent, McpToolDefinition};
use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};

pub struct McpTool {
    server_name: String,
    manager: Arc<McpManager>,
    definition: McpToolDefinition,
}

impl McpTool {
    pub fn new(
        server_name: String,
        manager: Arc<McpManager>,
        definition: McpToolDefinition,
    ) -> Self {
        Self {
            server_name,
            manager,
            definition,
        }
    }
}

#[async_trait]
impl Tool for McpTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: format!("mcp__{}__{}", self.server_name, self.definition.name),
            description: self
                .definition
                .description
                .clone()
                .unwrap_or_else(|| format!("MCP tool from server '{}'", self.server_name)),
            input_schema: self.definition.input_schema.clone(),
        }
    }

    async fn execute(&self, input: Value, _context: &ToolContext) -> ToolResult {
        match self
            .manager
            .call_tool(&self.server_name, &self.definition.name, input)
            .await
        {
            Ok(result) => {
                let mut combined_content = String::new();
                for content in result.content {
                    match content {
                        McpContent::Text { text } => {
                            if !combined_content.is_empty() {
                                combined_content.push_str("\n\n");
                            }
                            combined_content.push_str(&text);
                        }
                        McpContent::Image { .. } => {
                            combined_content
                                .push_str("\n[Image content received but not renderable in TUI]");
                        }
                        McpContent::Resource { resource } => {
                            combined_content
                                .push_str(&format!("\n[Resource content]: {}", resource));
                        }
                    }
                }

                if result.is_error {
                    ToolResult::error(combined_content)
                } else {
                    ToolResult::success(combined_content)
                }
            }
            Err(e) => ToolResult::error(format!("MCP execution failed: {}", e)),
        }
    }
}
