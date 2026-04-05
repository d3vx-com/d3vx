//! Commander Validation Agents
//!
//! Dedicated validation runners that execute type-check, test, and lint
//! commands in parallel and report structured results back.

pub mod runner;
pub mod tests;
pub mod types;

pub use runner::ValidationRunner;
pub use types::{ValidationCommand, ValidationKind, ValidationResult};
