//! Pipeline Orchestrator
//!
//! Central authority for all task operations in the d3vx system.
//! The orchestrator is the SINGLE entry point for creating, transitioning, and executing tasks.

pub mod config;
pub mod orchestrator;
pub mod reaction_bridge;
pub mod trait_impl;

// Re-export all public types for backward compatibility
pub use config::OrchestratorConfig;
pub use orchestrator::PipelineOrchestrator;
pub use reaction_bridge::{ReactionBridge, ReactionOutcome};
pub use trait_impl::TaskAuthority;
