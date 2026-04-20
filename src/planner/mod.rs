//! Planner: autonomous phase-selection over the 7-phase pipeline.
//!
//! # The product sentence
//!
//! d3vx reads what you asked for, decides what it takes to do it,
//! writes that decision down, and does it.
//!
//! # What this module owns
//!
//! The planner is the **decision layer** that sits between a user
//! message and the existing phase handlers. Every chat turn starts
//! with the model emitting a structured decision — which subset of
//! phases this task needs (possibly zero) — and a markdown plan file
//! becomes the source of truth for execution and resume.
//!
//! Responsibilities split across submodules:
//!
//! | Submodule   | Owns                                                  |
//! |-------------|-------------------------------------------------------|
//! | [`phase`]   | `PhaseSelection` — ordered subset of `pipeline::Phase`|
//! | [`plan`]    | `Plan` struct: the data model for a markdown plan     |
//! | [`markdown`]| Parse / serialise a `Plan` to and from markdown       |
//! | [`preamble`]| The system-prompt block that teaches the AI to decide |
//! | [`decision`]| Parse the AI's first-turn decision into `PhaseSelection` |
//! | [`advance`] | `advance_one_step` — drive one phase to completion    |
//! | [`errors`]  | Single unified error type                             |
//!
//! # What this module does *not* own
//!
//! - Chat/vex integration. Those will call the primitives here in a
//!   follow-up change; this commit ships the data plane only.
//! - Plan-file storage location, git integration, or backup policy.
//!   Callers choose where `.d3vx/plans/<id>.md` lives.
//! - UI rendering of plan state. The TUI will consume `Plan` via its
//!   public fields; no presentation logic lives here.

pub mod advance;
pub mod decision;
pub mod errors;
pub mod markdown;
pub mod phase;
pub mod plan;
pub mod preamble;

#[cfg(test)]
mod tests;

pub use advance::{advance_one_step, StepOutcome};
pub use decision::{parse_decision, PlanDecision};
pub use errors::PlannerError;
pub use markdown::{parse_plan, serialize_plan};
pub use phase::PhaseSelection;
pub use plan::{Plan, PlanSection, PlanStatus, SectionState, Subtask};
pub use preamble::planner_preamble;
