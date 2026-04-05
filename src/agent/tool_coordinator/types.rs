//! Tool coordinator types and trait definitions

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::tools::{Tool, ToolContext, ToolResult};

/// Result of a tool execution with metadata.
#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    /// The tool use ID
    pub id: String,
    /// The tool name
    pub name: String,
    /// The execution result
    pub result: ToolResult,
    /// Execution time in milliseconds
    pub elapsed_ms: u64,
}

impl ToolExecutionResult {
    /// Create a new execution result.
    pub fn new(id: String, name: String, result: ToolResult, elapsed_ms: u64) -> Self {
        Self {
            id,
            name,
            result,
            elapsed_ms,
        }
    }
}

/// Tool definition for the coordinator (uses serde_json::Value for schema)
#[derive(Debug, Clone)]
pub struct CoordinatorToolDefinition {
    /// Tool name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// JSON Schema for input parameters
    pub input_schema: serde_json::Value,
}

/// Trait for custom tool handlers.
///
/// Implement this trait to create custom tools that can be registered
/// with the tool coordinator.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Get the tool definition.
    fn definition(&self) -> CoordinatorToolDefinition;

    /// Execute the tool with the given input.
    async fn execute(
        &self,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolCoordinatorError>;
}

/// Adapter to implement ToolHandler from the Tool trait.
pub(crate) struct ToolAdapter {
    pub(crate) tool: Arc<dyn Tool>,
}

impl ToolAdapter {
    pub(crate) fn new(tool: Arc<dyn Tool>) -> Self {
        Self { tool }
    }
}

#[async_trait]
impl ToolHandler for ToolAdapter {
    fn definition(&self) -> CoordinatorToolDefinition {
        let def = self.tool.definition();
        CoordinatorToolDefinition {
            name: def.name,
            description: def.description,
            input_schema: def.input_schema,
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult, ToolCoordinatorError> {
        Ok(self.tool.execute(input, context).await)
    }
}

/// A tool handler that emits an event to spawn a sub-agent.
pub struct SubAgentToolHandler {
    event_tx: mpsc::Sender<crate::event::Event>,
}

impl SubAgentToolHandler {
    pub fn new(event_tx: mpsc::Sender<crate::event::Event>) -> Self {
        Self { event_tx }
    }
}

#[async_trait]
impl ToolHandler for SubAgentToolHandler {
    fn definition(&self) -> CoordinatorToolDefinition {
        CoordinatorToolDefinition {
            name: "SpawnAgent".to_string(),
            description: "Spawn a sub-agent to perform a sub-task in parallel. Use this for independent research or complex multi-step processes.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "task": {
                        "type": "string",
                        "description": "Specific goal or task for the sub-agent"
                    }
                },
                "required": ["task"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, ToolCoordinatorError> {
        let task = input["task"]
            .as_str()
            .ok_or_else(|| ToolCoordinatorError::InvalidInput("Missing 'task' field".to_string()))?
            .to_string();

        let _ = self.event_tx.try_send(crate::event::Event::Agent(
            super::super::agent_loop::AgentEvent::SubAgentSpawn { task: task.clone() },
        ));

        Ok(ToolResult::success(format!(
            "Sub-agent requested for task: '{}'",
            task
        )))
    }
}

/// Error type for tool coordinator operations.
#[derive(Debug, thiserror::Error)]
pub enum ToolCoordinatorError {
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid tool input: {0}")]
    InvalidInput(String),

    #[error("Tool registration failed: {0}")]
    RegistrationFailed(String),

    #[error("Permission denied for tool: {0}")]
    PermissionDenied(String),
}
