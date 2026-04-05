//! Orchestrator Meta-Agent
//!
//! The coordinator meta-agent manages multiple concurrent agent sessions,
//! making decisions about spawning, nudging, killing, and monitoring sessions.

pub mod coordinator_tools;
pub mod prompts;
pub mod types;

// Re-export core types
pub use coordinator_tools::{
    BatchLaunchTool, GetStatusTool, KillSessionTool, LaunchAgentTool, ListSessionsTool,
    SendNudgeTool,
};
pub use prompts::CoordinatorPromptBuilder;
pub use types::{CoordinatorAction, CoordinatorDecision, CoordinatorState, CoordinatorTool};
