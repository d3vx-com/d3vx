//! Automatic Post-Edit Quality Gate
//!
//! Reviews file changes after edit/write tool calls, checking for
//! common issues like syntax errors, line length, trailing whitespace,
//! and TODO/FIXME markers.

pub mod checks;
#[cfg(test)]
mod tests;
pub mod types;

use std::cmp::Ordering;
use std::fs;

pub use types::{AutoReviewConfig, QualityGateResult, ReviewFinding, Severity};

// ---------------------------------------------------------------------------
// Trigger check
// ---------------------------------------------------------------------------

/// Returns true if the given tool name should trigger an automatic review.
pub fn should_trigger_review(tool_name: &str) -> bool {
    matches!(tool_name, "edit" | "multi_edit" | "write")
}

// ---------------------------------------------------------------------------
// Core review
// ---------------------------------------------------------------------------

/// Review file changes and return a sorted list of findings.
///
/// Findings are sorted by severity (errors first), then by file path, then
/// by line number.
pub fn review_file_changes(file_paths: &[String]) -> Vec<ReviewFinding> {
    let config = AutoReviewConfig::default();
    review_file_changes_with_config(file_paths, &config)
}

/// Review file changes with an explicit config. Public for testing.
pub fn review_file_changes_with_config(
    file_paths: &[String],
    config: &AutoReviewConfig,
) -> Vec<ReviewFinding> {
    if !config.enabled || file_paths.is_empty() {
        return Vec::new();
    }

    let limited: Vec<&String> = file_paths
        .iter()
        .take(config.max_files_per_review)
        .collect();

    let mut findings = Vec::new();

    for file_path in &limited {
        if !checks::is_supported_source_file(file_path.as_str()) {
            continue;
        }

        let content = match fs::read_to_string(file_path.as_str()) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(file = %file_path, error = %e, "Cannot read file for review");
                continue;
            }
        };

        if config.check_syntax {
            checks::check_syntax(file_path, &content, &mut findings);
        }

        checks::check_style(file_path, &content, &mut findings);
        checks::check_markers(file_path, &content, &mut findings);
    }

    sort_findings(&mut findings);
    findings
}

// ---------------------------------------------------------------------------
// Sorting
// ---------------------------------------------------------------------------

fn sort_findings(findings: &mut [ReviewFinding]) {
    findings.sort_by(|a, b| match a.severity.cmp(&b.severity) {
        Ordering::Equal => match a.file.cmp(&b.file) {
            Ordering::Equal => a.line.cmp(&b.line),
            other => other,
        },
        other => other,
    });
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Format findings as a concise markdown summary.
///
/// Findings are sorted by severity (errors first) before formatting.
pub fn format_findings(findings: &[ReviewFinding]) -> String {
    if findings.is_empty() {
        return "Quality Gate: passed (no issues)".to_string();
    }

    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let infos = findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .count();

    let files: Vec<&str> = {
        let mut set: Vec<&str> = findings.iter().map(|f| f.file.as_str()).collect();
        set.sort();
        set.dedup();
        set
    };

    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(format!("{} error(s)", errors));
    }
    if warnings > 0 {
        parts.push(format!("{} warning(s)", warnings));
    }
    if infos > 0 {
        parts.push(format!("{} info", infos));
    }

    let header = format!(
        "Quality Gate: {} in {} file(s)",
        parts.join(", "),
        files.len()
    );

    // Sort findings by severity before rendering.
    let mut sorted: Vec<&ReviewFinding> = findings.iter().collect();
    sorted.sort_by(|a, b| a.severity.cmp(&b.severity));

    let mut lines = vec![header];
    for finding in sorted {
        let loc = match finding.line {
            Some(l) => format!(":{}", l),
            None => String::new(),
        };
        let sev = match finding.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN",
            Severity::Info => "INFO",
        };
        lines.push(format!(
            "  [{}] {}{} - {} ({})",
            sev, finding.file, loc, finding.message, finding.source
        ));
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Post-edit quality gate
// ---------------------------------------------------------------------------

/// Run the post-edit quality gate for a tool call.
///
/// Returns `None` if the tool does not trigger a review or if no findings
/// were produced.
pub fn check_post_edit_quality(
    tool_name: &str,
    file_paths: &[String],
    config: &AutoReviewConfig,
) -> Option<QualityGateResult> {
    if !should_trigger_review(tool_name) {
        return None;
    }

    let findings = review_file_changes_with_config(file_paths, config);
    if findings.is_empty() {
        return None;
    }

    let has_errors = findings.iter().any(|f| f.severity == Severity::Error);
    let summary = format_findings(&findings);

    Some(QualityGateResult {
        has_errors,
        findings,
        summary,
    })
}
