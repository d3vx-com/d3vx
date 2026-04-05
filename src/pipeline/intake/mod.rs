//! Task Intake Layer
//!
//! Normalizes various trigger sources into consistent task records.
//! This is the entry point for all task creation in the system.

pub mod layer;
#[cfg(test)]
mod tests;
pub mod types;

// Re-export all public types for backward compatibility
pub use layer::TaskIntake;
pub use types::{TaskIntakeInput, TaskSource};
