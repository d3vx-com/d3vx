//! Tests for the auto-review hook.

use std::fs;
use std::io::Write as IoWrite;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use super::types::{AutoReviewConfig, ReviewFinding, Severity};
use super::*;

static TEST_ID: AtomicU64 = AtomicU64::new(0);

fn unique_test_dir() -> String {
    let id = TEST_ID.fetch_add(1, AtomicOrdering::Relaxed);
    format!("d3vx_auto_review_test_{}", id)
}

fn write_temp_file(name: &str, content: &str) -> (String, String) {
    let dir_name = unique_test_dir();
    let dir = std::env::temp_dir().join(&dir_name);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    (path.to_string_lossy().to_string(), dir_name)
}

fn cleanup_temp(dir_name: &str) {
    let dir = std::env::temp_dir().join(dir_name);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_should_trigger_on_edit_tools() {
    assert!(should_trigger_review("edit"));
    assert!(should_trigger_review("multi_edit"));
    assert!(should_trigger_review("write"));
}

#[test]
fn test_should_not_trigger_on_read_tools() {
    assert!(!should_trigger_review("read"));
    assert!(!should_trigger_review("glob"));
    assert!(!should_trigger_review("grep"));
    assert!(!should_trigger_review("bash"));
}

#[test]
fn test_syntax_check_detects_unmatched_braces() {
    let (path, dir) = write_temp_file("test.rs", "fn main() {\n  let x = 1;\n");
    let findings = review_file_changes(&[path.clone()]);
    cleanup_temp(&dir);

    let brace_errors: Vec<_> = findings
        .iter()
        .filter(|f| f.message.contains("Unmatched '{'"))
        .collect();
    assert!(
        !brace_errors.is_empty(),
        "Expected unmatched brace finding, got: {:?}",
        findings
    );
}

#[test]
fn test_syntax_check_balanced_braces_passes() {
    let (path, dir) = write_temp_file("balanced.rs", "fn main() {\n  let x = 1;\n}\n");
    let findings = review_file_changes(&[path.clone()]);
    cleanup_temp(&dir);

    let brace_errors: Vec<_> = findings
        .iter()
        .filter(|f| f.message.contains("Unmatched"))
        .collect();
    assert!(
        brace_errors.is_empty(),
        "Expected no unmatched brace findings, got: {:?}",
        brace_errors
    );
}

#[test]
fn test_line_length_finding() {
    let long_line = "x".repeat(350);
    let content = format!("fn main() {{\n  {}\n}}\n", long_line);
    let (path, dir) = write_temp_file("long_line.rs", &content);
    let findings = review_file_changes(&[path.clone()]);
    cleanup_temp(&dir);

    let long_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.message.contains("Line too long"))
        .collect();
    assert!(
        !long_findings.is_empty(),
        "Expected line length finding, got: {:?}",
        findings
    );
    assert_eq!(long_findings[0].severity, Severity::Warning);
}

#[test]
fn test_todo_fixme_detection() {
    let content = "fn main() {\n  // TODO: implement this\n  // FIXME: broken\n}\n";
    let (path, dir) = write_temp_file("markers.rs", content);
    let findings = review_file_changes(&[path.clone()]);
    cleanup_temp(&dir);

    let markers: Vec<_> = findings.iter().filter(|f| f.source == "markers").collect();
    assert_eq!(markers.len(), 2, "Expected 2 marker findings");
}

#[test]
fn test_trailing_whitespace_detection() {
    let content = "fn main() {\n  let x = 1;   \n}\n";
    let (path, dir) = write_temp_file("trail.rs", content);
    let findings = review_file_changes(&[path.clone()]);
    cleanup_temp(&dir);

    let trailing: Vec<_> = findings
        .iter()
        .filter(|f| f.message == "Trailing whitespace")
        .collect();
    assert!(!trailing.is_empty(), "Expected trailing whitespace finding");
}

#[test]
fn test_format_findings_output() {
    let findings = vec![
        ReviewFinding {
            severity: Severity::Error,
            file: "src/main.rs".to_string(),
            line: Some(10),
            message: "Unmatched '{' (1 unclosed)".to_string(),
            source: "syntax".to_string(),
        },
        ReviewFinding {
            severity: Severity::Warning,
            file: "src/main.rs".to_string(),
            line: Some(5),
            message: "Line too long (350 chars)".to_string(),
            source: "style".to_string(),
        },
    ];
    let output = format_findings(&findings);

    assert!(output.starts_with("Quality Gate:"));
    assert!(output.contains("1 error(s)"));
    assert!(output.contains("1 warning(s)"));
    assert!(output.contains("[ERROR]"));
    assert!(output.contains("[WARN]"));
}

#[test]
fn test_empty_changes_no_findings() {
    let findings = review_file_changes(&[]);
    assert!(findings.is_empty());

    let output = format_findings(&[]);
    assert!(output.contains("passed"));
}

#[test]
fn test_disabled_config_returns_empty() {
    let config = AutoReviewConfig {
        enabled: false,
        ..AutoReviewConfig::default()
    };
    let (path, dir) = write_temp_file("disabled.rs", "fn main() {");
    let findings = review_file_changes_with_config(&[path], &config);
    cleanup_temp(&dir);
    assert!(findings.is_empty());
}

#[test]
fn test_non_source_file_skipped() {
    let (path, dir) = write_temp_file("data.json", "{\"key\": \"value\"}");
    let findings = review_file_changes(&[path]);
    cleanup_temp(&dir);
    assert!(findings.is_empty());
}

#[test]
fn test_python_mixed_indentation() {
    let content = "def foo():\n    pass\n\thello\n";
    let (path, dir) = write_temp_file("mixed.py", content);
    let findings = review_file_changes(&[path.clone()]);
    cleanup_temp(&dir);

    let indent: Vec<_> = findings
        .iter()
        .filter(|f| f.message.contains("Mixed indentation"))
        .collect();
    assert!(
        !indent.is_empty(),
        "Expected mixed indentation finding, got: {:?}",
        findings
    );
}

#[test]
fn test_findings_sorted_by_severity() {
    let findings = vec![
        ReviewFinding {
            severity: Severity::Info,
            file: "a.rs".to_string(),
            line: None,
            message: "info message".to_string(),
            source: "test".to_string(),
        },
        ReviewFinding {
            severity: Severity::Error,
            file: "a.rs".to_string(),
            line: None,
            message: "error message".to_string(),
            source: "test".to_string(),
        },
        ReviewFinding {
            severity: Severity::Warning,
            file: "a.rs".to_string(),
            line: None,
            message: "warning message".to_string(),
            source: "test".to_string(),
        },
    ];
    let output = format_findings(&findings);
    let error_pos = output.find("[ERROR]").unwrap();
    let warn_pos = output.find("[WARN]").unwrap();
    let info_pos = output.find("[INFO]").unwrap();
    assert!(error_pos < warn_pos, "ERROR should come before WARN");
    assert!(warn_pos < info_pos, "WARN should come before INFO");
}

#[test]
fn test_nonexistent_file_skipped_gracefully() {
    let findings = review_file_changes(&["/nonexistent/path/test.rs".to_string()]);
    // Should not panic, just skip the file.
    assert!(findings.is_empty());
}
