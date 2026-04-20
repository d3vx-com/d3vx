//! Single unified error type for the planner.
//!
//! One enum keeps callers from having to pattern-match N separate
//! error types as they chain decision parsing, plan loading,
//! markdown serialisation, and phase execution. Every variant
//! carries enough context that an operator reading a log can
//! reconstruct what went wrong.

use std::path::PathBuf;

use thiserror::Error;

use crate::pipeline::handlers::PhaseError;

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse plan file {path}: {reason}")]
    ParseFailed { path: PathBuf, reason: String },

    #[error("failed to parse plan YAML frontmatter: {0}")]
    FrontmatterParse(String),

    #[error("plan file {path} missing required field `{field}`")]
    MissingField { path: PathBuf, field: &'static str },

    #[error("no decision block found in AI output; expected a ```d3vx-decision``` block")]
    DecisionMissing,

    #[error("decision block could not be parsed: {0}")]
    DecisionInvalid(String),

    #[error(
        "decision references unknown phase `{phase}`; valid phases: {valid:?}"
    )]
    UnknownPhase {
        phase: String,
        valid: Vec<&'static str>,
    },

    #[error("plan has no more work to do — every phase is already complete")]
    AlreadyComplete,

    #[error("phase `{phase}` does not appear in this plan's selection")]
    PhaseNotSelected { phase: String },

    #[error("phase handler for `{phase}` failed: {source}")]
    PhaseHandler {
        phase: String,
        #[source]
        source: PhaseError,
    },
}
