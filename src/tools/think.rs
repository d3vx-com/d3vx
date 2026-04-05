//! Think Tool
//!
//! Structured reasoning scratchpad for the LLM to think step-by-step
//! without executing side effects.

use async_trait::async_trait;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Think tool for structured reasoning
pub struct ThinkTool {
    definition: ToolDefinition,
}

impl ThinkTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "Think".to_string(),
                description: concat!(
                    "Use this tool to think step-by-step about a complex problem before taking action. ",
                    "Your thoughts will NOT be shown to the user. ",
                    "Use this when you need to reason about architecture, debug a tricky issue, ",
                    "or plan a multi-step approach."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "thought": {
                            "type": "string",
                            "description": "Your step-by-step reasoning about the problem. Think carefully."
                        }
                    },
                    "required": ["thought"]
                }),
            },
        }
    }
}

impl Default for ThinkTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ThinkTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let thought = input["thought"].as_str().unwrap_or("");

        if thought.is_empty() {
            return ToolResult::error("thought is required");
        }

        ToolResult::success(format!(
            "Thought recorded ({} chars). Continue with your plan.",
            thought.len()
        ))
        .with_metadata("thoughtLength", serde_json::json!(thought.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_think_basic() {
        let tool = ThinkTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "thought": "Let me think about this problem step by step..."
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Thought recorded"));
        assert!(result.content.contains("47 chars"));
    }

    #[tokio::test]
    async fn test_think_empty() {
        let tool = ThinkTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(serde_json::json!({"thought": ""}), &context)
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("required"));
    }

    #[tokio::test]
    async fn test_think_metadata() {
        let tool = ThinkTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "thought": "Thinking..."
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert_eq!(
            result.metadata.get("thoughtLength"),
            Some(&serde_json::json!(11))
        );
    }
}
