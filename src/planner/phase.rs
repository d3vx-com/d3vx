//! `PhaseSelection` — an ordered subset of the pipeline's phase catalog.
//!
//! The planner reuses the existing
//! [`pipeline::phases::Phase`](crate::pipeline::phases::Phase) enum
//! instead of inventing a parallel one. This keeps the phase handlers
//! interoperable between the autonomous chat flow and the traditional
//! vex/pipeline flow — both consume the same `Phase` values and the
//! same handler trait.
//!
//! The selection carries only the *intent* — which phases the AI
//! decided apply to this task. Per-phase completion state lives on the
//! [`Plan`](super::Plan)'s sections, not here, so that a selection can
//! be round-tripped through decision → plan → execution without
//! state leaking between layers.

use serde::{Deserialize, Serialize};

use crate::pipeline::phases::Phase;

use super::errors::PlannerError;

/// Ordered set of phases chosen for a task. Empty selection means
/// "direct answer, no phases" — a valid, common outcome for trivial
/// questions. Order matters: phases execute in the order listed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseSelection {
    /// Canonical ordered phase list. Must not contain duplicates.
    phases: Vec<Phase>,
}

impl PhaseSelection {
    /// Empty selection — "no phases, direct answer."
    pub fn empty() -> Self {
        Self { phases: Vec::new() }
    }

    /// Build a selection from an ordered list of phases, removing
    /// duplicates while preserving first-occurrence order. Callers
    /// that want strict validation (no duplicates at all) should
    /// use [`try_from_phases`](Self::try_from_phases).
    pub fn from_phases(phases: Vec<Phase>) -> Self {
        let mut seen = [false; 7];
        let mut unique = Vec::with_capacity(phases.len());
        for p in phases {
            let idx = phase_index(p);
            if !seen[idx] {
                seen[idx] = true;
                unique.push(p);
            }
        }
        Self { phases: unique }
    }

    /// Strict constructor — returns an error if the slice contains
    /// duplicate phases. Useful when parsing a user-authored plan
    /// where duplicates are a bug, not a best-effort input.
    pub fn try_from_phases(phases: Vec<Phase>) -> Result<Self, PlannerError> {
        let mut seen = [false; 7];
        for p in &phases {
            let idx = phase_index(*p);
            if seen[idx] {
                return Err(PlannerError::DecisionInvalid(format!(
                    "duplicate phase `{p}` in selection"
                )));
            }
            seen[idx] = true;
        }
        Ok(Self { phases })
    }

    /// The ordered phases in this selection.
    pub fn phases(&self) -> &[Phase] {
        &self.phases
    }

    /// True if this selection has no phases (direct-answer outcome).
    pub fn is_empty(&self) -> bool {
        self.phases.is_empty()
    }

    /// Number of phases in this selection.
    pub fn len(&self) -> usize {
        self.phases.len()
    }

    /// True if the given phase is part of this selection.
    pub fn contains(&self, phase: Phase) -> bool {
        self.phases.contains(&phase)
    }

    /// The position of `phase` within the selection, if present.
    pub fn position(&self, phase: Phase) -> Option<usize> {
        self.phases.iter().position(|p| *p == phase)
    }
}

/// Index a `Phase` into `[0, 7)` so we can use a fixed-size bitmap
/// for duplicate detection without allocating a `HashSet`.
fn phase_index(p: Phase) -> usize {
    match p {
        Phase::Research => 0,
        Phase::Ideation => 1,
        Phase::Plan => 2,
        Phase::Draft => 3,
        Phase::Review => 4,
        Phase::Implement => 5,
        Phase::Docs => 6,
    }
}

/// Parse a phase name from a lowercase string. Matches exactly the
/// seven values of [`Phase`]. Used by the decision parser to convert
/// `phases: [research, plan]` from YAML into a `PhaseSelection`.
pub fn phase_from_name(name: &str) -> Result<Phase, PlannerError> {
    match name.trim().to_ascii_lowercase().as_str() {
        "research" => Ok(Phase::Research),
        "ideation" => Ok(Phase::Ideation),
        "plan" => Ok(Phase::Plan),
        "draft" => Ok(Phase::Draft),
        "review" => Ok(Phase::Review),
        "implement" => Ok(Phase::Implement),
        "docs" => Ok(Phase::Docs),
        other => Err(PlannerError::UnknownPhase {
            phase: other.to_string(),
            valid: vec![
                "research",
                "ideation",
                "plan",
                "draft",
                "review",
                "implement",
                "docs",
            ],
        }),
    }
}

/// Canonical lowercase name for a phase — symmetric with
/// [`phase_from_name`]. Used by the markdown serializer.
pub fn phase_name(p: Phase) -> &'static str {
    match p {
        Phase::Research => "research",
        Phase::Ideation => "ideation",
        Phase::Plan => "plan",
        Phase::Draft => "draft",
        Phase::Review => "review",
        Phase::Implement => "implement",
        Phase::Docs => "docs",
    }
}
