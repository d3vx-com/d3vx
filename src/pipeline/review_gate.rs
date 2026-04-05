//! Review Gate
//!
//! Merge blocking logic based on review findings and requirements.
//!
//! ## Usage
//!
//! ```rust
//! let gate = ReviewGate::new(requirements);
//! let result = gate.evaluate(&review);
//! if result.is_blocked() {
//!     // Don't merge
//! }
//! ```

use super::review_summary::{
    FindingCategory, ReviewFinding, ReviewRequirements, ReviewStatus, ReviewSummary,
};
use serde::{Deserialize, Serialize};

/// Result of evaluating the review gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Whether merge is blocked
    pub blocked: bool,
    /// Blocking reasons
    pub reasons: Vec<BlockingReason>,
    /// Non-blocking warnings
    pub warnings: Vec<String>,
    /// Overall readiness
    pub ready: bool,
}

/// Reason for merge block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockingReason {
    pub code: String,
    pub message: String,
    pub category: FindingCategory,
    pub finding_ids: Vec<String>,
}

/// ReviewGate evaluates reviews against requirements
#[derive(Debug, Clone)]
pub struct ReviewGate {
    requirements: ReviewRequirements,
}

impl ReviewGate {
    pub fn new(requirements: ReviewRequirements) -> Self {
        Self { requirements }
    }

    pub fn with_defaults() -> Self {
        Self::new(ReviewRequirements::default())
    }

    /// Evaluate a review against the gate requirements
    pub fn evaluate(&self, review: &ReviewSummary) -> GateResult {
        let mut reasons = Vec::new();
        let mut warnings = Vec::new();

        if review.status == ReviewStatus::Pending {
            reasons.push(BlockingReason {
                code: "REVIEW_NOT_STARTED".to_string(),
                message: "Review has not been started".to_string(),
                category: FindingCategory::Coverage,
                finding_ids: Vec::new(),
            });
        }

        if review.status == ReviewStatus::Rejected {
            reasons.push(BlockingReason {
                code: "REVIEW_REJECTED".to_string(),
                message: "Review was rejected with blocking issues".to_string(),
                category: FindingCategory::Coverage,
                finding_ids: review.blocking_findings.clone(),
            });
        }

        if self.requirements.require_automated
            && review.reviewer == super::review_summary::ReviewerType::Automated
            && review.findings.is_empty()
        {
            warnings.push("No automated review findings".to_string());
        }

        for finding in &review.findings {
            if !finding.resolved
                && self
                    .requirements
                    .block_on_categories
                    .contains(&finding.category)
            {
                reasons.push(BlockingReason {
                    code: format!("CATEGORY_{:?}", finding.category).to_uppercase(),
                    message: format!("Blocking finding in category: {:?}", finding.category),
                    category: finding.category,
                    finding_ids: vec![finding.id.clone()],
                });
            }
        }

        let blocked = !reasons.is_empty();
        let ready = !blocked
            && (review.status == ReviewStatus::Approved || review.status == ReviewStatus::Skipped);

        GateResult {
            blocked,
            reasons,
            warnings,
            ready,
        }
    }

    /// Check if a single finding should block merge
    pub fn should_block(&self, finding: &ReviewFinding) -> bool {
        if finding.resolved {
            return false;
        }

        if finding.severity.blocks_merge() {
            return true;
        }

        self.requirements
            .block_on_categories
            .contains(&finding.category)
    }

    /// Filter findings to only blocking ones
    pub fn filter_blocking(&self, findings: &[ReviewFinding]) -> Vec<ReviewFinding> {
        findings
            .iter()
            .filter(|f| self.should_block(f))
            .cloned()
            .collect()
    }
}

impl Default for ReviewGate {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::super::review_summary::{FindingLocation, ReviewSeverity};
    use super::*;

    fn make_review(status: ReviewStatus) -> ReviewSummary {
        ReviewSummary {
            task_id: "task-1".to_string(),
            status,
            findings: Vec::new(),
            changed_files: Vec::new(),
            reviewer: super::super::review_summary::ReviewerType::Automated,
            requested_at: None,
            completed_at: None,
            summary_text: None,
            merge_blocked: false,
            blocking_findings: Vec::new(),
        }
    }

    fn make_finding(severity: ReviewSeverity, category: FindingCategory) -> ReviewFinding {
        ReviewFinding {
            id: "f1".to_string(),
            category,
            severity,
            title: "Test".to_string(),
            description: "Test".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        }
    }

    #[test]
    fn test_gate_passes_for_approved_review() {
        let gate = ReviewGate::with_defaults();
        let review = make_review(ReviewStatus::Approved);
        let result = gate.evaluate(&review);

        assert!(!result.blocked);
        assert!(result.ready);
    }

    #[test]
    fn test_gate_blocks_pending_review() {
        let gate = ReviewGate::with_defaults();
        let review = make_review(ReviewStatus::Pending);
        let result = gate.evaluate(&review);

        assert!(result.blocked);
        assert!(!result.ready);
        assert!(!result.reasons.is_empty());
    }

    #[test]
    fn test_gate_blocks_critical_finding() {
        let mut gate = ReviewGate::with_defaults();
        let mut review = make_review(ReviewStatus::InProgress);

        review.add_finding(make_finding(
            ReviewSeverity::Critical,
            FindingCategory::Security,
        ));
        gate.evaluate(&review);
        review.finalize();

        let result = gate.evaluate(&review);
        assert!(result.blocked);
    }

    #[test]
    fn test_gate_allows_resolved_finding() {
        let gate = ReviewGate::with_defaults();
        let mut review = make_review(ReviewStatus::InProgress);

        let mut finding = make_finding(ReviewSeverity::High, FindingCategory::Security);
        finding.resolved = true;
        review.add_finding(finding);
        review.finalize();

        let result = gate.evaluate(&review);
        assert!(!result.blocked);
    }

    #[test]
    fn test_should_block_by_category() {
        let gate = ReviewGate::with_defaults();

        let security_finding = make_finding(ReviewSeverity::Medium, FindingCategory::Security);
        assert!(gate.should_block(&security_finding));

        let style_finding = make_finding(ReviewSeverity::Medium, FindingCategory::Maintainability);
        assert!(!gate.should_block(&style_finding));
    }
}
