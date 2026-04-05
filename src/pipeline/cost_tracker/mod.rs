//! Pipeline Cost Tracker
//!
//! Tracks API usage costs and enforces budget limits.
//! Follows Interface Segregation Principle - separate concerns for tracking vs enforcement.

#[cfg(test)]
mod tests;
pub mod tracker;
pub mod types;

// Re-export all public types for backward compatibility
pub use tracker::CostTracker;
pub use types::{estimate_cost, ApiUsage, CostStats, CostTrackerConfig, CostTrackerError};
