//! Types for PR Lifecycle Automation
//!
//! Core data structures: PrState, CiStatus, ReviewInfo, PrMetadata, PrError.

use serde::{Deserialize, Serialize};

/// PR state in the delivery lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    NotCreated,
    Open,
    CiRunning,
    CiPassed,
    CiFailed,
    ReviewPending,
    ChangesRequested,
    Approved,
    Mergeable,
    Merged,
    Closed,
}

/// CI check status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiStatus {
    pub check_name: String,
    pub status: CheckConclusion,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckConclusion {
    Pending,
    Success,
    Failure,
    Neutral,
    Cancelled,
    TimedOut,
    ActionRequired,
}

/// Review information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewInfo {
    pub reviewer: String,
    pub state: ReviewState,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Pending,
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
}

/// PR metadata tracked by d3vx
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMetadata {
    pub pr_number: Option<u64>,
    pub branch: String,
    pub state: PrState,
    pub title: String,
    pub body: Option<String>,
    pub url: Option<String>,
    pub ci_checks: Vec<CiStatus>,
    pub reviews: Vec<ReviewInfo>,
    pub mergeable: Option<bool>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl Default for PrMetadata {
    fn default() -> Self {
        Self {
            pr_number: None,
            branch: String::new(),
            state: PrState::NotCreated,
            title: String::new(),
            body: None,
            url: None,
            ci_checks: Vec::new(),
            reviews: Vec::new(),
            mergeable: None,
            created_at: None,
            updated_at: None,
        }
    }
}

impl PrMetadata {
    /// Create metadata for a branch that does not yet have a PR.
    pub fn new(branch: &str) -> Self {
        Self {
            branch: branch.to_string(),
            ..Self::default()
        }
    }

    /// True when every CI check has succeeded.
    pub fn ci_passed(&self) -> bool {
        !self.ci_checks.is_empty()
            && self
                .ci_checks
                .iter()
                .all(|c| c.status == CheckConclusion::Success)
    }

    /// True when at least one CI check has failed, timed out, or been cancelled.
    pub fn ci_failed(&self) -> bool {
        self.ci_checks.iter().any(|c| {
            matches!(
                c.status,
                CheckConclusion::Failure | CheckConclusion::TimedOut | CheckConclusion::Cancelled
            )
        })
    }

    /// True when at least one review is in the Approved state.
    pub fn has_approved_review(&self) -> bool {
        self.reviews
            .iter()
            .any(|r| r.state == ReviewState::Approved)
    }

    /// True when at least one review requested changes.
    pub fn has_changes_requested(&self) -> bool {
        self.reviews
            .iter()
            .any(|r| r.state == ReviewState::ChangesRequested)
    }

    /// True when the PR is approved, CI has passed, and GitHub reports it mergeable.
    pub fn is_mergeable(&self) -> bool {
        self.has_approved_review() && self.ci_passed() && self.mergeable.unwrap_or(false)
    }

    /// Return reviews that are still pending (useful for tracking blockers).
    pub fn pending_review_comments(&self) -> Vec<&ReviewInfo> {
        self.reviews
            .iter()
            .filter(|r| r.state == ReviewState::Pending)
            .collect()
    }
}

/// Errors that can occur during PR lifecycle operations.
#[derive(Debug, thiserror::Error)]
pub enum PrError {
    #[error("GitHub CLI not available: {0}")]
    CliNotAvailable(String),
    #[error("PR command failed: {0}")]
    CommandFailed(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("No repository configured")]
    NoRepo,
}
