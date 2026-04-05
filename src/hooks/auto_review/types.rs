//! Types shared across auto_review submodules.

/// Severity level for a review finding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A single quality finding from the review.
#[derive(Debug, Clone)]
pub struct ReviewFinding {
    pub severity: Severity,
    pub file: String,
    pub line: Option<usize>,
    pub message: String,
    pub source: String,
}

/// Configuration for the automatic review hook.
#[derive(Debug, Clone)]
pub struct AutoReviewConfig {
    pub enabled: bool,
    pub check_diagnostics: bool,
    pub check_syntax: bool,
    pub max_files_per_review: usize,
}

impl Default for AutoReviewConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_diagnostics: false,
            check_syntax: true,
            max_files_per_review: 20,
        }
    }
}

/// Result of the post-edit quality gate check.
#[derive(Debug, Clone)]
pub struct QualityGateResult {
    /// True when at least one Error-severity finding was detected.
    pub has_errors: bool,
    /// All findings produced by the gate.
    pub findings: Vec<ReviewFinding>,
    /// Formatted summary suitable for appending to tool output.
    pub summary: String,
}
