//! Subagent-Driven Development (SDD) module
//!
//! Implements the spec → plan → decompose → execute → integrate workflow,
//! ensuring that agents stay aligned to their original intent instead of
//! drifting through hours of autonomous execution.
//!
//! # Architecture
//!
//! ```text
//! User input → SpecExtractor → TaskSpec
//!                    ↓
//! Plan phase output → PlanGate → Approved ExecutionPlan
//!                    ↓
//! SddDecomposer → DecompositionPlan (if complexity > threshold)
//!                    ↓
//! SddExecutor → Child agent results
//!                    ↓
//! SddIntegrator → IntegrationResult (conflict check, merge)
//! ```
//!
//! # Usage
//!
//! The [`SddWorkflow`] struct is the main entry point:
//!
//! ```rust,ignore
//! use d3vx::pipeline::sdd::{SddWorkflow, SddSession, SddConfig};
//!
//! let mut session = SddSession::new(task_id);
//! let workflow = SddWorkflow::with_defaults(planner, provider);
//! let result = workflow.run(user_input, &task, &context, plan_text, &mut session).await?;
//! ```

mod decomposer;
mod executor;
mod integrator;
mod plan_gate;
mod spec_extractor;
mod types;
mod workflow;

pub use decomposer::SddDecomposer;
pub use executor::{AgentProvider, SddExecutor};
pub use integrator::{IntegrationResult, SddIntegrator};
pub use plan_gate::PlanGate;
pub use spec_extractor::SpecExtractor;
pub use types::TaskSpec;
pub use types::{Scope, SddConfig, SddError, SddSession, SddState};
pub use workflow::{SddResult, SddWorkflow};
