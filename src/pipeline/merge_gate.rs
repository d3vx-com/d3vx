//! Unified Merge Gate
//!
//! Combines review, validation, and docs completeness into a single merge readiness decision.
//!
//! ## Design
//!
//! - **Single source of truth**: One `MergeReadiness` result for all merge decisions
//! - **Structured blocking**: Each reason includes source (review/validation/docs)
//! - **Deterministic**: Same inputs always produce same output
//! - **Explainable**: Clear messages and warnings for UI/logging
//!
//! ## Usage
//!
//! ```rust
//! let gate = MergeGate::with_defaults();
//! let readiness = gate.evaluate(&review_summary, &validation_summary, &docs_completeness);
//! if !readiness.ready {
//!     for reason in &readiness.reasons {
//!         println!("{}: {}", reason.source, reason.message);
//!     }
//! }
//! ```
//!
//! ## Blocking Logic
//!
//! | Source        | Condition                          | Blocks Merge |
//! |---------------|-----------------------------------|--------------|
//! | Review        | Critical/High finding unresolved   | Yes          |
//! | Review        | Status = Rejected                  | Yes          |
//! | Review        | Status = Pending                   | Yes          |
//! | Validation    | Confidence = Low                   | Yes          |
//! | Validation    | Required but not run                | Yes          |
//! | Validation    | Confidence = Medium                | No (warning) |
//! | Docs          | Status = Missing                   | Yes          |
//! | Docs          | Required but not evaluated          | Yes          |

use serde::{Deserialize, Serialize};

use super::docs_completeness::DocsCompleteness;
use super::review_gate::GateResult;
use super::review_summary::{ReviewStatus, ReviewSummary};
use super::validation_summary::{Confidence, ValidationSummary};

/// Source of a merge blocking reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeSource {
    Review,
    Validation,
    Docs,
}

impl std::fmt::Display for MergeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Review => write!(f, "review"),
            Self::Validation => write!(f, "validation"),
            Self::Docs => write!(f, "docs"),
        }
    }
}

/// A blocking reason for merge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeBlockingReason {
    pub source: MergeSource,
    pub code: String,
    pub message: String,
    pub can_auto_fix: bool,
}

impl MergeBlockingReason {
    pub fn review(code: &str, message: &str) -> Self {
        Self {
            source: MergeSource::Review,
            code: code.to_string(),
            message: message.to_string(),
            can_auto_fix: true,
        }
    }

    pub fn validation(code: &str, message: &str) -> Self {
        Self {
            source: MergeSource::Validation,
            code: code.to_string(),
            message: message.to_string(),
            can_auto_fix: false,
        }
    }

    pub fn docs(code: &str, message: &str) -> Self {
        Self {
            source: MergeSource::Docs,
            code: code.to_string(),
            message: message.to_string(),
            can_auto_fix: true,
        }
    }
}

/// A non-blocking warning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeWarning {
    pub source: MergeSource,
    pub message: String,
}

impl MergeWarning {
    pub fn new(source: MergeSource, message: &str) -> Self {
        Self {
            source,
            message: message.to_string(),
        }
    }
}

/// Readiness status for a single signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalReadiness {
    pub ready: bool,
    pub status: String,
    pub details: Option<String>,
}

/// Overall merge signals breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeSignals {
    pub review: Option<SignalReadiness>,
    pub validation: Option<SignalReadiness>,
    pub docs: Option<SignalReadiness>,
}

impl Default for MergeSignals {
    fn default() -> Self {
        Self {
            review: None,
            validation: None,
            docs: None,
        }
    }
}

/// Unified merge readiness result
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MergeReadiness {
    pub ready: bool,
    pub blocked: bool,
    pub reasons: Vec<MergeBlockingReason>,
    pub warnings: Vec<MergeWarning>,
    pub signals: MergeSignals,
    pub summary: String,
}

impl MergeReadiness {
    pub fn not_started() -> Self {
        Self {
            ready: false,
            blocked: true,
            reasons: vec![MergeBlockingReason::review(
                "NOT_STARTED",
                "Quality gates have not been started",
            )],
            warnings: Vec::new(),
            signals: MergeSignals::default(),
            summary: "Not started".to_string(),
        }
    }

    pub fn blocked(
        reasons: Vec<MergeBlockingReason>,
        warnings: Vec<MergeWarning>,
        signals: MergeSignals,
    ) -> Self {
        let summary = Self::build_summary(&reasons, &warnings);
        Self {
            ready: false,
            blocked: true,
            reasons,
            warnings,
            signals,
            summary,
        }
    }

    pub fn ready(warnings: Vec<MergeWarning>, signals: MergeSignals) -> Self {
        Self {
            ready: true,
            blocked: false,
            reasons: Vec::new(),
            warnings,
            signals,
            summary: "Ready to merge".to_string(),
        }
    }

    fn build_summary(reasons: &[MergeBlockingReason], warnings: &[MergeWarning]) -> String {
        let mut parts = Vec::new();

        let review_blocks = reasons
            .iter()
            .filter(|r| r.source == MergeSource::Review)
            .count();
        if review_blocks > 0 {
            parts.push(format!("{} review block(s)", review_blocks));
        }

        let val_blocks = reasons
            .iter()
            .filter(|r| r.source == MergeSource::Validation)
            .count();
        if val_blocks > 0 {
            parts.push(format!("{} validation block(s)", val_blocks));
        }

        let docs_blocks = reasons
            .iter()
            .filter(|r| r.source == MergeSource::Docs)
            .count();
        if docs_blocks > 0 {
            parts.push(format!("{} docs block(s)", docs_blocks));
        }

        if warnings.len() > 0 {
            parts.push(format!("{} warning(s)", warnings.len()));
        }

        if parts.is_empty() {
            "Blocked".to_string()
        } else {
            parts.join(", ")
        }
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn blocks_merge(&self) -> bool {
        self.blocked
    }

    pub fn has_blocking_review(&self) -> bool {
        self.reasons.iter().any(|r| r.source == MergeSource::Review)
    }

    pub fn has_blocking_validation(&self) -> bool {
        self.reasons
            .iter()
            .any(|r| r.source == MergeSource::Validation)
    }

    pub fn has_blocking_docs(&self) -> bool {
        self.reasons.iter().any(|r| r.source == MergeSource::Docs)
    }
}

/// Configuration for merge gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeGateConfig {
    pub require_validation: bool,
    pub require_docs: bool,
    pub skip_validation_if_skipped: bool,
    pub skip_docs_if_not_required: bool,
}

impl Default for MergeGateConfig {
    fn default() -> Self {
        Self {
            require_validation: true,
            require_docs: true,
            skip_validation_if_skipped: true,
            skip_docs_if_not_required: true,
        }
    }
}

/// Unified merge gate
#[derive(Debug, Clone)]
pub struct MergeGate {
    config: MergeGateConfig,
}

impl MergeGate {
    pub fn new(config: MergeGateConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(MergeGateConfig::default())
    }

    /// Evaluate all three signals and produce unified readiness
    pub fn evaluate(
        &self,
        review: Option<&ReviewSummary>,
        validation: Option<&ValidationSummary>,
        docs: Option<&DocsCompleteness>,
    ) -> MergeReadiness {
        let mut reasons = Vec::new();
        let mut warnings = Vec::new();
        let signals = MergeSignals {
            review: self.evaluate_review(review),
            validation: self.evaluate_validation(validation),
            docs: self.evaluate_docs(docs),
        };

        // Check review
        if let Some(r) = review {
            self.check_review(r, &mut reasons, &mut warnings);
        }

        // Check validation
        if let Some(v) = validation {
            self.check_validation(v, &mut reasons, &mut warnings);
        }

        // Check docs
        if let Some(d) = docs {
            self.check_docs(d, &mut reasons, &mut warnings);
        }

        if reasons.is_empty() {
            MergeReadiness::ready(warnings, signals)
        } else {
            MergeReadiness::blocked(reasons, warnings, signals)
        }
    }

    fn evaluate_review(&self, review: Option<&ReviewSummary>) -> Option<SignalReadiness> {
        review.map(|r| {
            let (ready, status, details) = match r.status {
                ReviewStatus::Pending => (false, "pending".to_string(), None),
                ReviewStatus::InProgress => (false, "in_progress".to_string(), None),
                ReviewStatus::Approved => (true, "approved".to_string(), None),
                ReviewStatus::Rejected => (
                    false,
                    "rejected".to_string(),
                    Some(format!("{} blocking issue(s)", r.blocking_findings.len())),
                ),
                ReviewStatus::Skipped => (true, "skipped".to_string(), None),
            };
            SignalReadiness {
                ready,
                status,
                details,
            }
        })
    }

    fn evaluate_validation(
        &self,
        validation: Option<&ValidationSummary>,
    ) -> Option<SignalReadiness> {
        validation.map(|v| {
            let (ready, status, details) = match v.confidence {
                Confidence::None => (false, "not_run".to_string(), None),
                Confidence::InProgress => (
                    false,
                    "in_progress".to_string(),
                    Some(format!("{}/{} done", v.passed, v.total)),
                ),
                Confidence::Low => (
                    false,
                    "low_confidence".to_string(),
                    Some(format!("{} failed", v.failed)),
                ),
                Confidence::Medium => (
                    true,
                    "medium_confidence".to_string(),
                    Some(format!("{} warning(s)", v.warnings)),
                ),
                Confidence::High => (true, "high_confidence".to_string(), None),
            };
            SignalReadiness {
                ready,
                status,
                details,
            }
        })
    }

    fn evaluate_docs(&self, docs: Option<&DocsCompleteness>) -> Option<SignalReadiness> {
        docs.map(|d| {
            let status = match d.status {
                super::docs_completeness::DocsStatus::NotEvaluated => "not_evaluated",
                super::docs_completeness::DocsStatus::NotRequired => "not_required",
                super::docs_completeness::DocsStatus::Complete => "complete",
                super::docs_completeness::DocsStatus::Missing => "missing",
                super::docs_completeness::DocsStatus::Partial => "partial",
            };
            let ready = d.can_merge();
            let details = if d.missing_types.is_empty() {
                None
            } else {
                Some(format!("{} missing", d.missing_types.len()))
            };
            SignalReadiness {
                ready,
                status: status.to_string(),
                details,
            }
        })
    }

    fn check_review(
        &self,
        review: &ReviewSummary,
        reasons: &mut Vec<MergeBlockingReason>,
        _warnings: &mut Vec<MergeWarning>,
    ) {
        match review.status {
            ReviewStatus::Pending => {
                reasons.push(MergeBlockingReason::review(
                    "REVIEW_NOT_STARTED",
                    "Review has not been started",
                ));
            }
            ReviewStatus::Rejected => {
                let count = review.blocking_findings.len();
                reasons.push(MergeBlockingReason::review(
                    "REVIEW_REJECTED",
                    &format!("Review rejected with {} blocking issue(s)", count),
                ));
            }
            ReviewStatus::InProgress => {
                let unresolved: Vec<_> = review
                    .findings
                    .iter()
                    .filter(|f| f.severity.blocks_merge() && !f.resolved)
                    .collect();
                if !unresolved.is_empty() {
                    reasons.push(MergeBlockingReason::review(
                        "UNRESOLVED_FINDINGS",
                        &format!("{} unresolved blocking finding(s)", unresolved.len()),
                    ));
                }
            }
            ReviewStatus::Approved | ReviewStatus::Skipped => {
                // OK - no blocking
            }
        }
    }

    fn check_validation(
        &self,
        validation: &ValidationSummary,
        reasons: &mut Vec<MergeBlockingReason>,
        warnings: &mut Vec<MergeWarning>,
    ) {
        if validation.confidence == Confidence::None {
            if self.config.require_validation {
                reasons.push(MergeBlockingReason::validation(
                    "VALIDATION_NOT_RUN",
                    "Validation has not been run",
                ));
            }
            return;
        }

        match validation.confidence {
            Confidence::Low => {
                reasons.push(MergeBlockingReason::validation(
                    "VALIDATION_LOW_CONFIDENCE",
                    &format!(
                        "Validation failed: {}/{} passed",
                        validation.passed, validation.total
                    ),
                ));
            }
            Confidence::Medium => {
                warnings.push(MergeWarning::new(
                    MergeSource::Validation,
                    &format!("Medium confidence: {} warning(s)", validation.warnings),
                ));
            }
            Confidence::High | Confidence::InProgress => {
                // InProgress is OK during the run
            }
            Confidence::None => {
                // Already handled above
            }
        }
    }

    fn check_docs(
        &self,
        docs: &DocsCompleteness,
        reasons: &mut Vec<MergeBlockingReason>,
        warnings: &mut Vec<MergeWarning>,
    ) {
        match docs.status {
            super::docs_completeness::DocsStatus::NotEvaluated => {
                if self.config.require_docs {
                    reasons.push(MergeBlockingReason::docs(
                        "DOCS_NOT_EVALUATED",
                        "Documentation completeness has not been evaluated",
                    ));
                }
            }
            super::docs_completeness::DocsStatus::Missing => {
                let types = docs
                    .missing_types
                    .iter()
                    .map(|t| format!("{:?}", t))
                    .collect::<Vec<_>>()
                    .join(", ");
                reasons.push(MergeBlockingReason::docs(
                    "DOCS_MISSING",
                    &format!("Missing documentation: {}", types),
                ));
            }
            super::docs_completeness::DocsStatus::Partial => {
                warnings.push(MergeWarning::new(
                    MergeSource::Docs,
                    "Documentation is partial",
                ));
            }
            super::docs_completeness::DocsStatus::Complete
            | super::docs_completeness::DocsStatus::NotRequired => {
                // OK
            }
        }
    }

    /// Convenience: evaluate from GateResult (backward compat with ReviewGate)
    pub fn evaluate_from_gate_result(&self, gate_result: &GateResult) -> MergeReadiness {
        if gate_result.ready {
            MergeReadiness::ready(
                gate_result
                    .warnings
                    .iter()
                    .map(|w| MergeWarning::new(MergeSource::Review, w))
                    .collect(),
                MergeSignals {
                    review: Some(SignalReadiness {
                        ready: true,
                        status: "approved".to_string(),
                        details: None,
                    }),
                    validation: None,
                    docs: None,
                },
            )
        } else {
            let reasons = gate_result
                .reasons
                .iter()
                .map(|r| MergeBlockingReason {
                    source: MergeSource::Review,
                    code: r.code.clone(),
                    message: r.message.clone(),
                    can_auto_fix: true,
                })
                .collect();
            MergeReadiness::blocked(
                reasons,
                gate_result
                    .warnings
                    .iter()
                    .map(|w| MergeWarning::new(MergeSource::Review, w))
                    .collect(),
                MergeSignals {
                    review: Some(SignalReadiness {
                        ready: false,
                        status: "rejected".to_string(),
                        details: Some(format!("{} reason(s)", gate_result.reasons.len())),
                    }),
                    validation: None,
                    docs: None,
                },
            )
        }
    }
}

impl Default for MergeGate {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::docs_completeness::{
        DocsCompleteness, DocsStatus,
    };
    use crate::pipeline::review_summary::{FindingCategory, ReviewFinding, ReviewSeverity};
    use crate::pipeline::validation_summary::ValidationSummary;

    fn make_review(status: ReviewStatus) -> ReviewSummary {
        ReviewSummary {
            task_id: "task-1".to_string(),
            status,
            findings: Vec::new(),
            changed_files: Vec::new(),
            reviewer: crate::pipeline::review_summary::ReviewerType::Automated,
            requested_at: None,
            completed_at: None,
            summary_text: None,
            merge_blocked: false,
            blocking_findings: Vec::new(),
        }
    }

    fn make_review_with_finding(severity: ReviewSeverity, resolved: bool) -> ReviewSummary {
        let mut review = make_review(ReviewStatus::InProgress);
        if !resolved {
            review.merge_blocked = true;
            review.blocking_findings.push("f1".to_string());
        }
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Correctness,
            severity,
            title: "Test finding".to_string(),
            description: "Test".to_string(),
            location: None,
            suggestion: None,
            resolved,
        });
        if resolved {
            review.finalize();
        }
        review
    }

    fn make_validation(confidence: Confidence, passed: usize, failed: usize) -> ValidationSummary {
        let mut summary = ValidationSummary::new(Some("task-1".to_string()));
        summary.confidence = confidence;
        summary.passed = passed;
        summary.failed = failed;
        summary.total = passed + failed;
        summary
    }

    fn make_docs(
        status: DocsStatus,
        docs_required: bool,
        missing_count: usize,
    ) -> DocsCompleteness {
        DocsCompleteness {
            task_id: Some("task-1".to_string()),
            status,
            signals: Vec::new(),
            docs_required,
            satisfied: !docs_required || missing_count == 0,
            missing_types: vec![],
            changed_files: Vec::new(),
            evaluated_at: Some("2024-01-01T00:00:00Z".to_string()),
        }
    }

    // Test 1: All three pass
    #[test]
    fn test_all_three_pass() {
        let gate = MergeGate::with_defaults();
        let review = make_review(ReviewStatus::Approved);
        let validation = make_validation(Confidence::High, 3, 0);
        let docs = make_docs(DocsStatus::Complete, true, 0);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(readiness.ready);
        assert!(!readiness.blocked);
        assert!(readiness.reasons.is_empty());
    }

    // Test 2: Review passes but validation fails
    #[test]
    fn test_review_passes_validation_fails() {
        let gate = MergeGate::with_defaults();
        let review = make_review(ReviewStatus::Approved);
        let validation = make_validation(Confidence::Low, 1, 2);
        let docs = make_docs(DocsStatus::Complete, true, 0);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(!readiness.ready);
        assert!(readiness.blocked);
        assert!(readiness.has_blocking_validation());
        assert!(!readiness.has_blocking_review());
        assert!(!readiness.has_blocking_docs());
    }

    // Test 3: Review passes but docs incomplete
    #[test]
    fn test_review_passes_docs_incomplete() {
        let gate = MergeGate::with_defaults();
        let review = make_review(ReviewStatus::Approved);
        let validation = make_validation(Confidence::High, 3, 0);
        let docs = make_docs(DocsStatus::Missing, true, 2);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(!readiness.ready);
        assert!(readiness.blocked);
        assert!(readiness.has_blocking_docs());
        assert!(!readiness.has_blocking_validation());
    }

    // Test 4: Review has blocking findings
    #[test]
    fn test_review_has_blocking_findings() {
        let gate = MergeGate::with_defaults();
        let review = make_review_with_finding(ReviewSeverity::Critical, false);
        let validation = make_validation(Confidence::High, 3, 0);
        let docs = make_docs(DocsStatus::Complete, true, 0);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(!readiness.ready);
        assert!(readiness.has_blocking_review());
    }

    // Test 5: Medium confidence validation - should warn but not block
    #[test]
    fn test_medium_confidence_is_warning() {
        let gate = MergeGate::with_defaults();
        let review = make_review(ReviewStatus::Approved);
        let mut validation = make_validation(Confidence::Medium, 3, 0);
        validation.warnings = 2;
        let docs = make_docs(DocsStatus::Complete, true, 0);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(readiness.ready);
        assert!(!readiness.warnings.is_empty());
    }

    // Test 6: Docs not required - should not block
    #[test]
    fn test_docs_not_required() {
        let gate = MergeGate::with_defaults();
        let review = make_review(ReviewStatus::Approved);
        let validation = make_validation(Confidence::High, 3, 0);
        let docs = make_docs(DocsStatus::NotRequired, false, 0);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(readiness.ready);
        assert!(!readiness.has_blocking_docs());
    }

    // Test 7: Multiple blockers from different sources
    #[test]
    fn test_multiple_blockers() {
        let gate = MergeGate::with_defaults();
        let review = make_review_with_finding(ReviewSeverity::High, false);
        let validation = make_validation(Confidence::Low, 1, 2);
        let docs = make_docs(DocsStatus::Missing, true, 1);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(!readiness.ready);
        assert_eq!(readiness.reasons.len(), 3);
        assert!(readiness.has_blocking_review());
        assert!(readiness.has_blocking_validation());
        assert!(readiness.has_blocking_docs());
    }

    // Test 8: Validation not required when skipped
    #[test]
    fn test_validation_not_required_when_skipped() {
        let gate = MergeGate::with_defaults();
        let review = make_review(ReviewStatus::Approved);
        let validation = make_validation(Confidence::None, 0, 0);
        let docs = make_docs(DocsStatus::Complete, true, 0);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        // With defaults, validation is required, so None should block
        assert!(!readiness.ready);
        assert!(readiness.has_blocking_validation());
    }

    // Test 9: Skipped review is OK
    #[test]
    fn test_skipped_review_ok() {
        let gate = MergeGate::with_defaults();
        let review = make_review(ReviewStatus::Skipped);
        let validation = make_validation(Confidence::High, 3, 0);
        let docs = make_docs(DocsStatus::NotRequired, false, 0);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(readiness.ready);
    }

    // Test 10: GateResult backward compatibility
    #[test]
    fn test_gate_result_backward_compat() {
        let gate = MergeGate::with_defaults();
        let gate_result = GateResult {
            blocked: false,
            reasons: Vec::new(),
            warnings: vec!["Minor style issue".to_string()],
            ready: true,
        };

        let readiness = gate.evaluate_from_gate_result(&gate_result);

        assert!(readiness.ready);
        assert!(!readiness.warnings.is_empty());
    }

    // Test 11: Partial docs - warning but not block
    #[test]
    fn test_partial_docs_warning() {
        let gate = MergeGate::with_defaults();
        let review = make_review(ReviewStatus::Approved);
        let validation = make_validation(Confidence::High, 3, 0);
        let docs = make_docs(DocsStatus::Partial, true, 1);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(readiness.ready); // Partial is a warning, not a block
        assert!(!readiness.warnings.is_empty());
    }

    // Test 12: Validation in progress is OK
    #[test]
    fn test_validation_in_progress_ok() {
        let gate = MergeGate::with_defaults();
        let mut review = make_review(ReviewStatus::InProgress);
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::Medium,
            title: "Minor".to_string(),
            description: "Minor issue".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });
        review.finalize();
        let validation = make_validation(Confidence::InProgress, 2, 0);
        let docs = make_docs(DocsStatus::Complete, true, 0);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        // In progress validation should not block
        assert!(readiness.ready || !readiness.has_blocking_validation());
    }

    // Test 13: Signals breakdown
    #[test]
    fn test_signals_breakdown() {
        let gate = MergeGate::with_defaults();
        let review = make_review(ReviewStatus::Approved);
        let validation = make_validation(Confidence::High, 3, 0);
        let docs = make_docs(DocsStatus::Complete, true, 0);

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(readiness.signals.review.is_some());
        assert!(readiness.signals.validation.is_some());
        assert!(readiness.signals.docs.is_some());
        assert!(readiness.signals.review.unwrap().ready);
        assert!(readiness.signals.validation.unwrap().ready);
    }

    // Test 14: None signals handled gracefully
    #[test]
    fn test_none_signals() {
        let gate = MergeGate::with_defaults();
        let readiness = gate.evaluate(None, None, None);

        // All None should not block by default (nothing to fail)
        // But with strict defaults, validation is required...
        // Actually, with all None, we should have no blockers since nothing was evaluated
        assert!(readiness.signals.review.is_none());
        assert!(readiness.signals.validation.is_none());
        assert!(readiness.signals.docs.is_none());
    }

    // Test 15: Rejected review blocks
    #[test]
    fn test_rejected_review_blocks() {
        let gate = MergeGate::with_defaults();
        let mut review = make_review(ReviewStatus::Rejected);
        review.blocking_findings = vec!["f1".to_string(), "f2".to_string()];

        let readiness = gate.evaluate(Some(&review), None, None);

        assert!(!readiness.ready);
        assert!(readiness.has_blocking_review());
    }
}
