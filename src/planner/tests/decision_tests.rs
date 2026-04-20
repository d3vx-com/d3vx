//! Tests for the fenced `d3vx-decision` block parser.

use crate::pipeline::phases::Phase;
use crate::planner::decision::parse_decision;
use crate::planner::PlannerError;

#[test]
fn parses_block_with_phases_reason_and_null_resume() {
    let src = r#"Sure, let me think.

```d3vx-decision
phases: [plan, implement]
reason: non-trivial refactor across modules
resume: null
```

I'll now go ahead and start.
"#;
    let d = parse_decision(src).unwrap();
    assert_eq!(d.phases.phases(), &[Phase::Plan, Phase::Implement]);
    assert_eq!(d.reason, "non-trivial refactor across modules");
    assert_eq!(d.resume, None);
}

#[test]
fn parses_empty_phase_list_as_direct_answer() {
    let src = "```d3vx-decision\nphases: []\nreason: trivial question\nresume: null\n```\n";
    let d = parse_decision(src).unwrap();
    assert!(d.phases.is_empty());
    assert_eq!(d.reason, "trivial question");
    assert_eq!(d.resume, None);
}

#[test]
fn parses_resume_id_when_present() {
    let src = concat!(
        "```d3vx-decision\n",
        "phases: [implement]\n",
        "reason: resume existing plan\n",
        "resume: 2026-04-20-thumb-cache\n",
        "```\n",
    );
    let d = parse_decision(src).unwrap();
    assert_eq!(d.resume.as_deref(), Some("2026-04-20-thumb-cache"));
}

#[test]
fn accepts_quoted_reason_and_quoted_resume() {
    let src = concat!(
        "```d3vx-decision\n",
        "phases: [plan]\n",
        "reason: \"the design is the hard part\"\n",
        "resume: \"2026-04-20-x\"\n",
        "```\n",
    );
    let d = parse_decision(src).unwrap();
    assert_eq!(d.reason, "the design is the hard part");
    assert_eq!(d.resume.as_deref(), Some("2026-04-20-x"));
}

#[test]
fn missing_block_returns_decision_missing() {
    let err = parse_decision("no decision here").unwrap_err();
    assert!(matches!(err, PlannerError::DecisionMissing));
}

#[test]
fn malformed_phase_list_returns_decision_invalid() {
    let src = "```d3vx-decision\nphases: plan, implement\nreason: x\nresume: null\n```\n";
    let err = parse_decision(src).unwrap_err();
    assert!(matches!(err, PlannerError::DecisionInvalid(_)));
}

#[test]
fn unknown_phase_name_surfaces_unknown_phase_error() {
    let src = "```d3vx-decision\nphases: [magic]\nreason: x\nresume: null\n```\n";
    let err = parse_decision(src).unwrap_err();
    assert!(matches!(err, PlannerError::UnknownPhase { .. }));
}

#[test]
fn first_decision_block_wins_when_multiple_present() {
    let src = r#"```d3vx-decision
phases: []
reason: first
resume: null
```

Some more talk.

```d3vx-decision
phases: [plan]
reason: second
resume: null
```
"#;
    let d = parse_decision(src).unwrap();
    assert!(d.phases.is_empty());
    assert_eq!(d.reason, "first");
}

#[test]
fn missing_resume_defaults_to_none() {
    let src = "```d3vx-decision\nphases: [plan]\nreason: x\n```\n";
    let d = parse_decision(src).unwrap();
    assert_eq!(d.resume, None);
}

#[test]
fn tilde_is_treated_as_null_for_resume() {
    let src = "```d3vx-decision\nphases: []\nreason: x\nresume: ~\n```\n";
    let d = parse_decision(src).unwrap();
    assert_eq!(d.resume, None);
}
