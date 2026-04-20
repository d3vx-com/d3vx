//! `advance_one_step` — the shared primitive that drives one phase.
//!
//! Both the autonomous chat flow and the traditional `--vex` flow
//! converge on this single primitive. That convergence is the whole
//! point of the planner: the markdown plan file is the protocol, and
//! one function is the only writer.
//!
//! The primitive is deliberately *narrow*:
//!
//! - It picks the first [`SectionState::NotStarted`] section in the
//!   plan.
//! - It runs that section's phase handler, via the existing
//!   [`create_handler`](crate::pipeline::handlers::create_handler).
//! - It records the outcome on the plan (success ⇒ `Completed`,
//!   any handler error ⇒ `Failed`) and returns.
//!
//! It does **not** serialise the plan back to disk, spawn worktrees,
//! or mutate the task store. Those concerns stay with the caller
//! (chat surface vs. vex daemon) — keeping this function free of
//! I/O side-effects is what lets both surfaces share it without
//! tripping over each other's storage conventions.

use std::sync::Arc;

use crate::agent::AgentLoop;
use crate::pipeline::handlers::{create_handler, PhaseError};
use crate::pipeline::phases::{PhaseContext, Task};

use super::errors::PlannerError;
use super::plan::{Plan, SectionState};

/// What happened when we advanced one step.
#[derive(Debug, Clone, PartialEq)]
pub struct StepOutcome {
    /// The phase we ran. Useful for logs and UI banners.
    pub phase_completed: crate::pipeline::phases::Phase,
    /// Final state of the section we just touched.
    pub section_state: SectionState,
    /// True if the plan has reached a terminal state (every section
    /// completed, or any section failed) after this step. Callers
    /// that want to loop should break on `is_done`.
    pub is_done: bool,
}

/// Advance the plan by exactly one phase. Returns the outcome for the
/// advanced section and whether the plan is now done.
///
/// Errors:
///
/// - [`PlannerError::AlreadyComplete`] if there are no pending
///   sections to run.
/// - [`PlannerError::PhaseHandler`] wrapping any handler failure —
///   the corresponding section is also marked `Failed` on the plan
///   before returning.
pub async fn advance_one_step(
    plan: &mut Plan,
    context: &PhaseContext,
    agent: Option<Arc<AgentLoop>>,
) -> Result<StepOutcome, PlannerError> {
    let index = plan
        .first_pending_section_index()
        .ok_or(PlannerError::AlreadyComplete)?;

    let phase = plan.sections[index].phase;
    let handler = create_handler(phase);

    // Handlers assert `task.phase == self.phase()` via `can_execute`,
    // so clone the task and align its phase before dispatching. We
    // do not mutate the caller's task — advance is side-effect-free
    // outside the plan itself.
    let mut task_for_phase: Task = context.task.clone();
    task_for_phase.phase = phase;

    match handler.execute(&task_for_phase, context, agent).await {
        Ok(result) if result.success => {
            plan.record_outcome(index, SectionState::Completed, result.output)?;
        }
        Ok(result) => {
            // Handler returned a non-success PhaseResult — treat the
            // joined error string as the section body so a reader of
            // the plan file can see why the phase did not pass.
            let body = if result.errors.is_empty() {
                "phase reported failure with no error text".to_string()
            } else {
                result.errors.join("\n")
            };
            plan.record_outcome(index, SectionState::Failed, body)?;
        }
        Err(e) => {
            plan.record_outcome(
                index,
                SectionState::Failed,
                phase_error_summary(&e),
            )?;
            return Err(PlannerError::PhaseHandler {
                phase: phase.to_string(),
                source: e,
            });
        }
    }

    let state = plan.sections[index].state;
    let is_done = plan.is_complete() || plan.any_failed();
    Ok(StepOutcome {
        phase_completed: phase,
        section_state: state,
        is_done,
    })
}

/// Short human-readable summary of a [`PhaseError`] suitable for
/// embedding in a plan section body. Uses `Display` rather than
/// `Debug` so the plan file stays operator-readable.
fn phase_error_summary(e: &PhaseError) -> String {
    format!("{e}")
}
