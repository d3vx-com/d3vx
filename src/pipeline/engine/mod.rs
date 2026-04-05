//! Pipeline Engine
//!
//! Core engine that orchestrates phase execution for tasks.
//! Manages handler registration and task progression through phases.

pub mod config;
pub mod engine_impl;

// Re-export all public types for backward compatibility
pub use config::{PhaseCallback, PipelineConfig, StatusCallback};
pub use engine_impl::{PipelineEngine, PipelineRunResult};
