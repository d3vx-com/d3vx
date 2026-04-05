//! Task delivery lifecycle state machine
//!
//! Extended lifecycle states for the autonomous delivery pipeline, tracking
//! tasks from agent spawning through PR creation, CI, review, and merge.

pub mod machine;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_transitions;
pub mod types;

// Re-export all public types at module level for backward compatibility.
pub use machine::DeliveryStateMachine;
pub use types::{DeliveryState, DeliveryStateTransition, LifecycleError, StateTrigger};
