//! Complete Task Tool
//!
//! Provides the `CompleteTaskTool` which agents must call to signify
//! that they have verified their changes and completed their assigned task.

use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Tool to explicitly mark a task as complete
#[derive(Clone, Default)]
pub struct CompleteTaskTool;

impl CompleteTaskTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CompleteTaskTool {
    fn name(&self) -> String {
        "complete_task".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Mark your assigned task as complete. Call this when you have finished the requested work. If you made code changes, verify them first (tests, type-check, dry-run) as appropriate for the task scope.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "string",
                        "description": "A summary of what was done and any verification that was performed."
                    }
                },
                "required": ["summary"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let summary = match input.get("summary").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::error("Missing 'summary'"),
        };

        ToolResult::success(format!("Task marked as complete. Summary: {}", summary))
    }
}
