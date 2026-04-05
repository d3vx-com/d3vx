//! Tool Types
//!
//! Core types for the tool system.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::types::{SandboxConfig, SandboxMode};

/// Tool definition schema (JSON Schema format)
pub type ToolSchema = serde_json::Value;

/// Definition of a tool for the LLM
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (e.g., "Bash", "Read")
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// JSON Schema for input parameters
    pub input_schema: ToolSchema,
}

/// Team/swarm membership context for tools running inside a coordinated swarm
#[derive(Debug, Clone)]
pub struct SwarmContext {
    /// Name of the swarm this agent belongs to
    pub swarm_name: String,
    /// Human-readable call sign for this agent (e.g., "backend-1")
    pub call_sign: String,
    /// Whether this agent is the swarm lead
    pub is_lead: bool,
}

/// Context provided to tools during execution
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Current working directory
    pub cwd: String,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Whether to auto-approve operations
    pub trust_mode: bool,
    /// Session ID for this execution
    pub session_id: Option<String>,
    /// Parent session ID when this tool call belongs to a delegated child agent
    pub parent_session_id: Option<String>,
    /// Current delegation depth. Top-level agents run at depth 0.
    pub agent_depth: u8,
    /// Whether this agent is allowed to create more agents.
    pub allow_parallel_spawn: bool,
    /// Compiled regex patterns for blocked bash commands
    pub bash_blocklist: Vec<regex::Regex>,
    /// Active sandbox mode for command execution
    pub sandbox_mode: SandboxMode,
    /// Sandbox configuration when sandboxing is active
    pub sandbox_config: Option<SandboxConfig>,
    /// Team/swarm membership context, if this agent belongs to a swarm
    pub swarm_membership: Option<SwarmContext>,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string()),
            env: std::env::vars().collect(),
            trust_mode: false,
            session_id: None,
            parent_session_id: None,
            agent_depth: 0,
            allow_parallel_spawn: true,
            bash_blocklist: vec![],
            sandbox_mode: SandboxMode::Disabled,
            sandbox_config: None,
            swarm_membership: None,
        }
    }
}

/// Result of tool execution
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Output content (text or structured data)
    pub content: String,
    /// Whether the tool execution failed
    pub is_error: bool,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ToolResult {
    /// Create a successful result with text output
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            metadata: HashMap::new(),
        }
    }

    /// Create an error result
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: message.into(),
            is_error: true,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the result
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Trait that all tools must implement
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool definition for the LLM
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given input
    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult;

    /// Get the tool name (default implementation)
    fn name(&self) -> String {
        self.definition().name
    }
}
