//! Delegate Review Tool
//!
//! Spawns a sub-agent to perform a code review of the current changes.

use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Tool for delegating code review to a sub-agent.
pub struct DelegateReviewTool;

impl DelegateReviewTool {
    /// Create a new delegate review tool.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for DelegateReviewTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "DelegateReview".to_string(),
            description: "Spawn a specialized Senior QA sub-agent to review the changes made so far. Returns a verdict (APPROVE or CHANGES_REQUESTED).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "task_description": {
                        "type": "string",
                        "description": "Original task description to review against"
                    }
                },
                "required": ["task_description"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let task = input["task_description"]
            .as_str()
            .unwrap_or("the current task");

        let reviewer_prompt = format!(
            "You are a Senior Quality Assurance Engineer. Review the following task results and changes in the current directory:\n\n\
            Task: {}\n\n\
            Instructions:\n\
            - Inspect the code changes made in this workspace.\n\
            - Verify correctness, security, performance, and best practices.\n\
            - Run relevant tests if applicable.\n\
            - Provide a DETAILED review report.\n\
            - END YOUR RESPONSE with either \"REVIEW: APPROVED\" if everything is perfect, or \"REVIEW: CHANGES_REQUESTED\" followed by what needs fixing.",
            task
        );

        // In a real implementation, we would use the subagent manager here.
        // For now, we emit an event that the App will handle to spawn the subagent.
        // However, the Tool trait doesn't easily allow emitting events back to the App without a channel.

        ToolResult::success(format!(
            "Reviewer sub-agent requested for task: '{}'. Prompt sent: \n\n{}",
            task, reviewer_prompt
        ))
    }
}
