//! Validation Summary
//!
//! Aggregates validation results into a structured summary with confidence levels.
//! Integrates with the review gate system for merge readiness.
//!
//! ## Usage
//!
//! ```rust
//! use pipeline::commander::{ValidationRunner, ValidationKind};
//! use pipeline::validation_summary::{ValidationSummary, Confidence};
//!
//! let runner = ValidationRunner::new(project_root);
//! let results = runner.run_all().await;
//! let summary = ValidationSummary::from_results(results);
//! assert!(summary.confidence().can_merge());
//! ```

use serde::{Deserialize, Serialize};

use super::commander::{ValidationKind, ValidationResult};

/// Confidence level for validation summary
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// No validation run yet
    None,
    /// Validation run in progress
    InProgress,
    /// Low confidence - failures present
    Low,
    /// Medium confidence - some issues but acceptable
    Medium,
    /// High confidence - all checks passed
    High,
}

impl Confidence {
    /// Whether this confidence level allows merge
    pub fn can_merge(self) -> bool {
        matches!(self, Confidence::High)
    }

    /// Whether this confidence level blocks merge
    pub fn blocks_merge(self) -> bool {
        matches!(self, Confidence::Low)
    }

    /// Convert to a numeric score (0-100)
    pub fn score(self) -> u8 {
        match self {
            Confidence::None => 0,
            Confidence::InProgress => 25,
            Confidence::Low => 40,
            Confidence::Medium => 70,
            Confidence::High => 100,
        }
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Confidence::None
    }
}

/// Summary of a validation run with aggregated results
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ValidationSummary {
    /// Task ID this validation belongs to
    pub task_id: Option<String>,
    /// Overall confidence level
    pub confidence: Confidence,
    /// Total validations run
    pub total: usize,
    /// Number that passed
    pub passed: usize,
    /// Number that failed
    pub failed: usize,
    /// Number with warnings
    pub warnings: usize,
    /// Total duration in ms
    pub duration_ms: u64,
    /// Individual validation results
    pub results: Vec<ValidationResult>,
    /// When validation was started
    pub started_at: Option<String>,
    /// When validation was completed
    pub completed_at: Option<String>,
}

impl ValidationSummary {
    /// Create a new empty validation summary
    pub fn new(task_id: Option<String>) -> Self {
        Self {
            task_id,
            confidence: Confidence::None,
            total: 0,
            passed: 0,
            failed: 0,
            warnings: 0,
            duration_ms: 0,
            results: Vec::new(),
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            completed_at: None,
        }
    }

    /// Create a summary from validation results
    pub fn from_results(results: Vec<ValidationResult>) -> Self {
        let total = results.len();
        let passed = results.iter().filter(|r| r.success).count();
        let failed = total - passed;
        let warnings = results.iter().filter(|r| !r.warnings.is_empty()).count();
        let duration_ms: u64 = results.iter().map(|r| r.duration_ms).sum();
        let confidence = Self::compute_confidence(&results);

        Self {
            task_id: None,
            confidence,
            total,
            passed,
            failed,
            warnings,
            duration_ms,
            results,
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }

    /// Compute confidence from results
    fn compute_confidence(results: &[ValidationResult]) -> Confidence {
        if results.is_empty() {
            return Confidence::None;
        }

        let total = results.len();
        let passed = results.iter().filter(|r| r.success).count();
        let pass_rate = passed as f32 / total as f32;

        // Count critical failures (type check, test)
        let critical_failures: usize = results
            .iter()
            .filter(|r| {
                !r.success && matches!(r.kind, ValidationKind::TypeCheck | ValidationKind::Test)
            })
            .count();

        // High confidence: all pass
        if pass_rate >= 1.0 {
            return Confidence::High;
        }

        // Medium confidence: 80%+ pass, no critical failures
        if pass_rate >= 0.8 && critical_failures == 0 {
            return Confidence::Medium;
        }

        // Low confidence: anything else with failures
        let failed = total - passed;
        if failed > 0 || critical_failures > 0 {
            return Confidence::Low;
        }

        Confidence::Medium
    }

    /// Add a result to the summary
    pub fn add_result(&mut self, result: ValidationResult) {
        self.total += 1;
        if result.success {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        if !result.warnings.is_empty() {
            self.warnings += 1;
        }
        self.duration_ms += result.duration_ms;
        self.confidence = Self::compute_confidence(&self.results);
        self.results.push(result);
    }

    /// Mark validation as in progress
    pub fn start(&mut self) {
        self.started_at = Some(chrono::Utc::now().to_rfc3339());
        self.confidence = Confidence::InProgress;
    }

    /// Mark validation as complete
    pub fn complete(&mut self) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.confidence = Self::compute_confidence(&self.results);
    }

    /// Get pass rate as percentage
    pub fn pass_rate(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.passed as f32 / self.total as f32) * 100.0
        }
    }

    /// Get all errors across all validations
    pub fn all_errors(&self) -> Vec<(ValidationKind, String)> {
        self.results
            .iter()
            .flat_map(|r| r.errors.iter().map(|e| (r.kind.clone(), e.clone())))
            .collect()
    }

    /// Get all warnings across all validations
    pub fn all_warnings(&self) -> Vec<(ValidationKind, String)> {
        self.results
            .iter()
            .flat_map(|r| r.warnings.iter().map(|w| (r.kind.clone(), w.clone())))
            .collect()
    }

    /// Check if validation is ready for merge
    pub fn is_merge_ready(&self) -> bool {
        self.confidence.can_merge()
    }

    /// Check if validation blocks merge
    pub fn blocks_merge(&self) -> bool {
        self.confidence.blocks_merge() || self.failed > 0
    }

    /// Get a brief status line
    pub fn status_line(&self) -> String {
        match self.confidence {
            Confidence::None => "Not validated".to_string(),
            Confidence::InProgress => {
                format!("Validating... ({}/{} done)", self.passed, self.total)
            }
            Confidence::Low => format!(
                "{}/{} passed, {} failed - LOW CONFIDENCE",
                self.passed, self.total, self.failed
            ),
            Confidence::Medium => format!(
                "{}/{} passed, {} warnings - MEDIUM CONFIDENCE",
                self.passed, self.total, self.warnings
            ),
            Confidence::High => format!("{}/{} passed - HIGH CONFIDENCE", self.passed, self.total),
        }
    }

    /// Get structured summary for UI/inspection
    pub fn summary_for_ui(&self) -> ValidationUiSummary {
        ValidationUiSummary {
            status: self.status_line(),
            confidence: self.confidence,
            confidence_score: self.confidence.score(),
            pass_rate: self.pass_rate(),
            passed: self.passed,
            failed: self.failed,
            warnings: self.warnings,
            total: self.total,
            duration_ms: self.duration_ms,
            type_check_passed: self.get_kind_passed(ValidationKind::TypeCheck),
            test_passed: self.get_kind_passed(ValidationKind::Test),
            lint_passed: self.get_kind_passed(ValidationKind::Lint),
            can_merge: self.is_merge_ready(),
            blocks_merge: self.blocks_merge(),
        }
    }

    fn get_kind_passed(&self, kind: ValidationKind) -> Option<bool> {
        self.results
            .iter()
            .find(|r| r.kind == kind)
            .map(|r| r.success)
    }
}

/// Simplified summary for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationUiSummary {
    pub status: String,
    pub confidence: Confidence,
    pub confidence_score: u8,
    pub pass_rate: f32,
    pub passed: usize,
    pub failed: usize,
    pub warnings: usize,
    pub total: usize,
    pub duration_ms: u64,
    pub type_check_passed: Option<bool>,
    pub test_passed: Option<bool>,
    pub lint_passed: Option<bool>,
    pub can_merge: bool,
    pub blocks_merge: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::commander::ValidationKind;

    fn make_result(kind: ValidationKind, success: bool, warnings: usize) -> ValidationResult {
        ValidationResult {
            kind,
            success,
            output: "test output".to_string(),
            duration_ms: 100,
            errors: if success {
                vec![]
            } else {
                vec!["error".to_string()]
            },
            warnings: vec!["warning".to_string(); warnings],
        }
    }

    #[test]
    fn test_confidence_high() {
        let results = vec![
            make_result(ValidationKind::TypeCheck, true, 0),
            make_result(ValidationKind::Test, true, 0),
            make_result(ValidationKind::Lint, true, 0),
        ];
        let summary = ValidationSummary::from_results(results);
        assert_eq!(summary.confidence, Confidence::High);
        assert!(summary.is_merge_ready());
    }

    #[test]
    fn test_confidence_medium() {
        // Medium: 80%+ pass, no critical failures (TypeCheck/Test), but not 100%
        let results = vec![
            make_result(ValidationKind::TypeCheck, true, 0),
            make_result(ValidationKind::Test, true, 0),
            make_result(ValidationKind::Lint, true, 0),
            make_result(ValidationKind::Lint, true, 0),
            make_result(ValidationKind::Lint, false, 0), // 1 non-critical failure
        ];
        let summary = ValidationSummary::from_results(results);
        // 4/5 = 80% pass rate, 0 critical failures → Medium
        assert_eq!(summary.confidence, Confidence::Medium);
    }

    #[test]
    fn test_confidence_low() {
        let results = vec![
            make_result(ValidationKind::TypeCheck, false, 0), // critical failure
        ];
        let summary = ValidationSummary::from_results(results);
        assert_eq!(summary.confidence, Confidence::Low);
        assert!(summary.blocks_merge());
    }

    #[test]
    fn test_empty_summary() {
        let summary = ValidationSummary::new(Some("task-1".to_string()));
        assert_eq!(summary.confidence, Confidence::None);
        assert_eq!(summary.total, 0);
    }

    #[test]
    fn test_pass_rate() {
        let results = vec![
            make_result(ValidationKind::TypeCheck, true, 0),
            make_result(ValidationKind::Test, false, 0),
            make_result(ValidationKind::Lint, true, 0),
        ];
        let summary = ValidationSummary::from_results(results);
        assert!((summary.pass_rate() - 66.67).abs() < 0.1);
    }

    #[test]
    fn test_merge_ready() {
        let results = vec![
            make_result(ValidationKind::TypeCheck, true, 0),
            make_result(ValidationKind::Test, true, 0),
        ];
        let summary = ValidationSummary::from_results(results);
        assert!(summary.is_merge_ready());
        assert!(!summary.blocks_merge());
    }

    #[test]
    fn test_blocks_merge() {
        let results = vec![make_result(ValidationKind::TypeCheck, false, 0)];
        let summary = ValidationSummary::from_results(results);
        assert!(!summary.is_merge_ready());
        assert!(summary.blocks_merge());
    }

    #[test]
    fn test_ui_summary() {
        let results = vec![
            make_result(ValidationKind::TypeCheck, true, 0),
            make_result(ValidationKind::Test, true, 0),
        ];
        let summary = ValidationSummary::from_results(results);
        let ui = summary.summary_for_ui();

        assert_eq!(ui.passed, 2);
        assert_eq!(ui.failed, 0);
        assert!(ui.can_merge);
        assert!(!ui.blocks_merge);
        assert_eq!(ui.confidence_score, 100);
    }
}
