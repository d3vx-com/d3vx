//! Task run store for tracking task execution attempts
//!
//! Each task can have multiple runs (attempt numbers), enabling
//! retry tracking, execution history, and run-level metrics.

mod store;
#[cfg(test)]
mod tests;
mod types;

pub use store::TaskRunStore;
pub use types::*;
