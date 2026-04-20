//! Markdown round-trip tests for the planner's plan format.

use chrono::{TimeZone, Utc};

use crate::pipeline::phases::Phase;
use crate::planner::markdown::{parse_plan, serialize_plan};
use crate::planner::phase::PhaseSelection;
use crate::planner::plan::{Plan, PlanStatus, SectionState, Subtask};
use crate::planner::PlannerError;

fn fixed_plan() -> Plan {
    let ts = Utc.with_ymd_and_hms(2026, 4, 20, 10, 0, 0).unwrap();
    let sel = PhaseSelection::from_phases(vec![Phase::Plan, Phase::Implement]);
    let mut plan = Plan::new("2026-04-20-thumb-cache", "Thumbnail cache", "build it", sel);
    plan.created_at = ts;
    plan.updated_at = ts;
    plan
}

#[test]
fn fresh_plan_round_trips_through_markdown() {
    let original = fixed_plan();
    let md = serialize_plan(&original);
    let parsed = parse_plan(&md).expect("parse fresh plan");
    // updated_at may shift because record_outcome uses Utc::now(). Here
    // we didn't call record_outcome, so equality holds.
    assert_eq!(parsed, original);
}

#[test]
fn serialized_form_contains_frontmatter_and_phase_headings() {
    let md = serialize_plan(&fixed_plan());
    assert!(md.starts_with("---\n"));
    assert!(md.contains("id: 2026-04-20-thumb-cache"));
    assert!(md.contains("status: draft"));
    assert!(md.contains("phase_selection: [plan, implement]"));
    assert!(md.contains("# Thumbnail cache"));
    assert!(md.contains("## [ ] Plan"));
    assert!(md.contains("## [ ] Implement"));
}

#[test]
fn subtasks_and_completed_sections_round_trip() {
    let mut plan = fixed_plan();
    plan.set_plan_subtasks(vec![
        Subtask::pending("cache table migration"),
        Subtask::completed("loader.fetch_with_cache"),
    ]);
    plan.record_outcome(0, SectionState::Completed, "plan output body")
        .unwrap();
    let md = serialize_plan(&plan);
    assert!(md.contains("## [x] Plan"));
    assert!(md.contains("- [ ] cache table migration"));
    assert!(md.contains("- [x] loader.fetch_with_cache"));

    let parsed = parse_plan(&md).unwrap();
    let plan_section = parsed
        .sections
        .iter()
        .find(|s| s.phase == Phase::Plan)
        .unwrap();
    assert_eq!(plan_section.state, SectionState::Completed);
    assert_eq!(plan_section.body, "plan output body");
    assert_eq!(plan_section.subtasks.len(), 2);
    assert!(!plan_section.subtasks[0].done);
    assert!(plan_section.subtasks[1].done);
}

#[test]
fn parse_rejects_missing_frontmatter() {
    let src = "# No frontmatter\n";
    let err = parse_plan(src).unwrap_err();
    assert!(matches!(err, PlannerError::FrontmatterParse(_)));
}

#[test]
fn parse_rejects_missing_required_fields() {
    let src = concat!(
        "---\n",
        "id: x\n",
        // missing status/created_at/updated_at/phase_selection
        "---\n",
        "# t\n",
    );
    let err = parse_plan(src).unwrap_err();
    assert!(matches!(err, PlannerError::FrontmatterParse(_)));
}

#[test]
fn parse_rejects_section_phase_not_in_selection() {
    // Frontmatter lists only [plan], but body adds an Implement section.
    let src = concat!(
        "---\n",
        "id: mismatched\n",
        "status: draft\n",
        "created_at: 2026-04-20T10:00:00+00:00\n",
        "updated_at: 2026-04-20T10:00:00+00:00\n",
        "phase_selection: [plan]\n",
        "---\n\n",
        "# Mismatched plan\n\n",
        "## Original request\n",
        "do the thing\n\n",
        "## [ ] Plan\n",
        "body\n\n",
        "## [ ] Implement\n",
        "unexpected\n",
    );
    let err = parse_plan(src).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("not listed in frontmatter phase_selection"),
        "unexpected msg: {msg}"
    );
}

#[test]
fn parse_handles_capital_x_in_completed_checkboxes() {
    let src = concat!(
        "---\n",
        "id: caps\n",
        "status: completed\n",
        "created_at: 2026-04-20T10:00:00+00:00\n",
        "updated_at: 2026-04-20T10:00:00+00:00\n",
        "phase_selection: [plan]\n",
        "---\n\n",
        "# caps\n\n",
        "## Original request\n",
        "q\n\n",
        "## [X] Plan\n",
        "body\n",
        "- [X] a\n",
        "- [ ] b\n",
    );
    let plan = parse_plan(src).unwrap();
    assert_eq!(plan.sections[0].state, SectionState::Completed);
    assert!(plan.sections[0].subtasks[0].done);
    assert!(!plan.sections[0].subtasks[1].done);
}

#[test]
fn empty_phase_selection_is_representable() {
    let ts = Utc.with_ymd_and_hms(2026, 4, 20, 10, 0, 0).unwrap();
    let mut plan = Plan::new("direct", "trivial", "hello?", PhaseSelection::empty());
    plan.created_at = ts;
    plan.updated_at = ts;
    let md = serialize_plan(&plan);
    assert!(md.contains("phase_selection: []"));
    let parsed = parse_plan(&md).unwrap();
    assert!(parsed.phase_selection.is_empty());
    assert_eq!(parsed.status, PlanStatus::Completed);
}
