//! Pipeline Phase Types
//!
//! Defines the phases of the pipeline execution system.
//! Based on the 7-phase pipeline: Research -> Ideation -> Plan -> Draft -> Review -> Implement -> Docs

pub mod task;
#[cfg(test)]
mod tests;
pub mod types;

// Re-export all public types for backward compatibility
pub use task::{PhaseContext, Task};
pub use types::{ExecutionMode, Phase, Priority, TaskStatus};
