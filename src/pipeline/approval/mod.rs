//! Approval Flow Module
//!
//! Provides a planner/executor split with a user-approval gate between
//! plan generation and execution.
//!
//! # Architecture
//!
//! ```text
//! Task → Planner (builds ExecutionPlan)
//!              ↓
//!        ApprovalFlow (auto-approve or gate)
//!              ↓
//!   ┌─ Approved ──→ Executor agents proceed
//!   ├─ Rejected ──→ Plan revised or task cancelled
//!   └─ Expired  ──→ Timeout, task paused
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use d3vx::pipeline::approval::{
//!     Planner, ApprovalConfig, ApprovalState, ExecutionPlan,
//! };
//!
//! async fn example() {
//!     let planner = Planner::with_defaults();
//!
//!     // Build a plan from agent output
//!     let plan = planner.build_plan(&task, &context, agent_output);
//!
//!     // Submit for approval (blocks until decided or timeout)
//!     match planner.submit_for_approval(plan).await {
//!         Ok(ApprovalState::Approved) => { /* proceed */ },
//!         Ok(ApprovalState::Rejected) => { /* handle rejection */ },
//!         Err(_) => { /* timeout or error */ },
//!         _ => {}
//!     }
//! }
//! ```

pub mod flow;
pub mod planner;
pub mod types;

// Re-export public API
pub use flow::{ApprovalFlow, SubmitResult};
pub use planner::Planner;
pub use types::{
    ApprovalConfig, ApprovalDecision, ApprovalError, ApprovalState, ExecutionPlan, PlanStep,
    RiskLevel,
};
