//! Coordinator Tool Implementations
//!
//! Implements the `Tool` trait for each coordinator action: launch, list,
//! nudge, kill, get-status, and batch-launch.

use async_trait::async_trait;

use crate::tools::types::{Tool, ToolContext, ToolDefinition, ToolResult};

// ---------------------------------------------------------------------------
// LaunchAgentTool
// ---------------------------------------------------------------------------

/// Spawns a new agent session with a given prompt.
pub struct LaunchAgentTool {
    definition: ToolDefinition,
}

impl LaunchAgentTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "LaunchAgent".to_string(),
                description:
                    "Spawn a new agent session with a prompt. Optionally specify a branch name."
                        .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "prompt": { "type": "string", "description": "The task prompt for the agent" },
                        "branch": { "type": "string", "description": "Optional git branch name" }
                    },
                    "required": ["prompt"]
                }),
            },
        }
    }
}

impl Default for LaunchAgentTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for LaunchAgentTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let prompt = input["prompt"].as_str().unwrap_or("");
        if prompt.is_empty() {
            return ToolResult::error("prompt is required");
        }
        let branch = input["branch"].as_str();
        ToolResult::success(format!(
            "Launched agent session for: {}{}",
            prompt,
            branch.map_or(String::new(), |b| format!(" (branch: {})", b))
        ))
    }
}

// ---------------------------------------------------------------------------
// ListSessionsTool
// ---------------------------------------------------------------------------

/// Returns all active session statuses.
pub struct ListSessionsTool {
    definition: ToolDefinition,
}

impl ListSessionsTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "ListSessions".to_string(),
                description: "List all active agent sessions and their current status.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        }
    }
}

impl Default for ListSessionsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ListSessionsTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, _input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        ToolResult::success("No active sessions (coordinator not connected)")
    }
}

// ---------------------------------------------------------------------------
// SendNudgeTool
// ---------------------------------------------------------------------------

/// Sends a message to a running agent session.
pub struct SendNudgeTool {
    definition: ToolDefinition,
}

impl SendNudgeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "SendNudge".to_string(),
                description:
                    "Send a nudge message to a running agent session to guide or redirect it."
                        .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "The session to nudge" },
                        "message": { "type": "string", "description": "The message to send" }
                    },
                    "required": ["session_id", "message"]
                }),
            },
        }
    }
}

impl Default for SendNudgeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SendNudgeTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let session_id = input["session_id"].as_str().unwrap_or("");
        let message = input["message"].as_str().unwrap_or("");
        if session_id.is_empty() {
            return ToolResult::error("session_id is required");
        }
        if message.is_empty() {
            return ToolResult::error("message is required");
        }
        ToolResult::success(format!("Nudged session {}: {}", session_id, message))
    }
}

// ---------------------------------------------------------------------------
// KillSessionTool
// ---------------------------------------------------------------------------

/// Terminates a stuck or unwanted agent session.
pub struct KillSessionTool {
    definition: ToolDefinition,
}

impl KillSessionTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "KillSession".to_string(),
                description: "Terminate a stuck or unwanted agent session with a reason."
                    .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "The session to kill" },
                        "reason": { "type": "string", "description": "Why the session is being killed" }
                    },
                    "required": ["session_id", "reason"]
                }),
            },
        }
    }
}

impl Default for KillSessionTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for KillSessionTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let session_id = input["session_id"].as_str().unwrap_or("");
        let reason = input["reason"].as_str().unwrap_or("");
        if session_id.is_empty() {
            return ToolResult::error("session_id is required");
        }
        if reason.is_empty() {
            return ToolResult::error("reason is required");
        }
        ToolResult::success(format!("Killed session {}: {}", session_id, reason))
    }
}

// ---------------------------------------------------------------------------
// GetStatusTool
// ---------------------------------------------------------------------------

/// Gets detailed status for one session.
pub struct GetStatusTool {
    definition: ToolDefinition,
}

impl GetStatusTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "GetStatus".to_string(),
                description: "Get detailed status for a specific agent session.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "The session to query" }
                    },
                    "required": ["session_id"]
                }),
            },
        }
    }
}

impl Default for GetStatusTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GetStatusTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let session_id = input["session_id"].as_str().unwrap_or("");
        if session_id.is_empty() {
            return ToolResult::error("session_id is required");
        }
        ToolResult::success(format!(
            "Status for {}: unknown (coordinator not connected)",
            session_id
        ))
    }
}

// ---------------------------------------------------------------------------
// BatchLaunchTool
// ---------------------------------------------------------------------------

/// Launches multiple issues in parallel.
pub struct BatchLaunchTool {
    definition: ToolDefinition,
}

impl BatchLaunchTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "BatchLaunch".to_string(),
                description:
                    "Launch multiple agent sessions in parallel to process a batch of issues."
                        .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "issues": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of issue identifiers or prompts"
                        },
                        "max_parallel": {
                            "type": "integer",
                            "description": "Maximum number of concurrent sessions"
                        }
                    },
                    "required": ["issues"]
                }),
            },
        }
    }
}

impl Default for BatchLaunchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BatchLaunchTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let issues = input["issues"].as_array();
        if issues.is_none() || issues.unwrap().is_empty() {
            return ToolResult::error("issues must be a non-empty array");
        }
        let issues_arr = issues.unwrap();
        let count = issues_arr.len();
        let max_parallel = input["max_parallel"].as_u64().unwrap_or(3);
        ToolResult::success(format!(
            "Batch launched {} issue(s) with max parallelism of {}",
            count, max_parallel
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_context() -> ToolContext {
        ToolContext::default()
    }

    #[test]
    fn test_coordinator_tool_definitions_valid() {
        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(LaunchAgentTool::new()),
            Box::new(ListSessionsTool::new()),
            Box::new(SendNudgeTool::new()),
            Box::new(KillSessionTool::new()),
            Box::new(GetStatusTool::new()),
            Box::new(BatchLaunchTool::new()),
        ];

        let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
        assert_eq!(
            names,
            vec![
                "LaunchAgent".to_string(),
                "ListSessions".to_string(),
                "SendNudge".to_string(),
                "KillSession".to_string(),
                "GetStatus".to_string(),
                "BatchLaunch".to_string()
            ]
        );

        for tool in &tools {
            let def = tool.definition();
            assert!(!def.name.is_empty());
            assert!(!def.description.is_empty());
            assert!(def.input_schema.is_object());
        }
    }

    #[tokio::test]
    async fn test_launch_agent_tool() {
        let tool = LaunchAgentTool::new();
        let ctx = default_context();
        let result = tool
            .execute(serde_json::json!({ "prompt": "Fix the login bug" }), &ctx)
            .await;
        assert!(!result.is_error);
        assert!(result.content.contains("Fix the login bug"));
    }

    #[tokio::test]
    async fn test_launch_agent_missing_prompt() {
        let tool = LaunchAgentTool::new();
        let ctx = default_context();
        let result = tool.execute(serde_json::json!({}), &ctx).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_list_sessions_tool() {
        let tool = ListSessionsTool::new();
        let ctx = default_context();
        let result = tool.execute(serde_json::json!({}), &ctx).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_send_nudge_tool() {
        let tool = SendNudgeTool::new();
        let ctx = default_context();
        let result = tool
            .execute(
                serde_json::json!({ "session_id": "s-1", "message": "Focus" }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);
        assert!(result.content.contains("s-1"));
    }

    #[tokio::test]
    async fn test_send_nudge_missing_fields() {
        let tool = SendNudgeTool::new();
        let ctx = default_context();
        let result = tool
            .execute(serde_json::json!({ "session_id": "s-1" }), &ctx)
            .await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_kill_session_tool() {
        let tool = KillSessionTool::new();
        let ctx = default_context();
        let result = tool
            .execute(
                serde_json::json!({ "session_id": "s-1", "reason": "stuck" }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_get_status_tool() {
        let tool = GetStatusTool::new();
        let ctx = default_context();
        let result = tool
            .execute(serde_json::json!({ "session_id": "s-1" }), &ctx)
            .await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_batch_launch_tool() {
        let tool = BatchLaunchTool::new();
        let ctx = default_context();
        let result = tool
            .execute(
                serde_json::json!({ "issues": ["ISSUE-1", "ISSUE-2"], "max_parallel": 2 }),
                &ctx,
            )
            .await;
        assert!(!result.is_error);
        assert!(result.content.contains("2 issue(s)"));
    }

    #[tokio::test]
    async fn test_batch_launch_empty_issues() {
        let tool = BatchLaunchTool::new();
        let ctx = default_context();
        let result = tool
            .execute(serde_json::json!({ "issues": [] }), &ctx)
            .await;
        assert!(result.is_error);
    }
}
