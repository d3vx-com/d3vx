//! Event store for task lifecycle audit trail
//!
//! Append-only event log that tracks everything that happens
//! to tasks, runs, and related entities.

mod store;
#[cfg(test)]
mod tests;
mod types;

pub use store::{emit_state_change, emit_worker_assigned, EventStore};
pub use types::*;
