//! `Plan` — data model for a markdown plan file.
//!
//! A plan is the canonical unit of work in the planner. It carries the
//! user's original request, the phase selection the AI decided on, and
//! an ordered list of phase sections with per-section state and body
//! content. Serialisation to markdown lives in
//! [`super::markdown`]; this file owns only the data shape and
//! invariant-preserving mutators.
//!
//! # State model
//!
//! Each selected phase becomes a [`PlanSection`] on creation, in the
//! state [`SectionState::NotStarted`]. Phases advance one at a time:
//! the primitive [`super::advance::advance_one_step`] picks the first
//! `NotStarted` section, invokes the corresponding phase handler, and
//! on success flips the section to [`SectionState::Completed`] with
//! the handler's output as its `body`.
//!
//! A plan is [`PlanStatus::Completed`] once every section is completed.
//! Any section failure flips the plan to [`PlanStatus::Failed`] but
//! preserves prior completions — re-running the plan resumes from the
//! first not-yet-completed section, matching the Superpowers model of
//! "markdown checkboxes as the resume protocol."

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::pipeline::phases::Phase;

use super::errors::PlannerError;
use super::phase::PhaseSelection;

/// Plan-level lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Created but execution has not started.
    Draft,
    /// At least one section completed; still more work to do.
    InProgress,
    /// Every section completed successfully.
    Completed,
    /// A section failed; the plan halted before completing.
    Failed,
    /// User or system cancelled the plan before completion.
    Cancelled,
}

/// Per-section lifecycle state. Mirrors the markdown checkbox glyphs
/// in the serialised form (`[ ]`, `[~]`, `[x]`, `[!]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionState {
    NotStarted,
    InProgress,
    Completed,
    Failed,
}

impl SectionState {
    pub fn is_done(self) -> bool {
        matches!(self, SectionState::Completed)
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, SectionState::Completed | SectionState::Failed)
    }
}

/// A single phase section inside a plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanSection {
    pub phase: Phase,
    pub state: SectionState,
    /// Output written by the phase handler when it ran.
    pub body: String,
    /// Sub-checkboxes inside this section. The Plan phase uses these
    /// as the list of implementation subtasks the Implement phase will
    /// tick off; other phases leave this empty.
    #[serde(default)]
    pub subtasks: Vec<Subtask>,
}

/// A checkbox item nested inside a plan section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subtask {
    pub done: bool,
    pub text: String,
}

impl Subtask {
    pub fn pending(text: impl Into<String>) -> Self {
        Self {
            done: false,
            text: text.into(),
        }
    }

    pub fn completed(text: impl Into<String>) -> Self {
        Self {
            done: true,
            text: text.into(),
        }
    }
}

/// The plan file itself.
///
/// Cloning is cheap (all fields are owned, no `Arc`s), which lets the
/// advance primitive take a `&mut Plan` cheaply without borrowing
/// through the filesystem layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// Stable slug, e.g. `2026-04-20-thumbnail-cache`. The filename on
    /// disk is `{id}.md`; callers choose the parent directory.
    pub id: String,
    /// Human-readable short title.
    pub title: String,
    /// Lifecycle status across all sections.
    pub status: PlanStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// The user's prompt verbatim. Preserved so the plan file is
    /// self-contained — an operator reading `.d3vx/plans/foo.md` can
    /// see why this plan exists without chasing the originating chat.
    pub original_request: String,
    /// Phases the AI decided on. Stored separately from `sections`
    /// so that an empty-phase "direct answer" plan is still
    /// representable (selection empty, sections empty, body in the
    /// `original_request` / response log).
    pub phase_selection: PhaseSelection,
    /// Per-phase sections, in the same order as `phase_selection`.
    pub sections: Vec<PlanSection>,
}

impl Plan {
    /// Build a fresh plan from a selection. Every selected phase gets
    /// a [`SectionState::NotStarted`] section with an empty body.
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        original_request: impl Into<String>,
        selection: PhaseSelection,
    ) -> Self {
        let now = Utc::now();
        let sections = selection
            .phases()
            .iter()
            .map(|p| PlanSection {
                phase: *p,
                state: SectionState::NotStarted,
                body: String::new(),
                subtasks: Vec::new(),
            })
            .collect();

        Self {
            id: id.into(),
            title: title.into(),
            status: if selection.is_empty() {
                PlanStatus::Completed
            } else {
                PlanStatus::Draft
            },
            created_at: now,
            updated_at: now,
            original_request: original_request.into(),
            phase_selection: selection,
            sections,
        }
    }

    /// Position of the first `NotStarted` section, if any. The advance
    /// primitive uses this to decide what to run next.
    pub fn first_pending_section_index(&self) -> Option<usize> {
        self.sections
            .iter()
            .position(|s| s.state == SectionState::NotStarted)
    }

    /// True if every selected section reached a terminal state AND
    /// none failed. A plan with `InProgress` sections is not complete.
    pub fn is_complete(&self) -> bool {
        !self.sections.is_empty()
            && self.sections.iter().all(|s| s.state == SectionState::Completed)
    }

    /// True if at least one section is `Failed`.
    pub fn any_failed(&self) -> bool {
        self.sections.iter().any(|s| s.state == SectionState::Failed)
    }

    /// Mark the section at `index` with a new state and body. Updates
    /// the plan's own status accordingly and bumps `updated_at`.
    /// Returns an error if the index is out of range.
    pub fn record_outcome(
        &mut self,
        index: usize,
        state: SectionState,
        body: impl Into<String>,
    ) -> Result<(), PlannerError> {
        let len = self.sections.len();
        let section = self.sections.get_mut(index).ok_or_else(|| {
            PlannerError::DecisionInvalid(format!(
                "section index {index} out of range (len={len})"
            ))
        })?;
        section.state = state;
        section.body = body.into();
        self.updated_at = Utc::now();

        self.status = if self.any_failed() {
            PlanStatus::Failed
        } else if self.is_complete() {
            PlanStatus::Completed
        } else if self
            .sections
            .iter()
            .any(|s| s.state != SectionState::NotStarted)
        {
            PlanStatus::InProgress
        } else {
            PlanStatus::Draft
        };
        Ok(())
    }

    /// Attach subtasks to the Plan-phase section. Used by the Plan
    /// phase handler to record the list the Implement phase will tick
    /// off. No-op if the plan doesn't include a Plan phase.
    pub fn set_plan_subtasks(&mut self, subtasks: Vec<Subtask>) {
        if let Some(section) =
            self.sections.iter_mut().find(|s| s.phase == Phase::Plan)
        {
            section.subtasks = subtasks;
            self.updated_at = Utc::now();
        }
    }
}
