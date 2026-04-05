//! Worker Pool
//!
//! Manages available workers for task execution with lease semantics.

pub mod pool;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use pool::WorkerPool;
pub use types::{
    Worker, WorkerId, WorkerLease, WorkerPoolConfig, WorkerPoolError, WorkerPoolManager,
    WorkerPoolStats, WorkerStatus,
};
