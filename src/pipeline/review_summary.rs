//! Review Gate System
//!
//! Structured review data and merge gating for production-ready code.
//!
//! ## Architecture
//!
//! ```text
//! ReviewSummary       - Complete review state for a task
//! ReviewFinding      - Individual finding with severity
//! ReviewGate         - Merge blocking logic
//! ```

use serde::{Deserialize, Serialize};

/// Severity levels for review findings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewSeverity {
    /// Blocking issues - must be resolved before merge
    Critical,
    /// Important issues - should be resolved before merge
    High,
    /// Minor issues - can be addressed post-merge
    Medium,
    /// Informational - no action required
    Low,
}

impl ReviewSeverity {
    /// Check if this severity blocks merge
    pub fn blocks_merge(self) -> bool {
        matches!(self, ReviewSeverity::Critical | ReviewSeverity::High)
    }
}

/// Category of review finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    /// Code correctness issues
    Correctness,
    /// Security vulnerabilities
    Security,
    /// Performance concerns
    Performance,
    /// Code style and maintainability
    Maintainability,
    /// Missing tests or documentation
    Coverage,
    /// API or interface changes
    Breaking,
    /// Risks or concerns requiring human judgment
    Risk,
    /// Documentation completeness issues
    Documentation,
}

/// A single finding from a review
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    /// Unique finding ID
    pub id: String,
    /// Finding category
    pub category: FindingCategory,
    /// Severity level
    pub severity: ReviewSeverity,
    /// Human-readable title
    pub title: String,
    /// Detailed description
    pub description: String,
    /// File or location (optional)
    pub location: Option<FindingLocation>,
    /// Suggested fix (optional)
    pub suggestion: Option<String>,
    /// Whether this finding has been addressed
    pub resolved: bool,
}

/// Location reference for a finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingLocation {
    pub file: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Review status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewStatus {
    /// Review has not been started
    Pending,
    /// Review is in progress
    InProgress,
    /// Review passed - no blocking issues
    Approved,
    /// Review failed - has blocking issues
    Rejected,
    /// Review was skipped
    Skipped,
}

impl Default for ReviewStatus {
    fn default() -> Self {
        ReviewStatus::Pending
    }
}

/// Who performed the review
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewerType {
    /// Automated review (lint, tests, etc.)
    Automated,
    /// Human review
    Human,
    /// Agent review (AI-assisted)
    Agent,
}

impl Default for ReviewerType {
    fn default() -> Self {
        ReviewerType::Automated
    }
}

/// Complete review summary for a task
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ReviewSummary {
    /// Task ID this review belongs to
    pub task_id: String,
    /// Current review status
    pub status: ReviewStatus,
    /// All findings from the review
    pub findings: Vec<ReviewFinding>,
    /// Files that were reviewed
    pub changed_files: Vec<String>,
    /// Reviewer type
    pub reviewer: ReviewerType,
    /// When the review was requested
    pub requested_at: Option<String>,
    /// When the review was completed
    pub completed_at: Option<String>,
    /// Human-readable summary text
    pub summary_text: Option<String>,
    /// Whether merge is blocked
    pub merge_blocked: bool,
    /// Blocking finding IDs
    pub blocking_findings: Vec<String>,
}

impl ReviewSummary {
    /// Create a new pending review
    pub fn new(task_id: String) -> Self {
        Self {
            task_id,
            status: ReviewStatus::Pending,
            findings: Vec::new(),
            changed_files: Vec::new(),
            reviewer: ReviewerType::Automated,
            requested_at: Some(chrono::Utc::now().to_rfc3339()),
            completed_at: None,
            summary_text: None,
            merge_blocked: false,
            blocking_findings: Vec::new(),
        }
    }

    /// Add a finding and recalculate merge status
    pub fn add_finding(&mut self, finding: ReviewFinding) {
        if finding.severity.blocks_merge() && !finding.resolved {
            self.merge_blocked = true;
            if !self.blocking_findings.contains(&finding.id) {
                self.blocking_findings.push(finding.id.clone());
            }
        }
        self.findings.push(finding);
        self.status = ReviewStatus::InProgress;
    }

    /// Set the files that were changed and reviewed
    pub fn set_changed_files(&mut self, files: Vec<String>) {
        self.changed_files = files;
    }

    /// Finalize the review and set status
    pub fn finalize(&mut self) {
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());

        let blocking_unresolved: usize = self
            .findings
            .iter()
            .filter(|f| f.severity.blocks_merge() && !f.resolved)
            .count();

        if blocking_unresolved > 0 {
            self.status = ReviewStatus::Rejected;
            self.merge_blocked = true;
        } else if self.status == ReviewStatus::InProgress {
            self.status = ReviewStatus::Approved;
            self.merge_blocked = false;
        }
    }

    /// Count findings by severity
    pub fn count_by_severity(&self) -> [usize; 4] {
        let mut counts = [0; 4];
        for finding in &self.findings {
            let idx = match finding.severity {
                ReviewSeverity::Critical => 0,
                ReviewSeverity::High => 1,
                ReviewSeverity::Medium => 2,
                ReviewSeverity::Low => 3,
            };
            counts[idx] += 1;
        }
        counts
    }

    /// Check if review is ready for merge
    pub fn is_merge_ready(&self) -> bool {
        self.status == ReviewStatus::Approved && !self.merge_blocked
    }
}

/// Configuration for review requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequirements {
    /// Require automated checks to pass
    pub require_automated: bool,
    /// Require human review for critical changes
    pub require_human_for_critical: bool,
    /// Require tests to pass
    pub require_tests: bool,
    /// Minimum test coverage percentage
    pub min_coverage: Option<f32>,
    /// Block merge on specific finding categories
    pub block_on_categories: Vec<FindingCategory>,
}

impl Default for ReviewRequirements {
    fn default() -> Self {
        Self {
            require_automated: true,
            require_human_for_critical: true,
            require_tests: true,
            min_coverage: Some(80.0),
            block_on_categories: vec![FindingCategory::Security, FindingCategory::Breaking],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_summary_merge_blocked() {
        let mut review = ReviewSummary::new("task-1".to_string());
        assert!(!review.merge_blocked);

        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Security,
            severity: ReviewSeverity::High,
            title: "SQL Injection".to_string(),
            description: "Potential SQL injection".to_string(),
            location: Some(FindingLocation {
                file: "src/db.rs".to_string(),
                line: Some(42),
                column: None,
            }),
            suggestion: Some("Use parameterized queries".to_string()),
            resolved: false,
        });

        assert!(review.merge_blocked);
        assert_eq!(review.blocking_findings, vec!["f1"]);
    }

    #[test]
    fn test_review_finalize_approved() {
        let mut review = ReviewSummary::new("task-1".to_string());
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Maintainability,
            severity: ReviewSeverity::Low,
            title: "Style issue".to_string(),
            description: "Minor style issue".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });

        review.finalize();
        assert_eq!(review.status, ReviewStatus::Approved);
        assert!(!review.merge_blocked);
    }

    #[test]
    fn test_review_finalize_rejected() {
        let mut review = ReviewSummary::new("task-1".to_string());
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Security,
            severity: ReviewSeverity::High,
            title: "Security issue".to_string(),
            description: "Must fix".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });

        review.finalize();
        assert_eq!(review.status, ReviewStatus::Rejected);
        assert!(review.merge_blocked);
    }

    #[test]
    fn test_severity_blocks_merge() {
        assert!(ReviewSeverity::Critical.blocks_merge());
        assert!(ReviewSeverity::High.blocks_merge());
        assert!(!ReviewSeverity::Medium.blocks_merge());
        assert!(!ReviewSeverity::Low.blocks_merge());
    }

    #[test]
    fn test_changed_files_tracking() {
        let mut review = ReviewSummary::new("task-1".to_string());
        review.set_changed_files(vec!["src/main.rs".to_string(), "src/lib.rs".to_string()]);
        assert_eq!(review.changed_files.len(), 2);
    }

    #[test]
    fn test_count_by_severity() {
        let mut review = ReviewSummary::new("task-1".to_string());
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Security,
            severity: ReviewSeverity::Critical,
            title: "Crit".to_string(),
            description: "Crit".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });
        review.add_finding(ReviewFinding {
            id: "f2".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "High".to_string(),
            description: "High".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });
        review.add_finding(ReviewFinding {
            id: "f3".to_string(),
            category: FindingCategory::Maintainability,
            severity: ReviewSeverity::Medium,
            title: "Med".to_string(),
            description: "Med".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });

        let counts = review.count_by_severity();
        assert_eq!(counts[0], 1);
        assert_eq!(counts[1], 1);
        assert_eq!(counts[2], 1);
        assert_eq!(counts[3], 0);
    }
}
