//! Markdown ↔ `Plan` round-trip.
//!
//! The plan file is the protocol, so the format is strict and
//! documented here. A plan file is a single markdown document with:
//!
//! 1. A YAML frontmatter block (required).
//! 2. An H1 title (the first non-frontmatter line that starts with
//!    `# `).
//! 3. An `## Original request` section (optional — missing implies
//!    empty string).
//! 4. Zero or more phase sections, each shaped `## [<state>] <Phase>`,
//!    where `<state>` is one of `' '`, `'~'`, `'x'`, `'!'` and
//!    `<Phase>` is a capitalised phase name (e.g. `Research`).
//!
//! Anything between a phase heading and the next phase heading (or
//! end-of-file) is that section's body. Lines starting with `- [ ] `
//! or `- [x] ` inside the Plan section are extracted into `subtasks`;
//! all other body lines are preserved verbatim.
//!
//! # Example
//!
//! ```markdown
//! ---
//! id: 2026-04-20-thumbnail-cache
//! status: in_progress
//! created_at: 2026-04-20T10:00:00Z
//! updated_at: 2026-04-20T10:12:34Z
//! phase_selection: [research, plan, implement]
//! ---
//!
//! # Thumbnail cache for image gallery
//!
//! ## Original request
//! Build a thumbnail cache for the image gallery.
//!
//! ## [x] Research
//! Existing gallery uses on-demand resizing. No cache layer yet.
//!
//! ## [ ] Plan
//! - [ ] Migration 104: cache table
//! - [ ] ImageLoader.fetch_with_cache
//!
//! ## [ ] Implement
//! ```

use chrono::{DateTime, Utc};

use crate::pipeline::phases::Phase;

use super::errors::PlannerError;
use super::phase::{phase_from_name, phase_name, PhaseSelection};
use super::plan::{Plan, PlanSection, PlanStatus, SectionState, Subtask};

/// Serialise a plan to the strict markdown format above.
pub fn serialize_plan(plan: &Plan) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(1024);

    // Frontmatter.
    out.push_str("---\n");
    writeln!(out, "id: {}", plan.id).unwrap();
    writeln!(out, "status: {}", plan_status_name(plan.status)).unwrap();
    writeln!(
        out,
        "created_at: {}",
        plan.created_at.to_rfc3339()
    )
    .unwrap();
    writeln!(
        out,
        "updated_at: {}",
        plan.updated_at.to_rfc3339()
    )
    .unwrap();
    write!(out, "phase_selection: [").unwrap();
    for (i, p) in plan.phase_selection.phases().iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(phase_name(*p));
    }
    out.push_str("]\n");
    out.push_str("---\n\n");

    // Title + original request.
    writeln!(out, "# {}\n", plan.title).unwrap();
    out.push_str("## Original request\n");
    writeln!(out, "{}\n", plan.original_request.trim_end()).unwrap();

    // Phase sections.
    for section in &plan.sections {
        writeln!(
            out,
            "## [{}] {}",
            state_glyph(section.state),
            phase_heading(section.phase)
        )
        .unwrap();
        if !section.body.trim().is_empty() {
            writeln!(out, "{}", section.body.trim_end()).unwrap();
        }
        for sub in &section.subtasks {
            writeln!(
                out,
                "- [{}] {}",
                if sub.done { 'x' } else { ' ' },
                sub.text
            )
            .unwrap();
        }
        out.push('\n');
    }

    out
}

/// Parse a plan from its markdown form. Returns a specific error on
/// any format violation — this parser is strict on purpose.
pub fn parse_plan(source: &str) -> Result<Plan, PlannerError> {
    let (frontmatter, rest) = split_frontmatter(source)?;
    let fm = parse_frontmatter(frontmatter)?;

    let mut title = String::new();
    let mut original_request = String::new();
    let mut sections: Vec<PlanSection> = Vec::new();

    // Walk line by line, collecting title, original request, and
    // per-section bodies.
    let mut lines = rest.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") && title.is_empty() {
            title = trimmed[2..].trim().to_string();
            continue;
        }
        if trimmed == "## Original request" {
            original_request = collect_section_body(&mut lines);
            continue;
        }
        if let Some((state, phase)) = parse_phase_heading(trimmed) {
            let mut body_lines: Vec<String> = Vec::new();
            let mut subtasks: Vec<Subtask> = Vec::new();
            while let Some(peek) = lines.peek() {
                if peek.trim().starts_with("## ") {
                    break;
                }
                let owned = lines.next().unwrap().to_string();
                if let Some(st) = parse_subtask(&owned) {
                    subtasks.push(st);
                } else {
                    body_lines.push(owned);
                }
            }
            sections.push(PlanSection {
                phase,
                state,
                body: body_lines.join("\n").trim().to_string(),
                subtasks,
            });
            continue;
        }
    }

    // Build PhaseSelection strictly from the frontmatter list, then
    // cross-check that every section's phase is in the selection (so
    // a hand-edited file can't drift the section list out of sync).
    let selection = PhaseSelection::try_from_phases(fm.phase_selection)?;
    for s in &sections {
        if !selection.contains(s.phase) {
            return Err(PlannerError::DecisionInvalid(format!(
                "section phase `{}` not listed in frontmatter phase_selection",
                phase_name(s.phase)
            )));
        }
    }

    Ok(Plan {
        id: fm.id,
        title,
        status: fm.status,
        created_at: fm.created_at,
        updated_at: fm.updated_at,
        original_request,
        phase_selection: selection,
        sections,
    })
}

struct Frontmatter {
    id: String,
    status: PlanStatus,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    phase_selection: Vec<Phase>,
}

fn split_frontmatter(source: &str) -> Result<(&str, &str), PlannerError> {
    let source = source.trim_start_matches('\u{feff}'); // strip BOM if any
    let source = source.trim_start_matches(['\r', '\n']);
    let source = source
        .strip_prefix("---\n")
        .or_else(|| source.strip_prefix("---\r\n"))
        .ok_or_else(|| PlannerError::FrontmatterParse(
            "plan must start with `---` frontmatter".to_string(),
        ))?;
    let end = source.find("\n---").ok_or_else(|| {
        PlannerError::FrontmatterParse(
            "frontmatter not terminated by `---` on its own line".to_string(),
        )
    })?;
    let fm = &source[..end];
    let rest = &source[end + "\n---".len()..];
    let rest = rest.trim_start_matches(['\r', '\n']);
    Ok((fm, rest))
}

fn parse_frontmatter(fm: &str) -> Result<Frontmatter, PlannerError> {
    let mut id: Option<String> = None;
    let mut status: Option<PlanStatus> = None;
    let mut created_at: Option<DateTime<Utc>> = None;
    let mut updated_at: Option<DateTime<Utc>> = None;
    let mut phase_selection: Option<Vec<Phase>> = None;

    for line in fm.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line.split_once(':').ok_or_else(|| {
            PlannerError::FrontmatterParse(format!(
                "frontmatter line lacks `:` separator: {line}"
            ))
        })?;
        let value = value.trim();
        match key.trim() {
            "id" => id = Some(value.trim_matches('"').to_string()),
            "status" => status = Some(parse_plan_status(value)?),
            "created_at" => {
                created_at = Some(parse_rfc3339(value)?);
            }
            "updated_at" => {
                updated_at = Some(parse_rfc3339(value)?);
            }
            "phase_selection" => {
                phase_selection = Some(parse_phase_list(value)?);
            }
            _ => {} // ignore unknown fields for forward-compat
        }
    }
    Ok(Frontmatter {
        id: id.ok_or(PlannerError::FrontmatterParse("missing `id`".into()))?,
        status: status.ok_or(PlannerError::FrontmatterParse(
            "missing `status`".into(),
        ))?,
        created_at: created_at.ok_or(PlannerError::FrontmatterParse(
            "missing `created_at`".into(),
        ))?,
        updated_at: updated_at.ok_or(PlannerError::FrontmatterParse(
            "missing `updated_at`".into(),
        ))?,
        phase_selection: phase_selection.ok_or(PlannerError::FrontmatterParse(
            "missing `phase_selection`".into(),
        ))?,
    })
}

fn parse_plan_status(value: &str) -> Result<PlanStatus, PlannerError> {
    match value {
        "draft" => Ok(PlanStatus::Draft),
        "in_progress" => Ok(PlanStatus::InProgress),
        "completed" => Ok(PlanStatus::Completed),
        "failed" => Ok(PlanStatus::Failed),
        "cancelled" => Ok(PlanStatus::Cancelled),
        other => Err(PlannerError::FrontmatterParse(format!(
            "unknown status `{other}`"
        ))),
    }
}

fn plan_status_name(s: PlanStatus) -> &'static str {
    match s {
        PlanStatus::Draft => "draft",
        PlanStatus::InProgress => "in_progress",
        PlanStatus::Completed => "completed",
        PlanStatus::Failed => "failed",
        PlanStatus::Cancelled => "cancelled",
    }
}

fn parse_phase_list(value: &str) -> Result<Vec<Phase>, PlannerError> {
    let inner = value
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| PlannerError::FrontmatterParse(
            "phase_selection must be a bracketed list".into(),
        ))?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    inner.split(',').map(phase_from_name).collect()
}

fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>, PlannerError> {
    let value = value.trim_matches('"');
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| PlannerError::FrontmatterParse(format!("invalid datetime `{value}`: {e}")))
}

fn parse_phase_heading(line: &str) -> Option<(SectionState, Phase)> {
    let rest = line.strip_prefix("## [")?;
    let (state_char, rest) = rest.split_once("] ")?;
    let state = match state_char {
        " " => SectionState::NotStarted,
        "~" => SectionState::InProgress,
        "x" | "X" => SectionState::Completed,
        "!" => SectionState::Failed,
        _ => return None,
    };
    let phase = phase_from_name(rest.trim()).ok()?;
    Some((state, phase))
}

fn parse_subtask(line: &str) -> Option<Subtask> {
    let t = line.trim();
    if let Some(rest) = t.strip_prefix("- [ ] ") {
        return Some(Subtask::pending(rest));
    }
    if let Some(rest) = t.strip_prefix("- [x] ") {
        return Some(Subtask::completed(rest));
    }
    if let Some(rest) = t.strip_prefix("- [X] ") {
        return Some(Subtask::completed(rest));
    }
    None
}

fn collect_section_body<'a>(
    lines: &mut std::iter::Peekable<std::str::Lines<'a>>,
) -> String {
    let mut buf: Vec<&str> = Vec::new();
    while let Some(peek) = lines.peek() {
        if peek.trim().starts_with("## ") || peek.trim().starts_with("# ") {
            break;
        }
        buf.push(lines.next().unwrap());
    }
    buf.join("\n").trim().to_string()
}

fn state_glyph(s: SectionState) -> char {
    match s {
        SectionState::NotStarted => ' ',
        SectionState::InProgress => '~',
        SectionState::Completed => 'x',
        SectionState::Failed => '!',
    }
}

fn phase_heading(p: Phase) -> &'static str {
    match p {
        Phase::Research => "Research",
        Phase::Ideation => "Ideation",
        Phase::Plan => "Plan",
        Phase::Draft => "Draft",
        Phase::Review => "Review",
        Phase::Implement => "Implement",
        Phase::Docs => "Docs",
    }
}
