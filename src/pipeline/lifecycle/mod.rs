//! Session lifecycle state machine.
//!
//! Tracks a session through provisioning, agent work, PR pipeline, CI, review,
//! merge, and exception phases with validated transitions and probe-based
//! phase detection.

pub mod hooks;
pub mod probe;
pub mod restore;
pub mod tracker;
pub mod types;

pub use probe::{probe_agent_status, CompositeProbe, GitProbe, TransitionProbe};
pub use restore::{
    generate_reconnect_command, ConflictCheckResult, RestoreCheck, RestoreError, RestoreOutcome,
    RestorePlan, RestoreSafetyChecker, RestoreStatus, SessionRestore,
};
pub use tracker::SessionTracker;
pub use types::{
    PhaseMetadata, PhaseTransition, SessionPhase, SessionSummary, TransitionCause, TransitionError,
};

#[cfg(test)]
mod tests;
