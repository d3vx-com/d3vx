//! Task Queue
//!
//! Priority-based task queue for managing multiple tasks in the pipeline.
//!
//! # Important
//!
//! The task queue should ONLY receive tasks from the PipelineOrchestrator.
//! Direct task creation bypasses the intake layer, classification, and checkpointing.
//! Use `PipelineOrchestrator::create_task_from_*` methods instead.

pub mod queries;
pub mod task_queue;
#[cfg(test)]
mod tests;
pub mod types;

// Re-export all public types
pub use task_queue::TaskQueue;
pub use types::{merge_json, QueueError, QueueStats, TaskDependency};
