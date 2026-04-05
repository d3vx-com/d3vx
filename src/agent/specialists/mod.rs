//! Specialized SDLC Agent Roles
//!
//! Defines specialized agent roles that can be spawned for specific tasks.
//! The orchestrator agent decides which specialized agents to use based on task context.
//! NO keyword-based detection - the AI decides based on task analysis.

mod methods;
mod prompts;
mod types;

pub use methods::SPECIALIST_AGENT_TYPES;
pub use types::{AgentType, SpecialistProfile};
