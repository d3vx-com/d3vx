//! Agent configuration types and constants.

use serde::Serialize;

use crate::tools::AgentRole;

/// Maximum number of iterations before stopping to prevent runaway costs.
pub const DEFAULT_MAX_ITERATIONS: u32 = 200;

/// Maximum number of retries for transient API errors.
pub const MAX_RETRIES: u32 = 10;

/// Base delay in milliseconds for exponential backoff.
pub const BASE_DELAY_MS: u64 = 500;

/// Configuration for the agent loop.
#[derive(Debug, Clone, Serialize)]
pub struct AgentConfig {
    /// Model to use for completions.
    pub model: String,
    /// System prompt to use.
    pub system_prompt: String,
    /// Maximum number of iterations per turn.
    pub max_iterations: u32,
    /// Working directory for tool execution.
    pub working_dir: String,
    /// Session ID for tracking.
    pub session_id: String,
    /// Parent session ID when this agent is a delegated child.
    pub parent_session_id: Option<String>,
    /// Whether this loop belongs to an autonomous sub-agent (enforces TDD).
    pub is_subagent: bool,
    /// Current delegation depth. Top-level agents start at 0.
    pub delegation_depth: u8,
    /// Whether this agent may delegate to other agents.
    pub allow_parallel_spawn: bool,
    /// Agent role for tool access control.
    pub role: AgentRole,
    /// Recovery configuration.
    pub recovery: crate::config::types::RecoveryConfig,
    /// Sub-agent configuration.
    pub subagent: crate::config::types::SubAgentConfig,
    /// Whether thinking mode is enabled by default.
    pub thinking_enabled: bool,
    /// Thinking budget override.
    pub thinking_budget: Option<u32>,
    /// Read-only mode flag that blocks write tools.
    pub plan_mode: bool,
    /// Skip context compaction for faster sub-agent execution.
    pub skip_compaction: bool,
    /// Optional database handle for session state persistence.
    #[serde(skip)]
    pub db: Option<crate::store::database::DatabaseHandle>,
    /// Budget enforcement config.
    #[serde(skip)]
    pub budget: Option<crate::config::types::BudgetConfig>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            system_prompt: String::new(),
            max_iterations: DEFAULT_MAX_ITERATIONS,
            working_dir: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string()),
            session_id: uuid::Uuid::new_v4().to_string(),
            parent_session_id: None,
            is_subagent: false,
            delegation_depth: 0,
            allow_parallel_spawn: true,
            role: AgentRole::default(),
            recovery: crate::config::types::RecoveryConfig::default(),
            subagent: crate::config::types::SubAgentConfig::default(),
            thinking_enabled: true,
            thinking_budget: None,
            plan_mode: false,
            skip_compaction: false,
            db: None,
            budget: None,
        }
    }
}
