//! Commander Validation Tests

use super::runner::parse_output_issues;
use super::types::*;

#[test]
fn test_validation_kind_display() {
    assert_eq!(format!("{}", ValidationKind::TypeCheck), "type_check");
    assert_eq!(format!("{}", ValidationKind::Test), "test");
    assert_eq!(format!("{}", ValidationKind::Lint), "lint");
    assert_eq!(
        format!("{}", ValidationKind::Custom("fmt".into())),
        "custom(fmt)"
    );
}

#[test]
fn test_default_validation_command() {
    let cmd = ValidationCommand::default();
    assert_eq!(cmd.kind, ValidationKind::TypeCheck);
    assert_eq!(cmd.command, "cargo check");
    assert_eq!(cmd.timeout_secs, 120);
}

#[test]
fn test_convenience_constructors() {
    let tc = ValidationCommand::type_check();
    assert_eq!(tc.kind, ValidationKind::TypeCheck);
    assert_eq!(tc.command, "cargo check");

    let t = ValidationCommand::test();
    assert_eq!(t.kind, ValidationKind::Test);
    assert_eq!(t.command, "cargo test");

    let l = ValidationCommand::lint();
    assert_eq!(l.kind, ValidationKind::Lint);
    assert_eq!(l.command, "cargo clippy");

    let c = ValidationCommand::custom("fmt", "cargo fmt --check", 60);
    assert_eq!(c.kind, ValidationKind::Custom("fmt".into()));
    assert_eq!(c.timeout_secs, 60);
}

#[test]
fn test_default_commands_returns_three() {
    let cmds = super::runner::ValidationRunner::default_commands();
    assert_eq!(cmds.len(), 3);
    assert_eq!(cmds[0].kind, ValidationKind::TypeCheck);
    assert_eq!(cmds[1].kind, ValidationKind::Test);
    assert_eq!(cmds[2].kind, ValidationKind::Lint);
}

#[test]
fn test_all_passed_empty() {
    assert!(super::runner::ValidationRunner::all_passed(&[]));
}

#[test]
fn test_all_passed_true() {
    let results = vec![make_pass_result(), make_pass_result()];
    assert!(super::runner::ValidationRunner::all_passed(&results));
}

#[test]
fn test_all_passed_false() {
    let results = vec![make_pass_result(), make_fail_result()];
    assert!(!super::runner::ValidationRunner::all_passed(&results));
}

#[test]
fn test_summarize_empty() {
    let summary = super::runner::ValidationRunner::summarize(&[]);
    assert!(summary.contains("No validation results"));
}

#[test]
fn test_summarize_results() {
    let results = vec![make_pass_result(), make_fail_result()];
    let summary = super::runner::ValidationRunner::summarize(&results);
    assert!(summary.contains("1/2 passed"));
    assert!(summary.contains("PASS"));
    assert!(summary.contains("FAIL"));
    assert!(summary.contains("1 validation(s) failed"));
}

#[test]
fn test_parse_output_issues_errors_and_warnings() {
    let output = "error[E0432]: unresolved import\nwarning: unused variable\nnote: something\n";
    let (errors, warnings) = parse_output_issues(output, false);
    assert_eq!(errors.len(), 1);
    assert_eq!(warnings.len(), 1);
    assert!(errors[0].contains("error[E0432]"));
    assert!(warnings[0].contains("warning:"));
}

#[test]
fn test_parse_output_issues_no_explicit_error_falls_back() {
    let output = "something went wrong\nbad state";
    let (errors, _warnings) = parse_output_issues(output, false);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("something went wrong"));
}

#[test]
fn test_parse_output_issues_success_no_errors() {
    let output = "running 5 tests\nall passed\n";
    let (errors, warnings) = parse_output_issues(output, true);
    assert!(errors.is_empty());
    assert!(warnings.is_empty());
}

// -- Helpers for tests ----------------------------------------------------

#[allow(dead_code)]
fn make_pass_result() -> ValidationResult {
    ValidationResult {
        kind: ValidationKind::Test,
        success: true,
        output: "all tests passed".to_string(),
        duration_ms: 1500,
        errors: vec![],
        warnings: vec![],
    }
}

#[allow(dead_code)]
fn make_fail_result() -> ValidationResult {
    ValidationResult {
        kind: ValidationKind::Lint,
        success: false,
        output: "error: clippy lint failed".to_string(),
        duration_ms: 800,
        errors: vec!["error: clippy lint failed".to_string()],
        warnings: vec![],
    }
}
