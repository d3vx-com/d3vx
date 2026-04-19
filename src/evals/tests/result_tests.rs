//! Tests for eval result aggregation and report rendering.

use crate::evals::grader::GradeOutcome;
use crate::evals::result::{EvalReport, EvalResult, ReportFormat};

fn passing(id: &str, name: &str) -> EvalResult {
    EvalResult::success(
        id,
        name,
        vec![GradeOutcome::passed("ok")],
        100,
    )
}

fn failing(id: &str, name: &str) -> EvalResult {
    EvalResult::success(
        id,
        name,
        vec![
            GradeOutcome::passed("ok"),
            GradeOutcome::failed("bad"),
        ],
        200,
    )
}

#[test]
fn success_result_is_passed_iff_every_grader_passed() {
    let ok = EvalResult::success("t", "T", vec![GradeOutcome::passed("a")], 1);
    assert!(ok.passed);

    let mixed = EvalResult::success(
        "t",
        "T",
        vec![GradeOutcome::passed("a"), GradeOutcome::failed("b")],
        1,
    );
    assert!(!mixed.passed);
}

#[test]
fn harness_failure_result_is_always_failed_and_carries_error() {
    let r = EvalResult::harness_failure("t", "T", "provision crashed", 42);
    assert!(!r.passed);
    assert!(r.grader_outcomes.is_empty());
    assert_eq!(r.harness_error.as_deref(), Some("provision crashed"));
    assert_eq!(r.duration_ms, 42);
}

#[test]
fn result_builders_set_fields_without_mutation_surprise() {
    let r = passing("t", "T")
        .with_cost(1.23)
        .with_iterations(7)
        .with_tool_calls(15);
    assert_eq!(r.cost_usd, Some(1.23));
    assert_eq!(r.iterations, Some(7));
    assert_eq!(r.tool_calls, Some(15));
}

#[test]
fn pass_rate_on_empty_report_is_zero_not_nan() {
    let report = EvalReport::new(Vec::new());
    assert_eq!(report.pass_rate(), 0.0);
    assert_eq!(report.passed_count(), 0);
}

#[test]
fn pass_rate_reflects_fraction_of_passing_results() {
    let report = EvalReport::new(vec![
        passing("a", "A"),
        passing("b", "B"),
        failing("c", "C"),
        failing("d", "D"),
    ]);
    assert_eq!(report.passed_count(), 2);
    assert!((report.pass_rate() - 0.5).abs() < 1e-9);
}

#[test]
fn total_cost_sums_only_known_costs() {
    let report = EvalReport::new(vec![
        passing("a", "A").with_cost(1.0),
        passing("b", "B"), // unknown cost
        passing("c", "C").with_cost(2.5),
    ]);
    assert!((report.total_cost_usd() - 3.5).abs() < 1e-9);
}

#[test]
fn summary_mentions_counts_and_cost() {
    let report = EvalReport::new(vec![
        passing("a", "A").with_cost(1.0),
        failing("b", "B"),
    ]);
    let s = report.summary();
    assert!(s.contains("1/2"));
    assert!(s.contains("$1.00"));
}

#[test]
fn markdown_render_contains_header_and_rows() {
    let report = EvalReport::new(vec![
        passing("a", "Task A").with_cost(0.123).with_iterations(3),
        failing("b", "Task B"),
    ]);
    let md = report.render(ReportFormat::Markdown);
    assert!(md.contains("# Eval report"));
    assert!(md.contains("Task A"));
    assert!(md.contains("Task B"));
    assert!(md.contains("✅ pass"));
    assert!(md.contains("❌ fail"));
    assert!(md.contains("$0.123"));
}

#[test]
fn markdown_render_shows_em_dash_for_missing_cost_and_iter() {
    let report = EvalReport::new(vec![passing("a", "A")]);
    let md = report.render(ReportFormat::Markdown);
    // Two em-dashes: cost and iterations (tools also unset)
    let dash_count = md.matches('—').count();
    assert!(dash_count >= 2, "expected em-dashes for missing metrics: {md}");
}

#[test]
fn tsv_render_has_header_row_and_one_line_per_result() {
    let report = EvalReport::new(vec![passing("a", "A"), failing("b", "B")]);
    let tsv = report.render(ReportFormat::Tsv);
    let lines: Vec<&str> = tsv.lines().collect();
    assert_eq!(lines.len(), 3); // header + 2 rows
    assert!(lines[0].contains("task_id"));
    assert!(lines[0].contains("cost_usd"));
    assert!(lines[1].contains("\ttrue\t"));
    assert!(lines[2].contains("\tfalse\t"));
}

#[test]
fn json_render_is_valid_and_round_trips() {
    let original = EvalReport::new(vec![
        passing("a", "A").with_cost(0.5).with_iterations(2).with_tool_calls(4),
        failing("b", "B"),
    ]);
    let json = original.render(ReportFormat::Json);
    let parsed: EvalReport = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.results.len(), 2);
    assert_eq!(parsed.results[0].task_id, "a");
    assert_eq!(parsed.results[0].cost_usd, Some(0.5));
    assert_eq!(parsed.results[1].task_id, "b");
    assert!(!parsed.results[1].passed);
}

#[test]
fn render_empty_report_still_renders_for_all_formats() {
    let report = EvalReport::new(Vec::new());
    for fmt in [ReportFormat::Markdown, ReportFormat::Tsv, ReportFormat::Json] {
        let out = report.render(fmt);
        assert!(!out.is_empty(), "{fmt:?} render should not be empty");
    }
}
