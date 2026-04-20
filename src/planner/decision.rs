//! Parse the AI's first-turn phase-selection decision.
//!
//! The model is instructed (see [`super::preamble`]) to emit a single
//! fenced block like:
//!
//! ```text
//! ```d3vx-decision
//! phases: [plan, implement]
//! reason: non-trivial refactor touching multiple modules
//! resume: null
//! ```
//! ```
//!
//! This module extracts that block and converts it into a
//! [`PlanDecision`]. We deliberately hand-roll a tiny parser for the
//! three known keys instead of pulling in `serde_yaml` — the grammar
//! is closed (no nested maps, no multi-line scalars), keeping the
//! dependency footprint flat and the error messages tied to exactly
//! the field that failed.

use super::errors::PlannerError;
use super::phase::{phase_from_name, PhaseSelection};

/// The AI's decision about how to handle one user message.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanDecision {
    /// Ordered phase selection the AI chose. Empty means "direct
    /// answer, no pipeline structure."
    pub phases: PhaseSelection,
    /// Short human-readable rationale. Shown to the user in the TUI's
    /// decision banner; stored in the plan file for audit.
    pub reason: String,
    /// Optional id of an existing plan file to resume from instead of
    /// creating a new one. `None` means "this is a fresh task."
    pub resume: Option<String>,
}

/// Extract the first `d3vx-decision` fenced block from `source` and
/// parse it into a [`PlanDecision`]. Returns
/// [`PlannerError::DecisionMissing`] if no such block is present.
pub fn parse_decision(source: &str) -> Result<PlanDecision, PlannerError> {
    let body = extract_decision_block(source).ok_or(PlannerError::DecisionMissing)?;
    parse_decision_body(body)
}

/// Find the first ```d3vx-decision ... ``` fenced block. Returns the
/// body between the opening fence line and the closing fence.
fn extract_decision_block(source: &str) -> Option<&str> {
    // We want a line that starts with ``` followed by d3vx-decision,
    // then a closing ``` on its own line. Extra attributes after the
    // language tag are tolerated.
    let mut lines = source.lines();
    let mut offset = 0usize;
    let mut body_start: Option<usize> = None;
    for line in lines.by_ref() {
        let line_len = line.len() + 1; // include the newline we skipped
        let trimmed = line.trim_start();
        if body_start.is_none() {
            if let Some(rest) = trimmed.strip_prefix("```") {
                if rest.trim() == "d3vx-decision"
                    || rest.trim_start().starts_with("d3vx-decision")
                {
                    offset += line_len;
                    body_start = Some(offset);
                    continue;
                }
            }
        } else if trimmed.starts_with("```") {
            let start = body_start.unwrap();
            let end = offset;
            return Some(&source[start..end]);
        }
        offset += line_len;
    }
    None
}

fn parse_decision_body(body: &str) -> Result<PlanDecision, PlannerError> {
    let mut phases: Option<Vec<_>> = None;
    let mut reason: Option<String> = None;
    let mut resume: Option<Option<String>> = None;

    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once(':').ok_or_else(|| {
            PlannerError::DecisionInvalid(format!(
                "line lacks `:` separator: `{line}`"
            ))
        })?;
        let key = key.trim();
        let value = value.trim();
        match key {
            "phases" => phases = Some(parse_phase_list(value)?),
            "reason" => reason = Some(unquote(value).to_string()),
            "resume" => resume = Some(parse_nullable_string(value)),
            other => {
                return Err(PlannerError::DecisionInvalid(format!(
                    "unknown key `{other}`"
                )));
            }
        }
    }

    let phases = phases.ok_or_else(|| {
        PlannerError::DecisionInvalid("missing `phases` field".into())
    })?;
    let reason = reason.ok_or_else(|| {
        PlannerError::DecisionInvalid("missing `reason` field".into())
    })?;
    let resume = resume.unwrap_or(None);

    Ok(PlanDecision {
        phases: PhaseSelection::try_from_phases(phases)?,
        reason,
        resume,
    })
}

fn parse_phase_list(value: &str) -> Result<Vec<crate::pipeline::phases::Phase>, PlannerError> {
    let inner = value
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| {
            PlannerError::DecisionInvalid(
                "`phases` must be a bracketed list, e.g. `[plan, implement]`".into(),
            )
        })?;
    let inner = inner.trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|name| phase_from_name(name.trim().trim_matches('"')))
        .collect()
}

/// Unwraps optional surrounding `"..."` quotes. Values without quotes
/// are returned unchanged — plan YAML allows both.
fn unquote(value: &str) -> &str {
    let v = value.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// `null` / empty / `~` → `None`; anything else → `Some(unquoted)`.
fn parse_nullable_string(value: &str) -> Option<String> {
    let v = unquote(value);
    if v.is_empty() || v.eq_ignore_ascii_case("null") || v == "~" {
        None
    } else {
        Some(v.to_string())
    }
}
