//! Plan Mode Tool -- EnterPlanMode / ExitPlanMode
//!
//! Allows the agent to enter a planning phase (read-only exploration)
//! and exit after the user approves the plan.

use async_trait::async_trait;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Tool for entering plan mode (read-only exploration).
pub struct EnterPlanModeTool {
    definition: ToolDefinition,
}

/// Tool for exiting plan mode (requires user approval).
pub struct ExitPlanModeTool {
    definition: ToolDefinition,
}

impl EnterPlanModeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "EnterPlanMode".to_string(),
                description: concat!(
                    "Switch to planning mode. You can explore the codebase and design ",
                    "implementation plans but cannot modify files. Submit your plan ",
                    "for user approval before implementing."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "plan_description": {
                            "type": "string",
                            "description": "A brief description of what you plan to investigate and design."
                        }
                    },
                    "required": ["plan_description"]
                }),
            },
        }
    }
}

impl Default for EnterPlanModeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let description = input["plan_description"].as_str().unwrap_or("");

        if description.is_empty() {
            return ToolResult::error("plan_description is required");
        }

        ToolResult::success(format!(
            "Entered plan mode. You can now explore the codebase and design your approach.\n\n\
             Plan: {}\n\n\
             Restrictions:\n\
             - Read-only operations only (Read, Grep, Glob, Bash for read commands)\n\
             - No file modifications (Write, Edit, MultiEdit)\n\
             - Use ExitPlanMode to submit your plan for user approval.",
            description
        ))
        .with_metadata("planMode", serde_json::json!(true))
        .with_metadata("planDescription", serde_json::json!(description))
    }
}

impl ExitPlanModeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "ExitPlanMode".to_string(),
                description: concat!(
                    "Submit your implementation plan for user approval. ",
                    "Once approved, you can proceed with implementation."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "plan_summary": {
                            "type": "string",
                            "description": "Summary of the implementation plan for user review."
                        },
                        "allowed_prompts": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "tool": {
                                        "type": "string",
                                        "description": "Tool name (e.g., Write, Edit, Bash)."
                                    },
                                    "prompt": {
                                        "type": "string",
                                        "description": "Description of what this tool call will do."
                                    }
                                },
                                "required": ["tool", "prompt"]
                            },
                            "description": "List of tool operations the plan requires."
                        }
                    },
                    "required": ["plan_summary"]
                }),
            },
        }
    }
}

impl Default for ExitPlanModeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let summary = input["plan_summary"].as_str().unwrap_or("");

        if summary.is_empty() {
            return ToolResult::error("plan_summary is required");
        }

        let allowed_prompts = input["allowed_prompts"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let tool = item["tool"].as_str().unwrap_or("").to_string();
                        let prompt = item["prompt"].as_str().unwrap_or("").to_string();
                        if tool.is_empty() || prompt.is_empty() {
                            None
                        } else {
                            Some((tool, prompt))
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut message = format!(
            "Plan submitted for user review.\n\n\
             == Implementation Plan ==\n\
             {}\n",
            summary
        );

        if !allowed_prompts.is_empty() {
            message.push_str("\nPlanned operations:\n");
            for (tool, prompt) in &allowed_prompts {
                message.push_str(&format!("  - [{}] {}\n", tool, prompt));
            }
        }

        message.push_str("\nAwaiting user approval before proceeding.");

        let tool_names: Vec<serde_json::Value> = allowed_prompts
            .iter()
            .map(|(tool, _)| serde_json::json!(tool))
            .collect();

        ToolResult::success(message)
            .with_metadata("planMode", serde_json::json!(false))
            .with_metadata("planSummary", serde_json::json!(summary))
            .with_metadata("allowedTools", serde_json::json!(tool_names))
            .with_metadata("requiresApproval", serde_json::json!(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- EnterPlanModeTool tests --

    #[tokio::test]
    async fn test_enter_plan_mode_basic() {
        let tool = EnterPlanModeTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "plan_description": "Investigate the auth module and design a refactor"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Entered plan mode"));
        assert!(result.content.contains("Investigate the auth module"));
        assert!(result.content.contains("Read-only operations only"));
    }

    #[tokio::test]
    async fn test_enter_plan_mode_empty_description() {
        let tool = EnterPlanModeTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(serde_json::json!({"plan_description": ""}), &context)
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("required"));
    }

    #[tokio::test]
    async fn test_enter_plan_mode_metadata() {
        let tool = EnterPlanModeTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "plan_description": "Test plan"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert_eq!(
            result.metadata.get("planMode"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            result.metadata.get("planDescription"),
            Some(&serde_json::json!("Test plan"))
        );
    }

    #[tokio::test]
    async fn test_enter_plan_mode_definition() {
        let tool = EnterPlanModeTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "EnterPlanMode");
        assert!(def.description.contains("planning mode"));
    }

    // -- ExitPlanModeTool tests --

    #[tokio::test]
    async fn test_exit_plan_mode_basic() {
        let tool = ExitPlanModeTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "plan_summary": "Refactor auth module into separate concerns"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Plan submitted"));
        assert!(result.content.contains("Refactor auth module"));
        assert!(result.content.contains("Awaiting user approval"));
    }

    #[tokio::test]
    async fn test_exit_plan_mode_with_allowed_prompts() {
        let tool = ExitPlanModeTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "plan_summary": "Fix the bug in parsing",
                    "allowed_prompts": [
                        { "tool": "Edit", "prompt": "Fix the off-by-one error in parser.rs" },
                        { "tool": "Bash", "prompt": "Run cargo test to verify the fix" }
                    ]
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Planned operations:"));
        assert!(result.content.contains("[Edit] Fix the off-by-one error"));
        assert!(result.content.contains("[Bash] Run cargo test"));
    }

    #[tokio::test]
    async fn test_exit_plan_mode_empty_summary() {
        let tool = ExitPlanModeTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(serde_json::json!({"plan_summary": ""}), &context)
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("required"));
    }

    #[tokio::test]
    async fn test_exit_plan_mode_filters_invalid_prompts() {
        let tool = ExitPlanModeTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "plan_summary": "Do the thing",
                    "allowed_prompts": [
                        { "tool": "Edit", "prompt": "Change file" },
                        { "tool": "", "prompt": "Missing tool name" },
                        { "tool": "Bash", "prompt": "" }
                    ]
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("[Edit] Change file"));
        assert!(!result.content.contains("Missing tool name"));
        // Bash with empty prompt is filtered out
        assert!(!result.content.contains("[Bash]"));
    }

    #[tokio::test]
    async fn test_exit_plan_mode_metadata() {
        let tool = ExitPlanModeTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "plan_summary": "Plan summary text",
                    "allowed_prompts": [
                        { "tool": "Write", "prompt": "Create new module" }
                    ]
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert_eq!(
            result.metadata.get("planMode"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(
            result.metadata.get("planSummary"),
            Some(&serde_json::json!("Plan summary text"))
        );
        assert_eq!(
            result.metadata.get("allowedTools"),
            Some(&serde_json::json!(["Write"]))
        );
        assert_eq!(
            result.metadata.get("requiresApproval"),
            Some(&serde_json::json!(true))
        );
    }

    #[tokio::test]
    async fn test_exit_plan_mode_definition() {
        let tool = ExitPlanModeTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "ExitPlanMode");
        assert!(def.description.contains("user approval"));
    }
}
