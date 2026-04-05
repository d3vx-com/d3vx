//! Heartbeat and Lease Management
//!
//! Provides worker health monitoring and lease lifecycle management.
//! Workers must send periodic heartbeats to maintain their leases.
//! Stale workers are detected and their tasks are requeued.
//!
//! # Lifecycle States
//!
//! ## Worker Lifecycle
//! - `Active` -> healthy worker
//! - `Degraded` -> heartbeat late but not stale
//! - `Stale` -> heartbeat too late, can be replaced
//! - `Replaced` -> replaced by another worker
//! - `Released` -> voluntarily released
//! - `Offline` -> not registered
//!
//! ## Lease Lifecycle
//! - `Active` -> lease valid
//! - `Expired` -> time-based expiry
//! - `Revoked` -> stale worker replaced
//! - `Replaced` -> newer lease acquired
//! - `Released` -> voluntarily released

pub mod manager;
#[cfg(test)]
mod tests;
pub mod types;

// Re-export all public types
pub use manager::HeartbeatManager;
pub use types::{
    Heartbeat, HeartbeatConfig, HeartbeatError, HeartbeatStats, LeaseId, LeaseLifecycle,
    LeaseResult, LeaseState, StaleWorkerInfo, WorkerHealth, WorkerHeartbeatState, WorkerLifecycle,
};
