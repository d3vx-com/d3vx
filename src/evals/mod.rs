//! Evaluation harness for measuring agent performance.
//!
//! # Overview
//!
//! An *eval* is a reproducible task — instruction + setup + grading rules —
//! that an agent attempts. The harness runs the task in an isolated
//! workspace, lets the agent work, then grades the result against
//! declarative rules. A collection of tasks produces a pass-rate and
//! cost/duration profile — the raw material for "is this change actually
//! better?" decisions.
//!
//! # Design
//!
//! The harness is split into four concerns, one per submodule:
//!
//! | Submodule       | Owns                                            |
//! |-----------------|-------------------------------------------------|
//! | [`task`]        | Task definition, TOML loading, defaults         |
//! | [`grader`]      | Grading rules and pass/fail adjudication        |
//! | [`environment`] | Isolated workspace provisioning and cleanup     |
//! | [`result`]      | Per-task results and aggregate reporting        |
//!
//! Agent execution itself is *not* part of this module — tasks are run by
//! external code (CLI subcommand or integration tests) that knows how to
//! drive an agent. This keeps the grading layer testable in isolation and
//! lets the eval harness live without coupling to any specific runtime.

pub mod environment;
pub mod grader;
pub mod metrics;
pub mod result;
pub mod runner;
pub mod task;

#[cfg(test)]
mod tests;

pub use environment::{EvalEnvironment, EnvironmentError};
pub use grader::{GradeOutcome, GraderSpec};
pub use metrics::AgentMetrics;
pub use result::{EvalReport, EvalResult, ReportFormat};
pub use runner::{AgentDriver, DriverError, EvalRunner};
pub use task::{EvalTask, TaskError, TaskLoadError};
