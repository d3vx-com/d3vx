//! Review Comment Response Loop
//!
//! When review comments arrive on a PR, this module extracts actionable
//! feedback from GitHub's native review states, coordinates fixes, and
//! requests re-review.
//!
//! ## Design
//!
//! Uses GitHub's official review system instead of keyword heuristics:
//! - `CHANGES_REQUESTED` reviews are actionable
//! - `isResolved: false` threads need work
//! - `APPROVED` / `COMMENTED` are informational, not blocking
//!
//! ## Flow
//!
//! ```text
//! Review (CHANGES_REQUESTED) → Extract threads → Address feedback → Push → Request re-review
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Official GitHub review state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GitHubReviewState {
    /// Reviewer approved the changes
    Approved,
    /// Reviewer requested changes
    ChangesRequested,
    /// Reviewer left comments without a verdict
    Commented,
    /// Review was dismissed
    Dismissed,
    /// Unknown state
    Pending,
}

impl GitHubReviewState {
    pub fn is_actionable(&self) -> bool {
        *self == Self::ChangesRequested
    }

    pub fn is_blocking_merge(&self) -> bool {
        matches!(self, Self::ChangesRequested | Self::Pending)
    }
}

/// A single review left on a PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubReview {
    pub id: String,
    pub author: String,
    pub state: GitHubReviewState,
    pub body: String,
    pub submitted_at: Option<String>,
}

/// A thread of review comments on a specific file/location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewThread {
    /// Whether GitHub has marked this thread as resolved
    pub is_resolved: bool,
    /// File path this thread is on
    pub path: Option<String>,
    /// Start line (for multi-line comments)
    pub start_line: Option<u32>,
    /// End line
    pub line: Option<u32>,
    /// All comments in this thread
    pub comments: Vec<ThreadComment>,
}

impl ReviewThread {
    /// Whether this thread still needs attention.
    /// Only uses GitHub's native resolution state.
    pub fn needs_attention(&self) -> bool {
        !self.is_resolved
    }
}

/// A single comment inside a review thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadComment {
    pub id: String,
    pub author: String,
    pub body: String,
    pub created_at: Option<String>,
}

/// Structured actionable feedback extracted from reviews.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionableFeedback {
    /// Which review thread this came from
    pub thread_id: String,
    /// File involved (if any)
    pub file: Option<String>,
    /// Line number (if any)
    pub line: Option<u32>,
    /// The comment text
    pub comment: String,
    /// Who left the comment
    pub reviewer: String,
    /// The official review state this feedback belongs to
    pub review_state: GitHubReviewState,
}

/// Report of all review feedback on a PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCommentsReport {
    pub pr_number: u64,
    pub repository: String,
    /// All official reviews left on this PR
    pub reviews: Vec<GitHubReview>,
    /// All unresolved review threads
    pub unresolved_threads: Vec<ReviewThread>,
    /// Extracted actionable items (from CHANGES_REQUESTED reviews)
    pub actionable: Vec<ActionableFeedback>,
    /// Count of reviews with changes requested
    pub changes_requested_count: usize,
}

impl ReviewCommentsReport {
    /// Whether there are blocking reviews that need a response.
    pub fn has_blocking_reviews(&self) -> bool {
        self.reviews.iter().any(|r| r.state.is_blocking_merge())
    }

    /// Whether there's anything left to fix.
    pub fn needs_changes(&self) -> bool {
        !self.actionable.is_empty()
    }
}

/// State for the review response loop on a PR.
pub struct ReviewResponseLoop {
    repository: String,
    pr_number: u64,
    processed_thread_ids: HashSet<String>,
    max_response_attempts: u32,
    response_attempts: u32,
}

impl ReviewResponseLoop {
    pub fn new(repository: String, pr_number: u64) -> Self {
        Self {
            repository,
            pr_number,
            processed_thread_ids: HashSet::new(),
            max_response_attempts: 3,
            response_attempts: 0,
        }
    }

    pub fn with_max_attempts(mut self, max: u32) -> Self {
        self.max_response_attempts = max;
        self
    }

    /// Fetch official reviews and review threads from the PR.
    pub async fn fetch_review_report(&self) -> anyhow::Result<ReviewCommentsReport> {
        let pr_str = self.pr_number.to_string();
        let _repo = &self.repository;

        // Fetch reviews: official review states
        let reviews = self.fetch_reviews(&pr_str).await?;
        let changes_requested_count = reviews
            .iter()
            .filter(|r| r.state == GitHubReviewState::ChangesRequested)
            .count();

        // Fetch review threads: individual comment threads with resolution state
        let threads = self.fetch_review_threads(&pr_str).await?;

        // Extract actionable feedback from CHANGES_REQUESTED reviews
        let actionable = self.extract_actionable_feedback(&reviews, &threads);

        // Filter to unresolved threads only
        let unresolved_threads: Vec<_> = threads
            .into_iter()
            .filter(|t| {
                !self
                    .processed_thread_ids
                    .contains(&format!("thread-{}", t.line.unwrap_or(0)))
                    && t.needs_attention()
            })
            .collect();

        Ok(ReviewCommentsReport {
            pr_number: self.pr_number,
            repository: self.repository.clone(),
            reviews,
            unresolved_threads,
            actionable,
            changes_requested_count,
        })
    }

    /// Fetch official PR reviews using `gh pr view`.
    async fn fetch_reviews(&self, pr_str: &str) -> anyhow::Result<Vec<GitHubReview>> {
        let output = tokio::process::Command::new("gh")
            .args([
                "pr",
                "view",
                pr_str,
                "--repo",
                &self.repository,
                "--json",
                "reviews",
            ])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("gh pr view failed: {e}"))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut reviews = Vec::new();

        if let Some(arr) = json["reviews"].as_array() {
            for item in arr {
                let state_str = item["state"].as_str().unwrap_or("COMMENTED");
                let state = match state_str {
                    "APPROVED" => GitHubReviewState::Approved,
                    "CHANGES_REQUESTED" => GitHubReviewState::ChangesRequested,
                    "COMMENTED" => GitHubReviewState::Commented,
                    "DISMISSED" => GitHubReviewState::Dismissed,
                    "PENDING" => GitHubReviewState::Pending,
                    _ => GitHubReviewState::Commented,
                };

                reviews.push(GitHubReview {
                    id: item["id"]
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    author: item["author"]["login"]
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    state,
                    body: item["body"]
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    submitted_at: item["submittedAt"].as_str().map(|s| s.to_string()),
                });
            }
        }

        Ok(reviews)
    }

    /// Fetch review threads using `gh pr view --json reviewThreads`.
    async fn fetch_review_threads(&self, pr_str: &str) -> anyhow::Result<Vec<ReviewThread>> {
        let output = tokio::process::Command::new("gh")
            .args([
                "pr",
                "view",
                pr_str,
                "--repo",
                &self.repository,
                "--json",
                "reviewThreads",
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut threads = Vec::new();

        if let Some(arr) = json["reviewThreads"].as_array() {
            for item in arr {
                let is_resolved = item["isResolved"].as_bool().unwrap_or(false);
                let path = item["path"].as_str().map(|s| s.to_string());
                let line = item["line"].as_u64().map(|n| n as u32);
                let start_line = item["startLine"].as_u64().map(|n| n as u32);

                let mut comments = Vec::new();
                if let Some(comments_arr) = item["comments"].as_array() {
                    for c in comments_arr {
                        comments.push(ThreadComment {
                            id: c["databaseId"]
                                .as_u64()
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                            author: c["author"]["login"]
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "unknown".to_string()),
                            body: c["body"]
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_default(),
                            created_at: c["createdAt"].as_str().map(|s| s.to_string()),
                        });
                    }
                }

                threads.push(ReviewThread {
                    is_resolved,
                    path,
                    start_line,
                    line,
                    comments,
                });
            }
        }

        Ok(threads)
    }

    /// Extract actionable feedback from reviews that requested changes.
    fn extract_actionable_feedback(
        &self,
        reviews: &[GitHubReview],
        threads: &[ReviewThread],
    ) -> Vec<ActionableFeedback> {
        // Collect reviewer names who requested changes (owned Strings)
        let requested_reviewers: Vec<String> = reviews
            .iter()
            .filter(|r| r.state == GitHubReviewState::ChangesRequested)
            .map(|r| r.author.clone())
            .collect();

        threads
            .iter()
            .filter(|t| !t.is_resolved && !t.comments.is_empty())
            .flat_map(|thread| {
                thread
                    .comments
                    .iter()
                    .filter(|c| requested_reviewers.contains(&c.author))
                    .map(|c| {
                        let thread_id = format!(
                            "{}-{}-{}",
                            thread.path.as_deref().unwrap_or("general"),
                            thread.line.unwrap_or(0),
                            c.id
                        );
                        ActionableFeedback {
                            thread_id,
                            file: thread.path.clone(),
                            line: thread.line,
                            comment: c.body.clone(),
                            reviewer: c.author.clone(),
                            review_state: GitHubReviewState::ChangesRequested,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Mark a thread as processed (its feedback was addressed).
    pub fn mark_thread_processed(&mut self, thread_id: &str) {
        self.processed_thread_ids.insert(thread_id.to_string());
    }

    /// Respond to review comments by applying fixes.
    ///
    /// `apply_feedback` receives actionable feedback and should make the
    /// necessary code changes and commit them. Returns true if changes were
    /// made.
    ///
    /// `push_changes` should push the branch to origin.
    pub async fn address_comments(
        &mut self,
        report: &ReviewCommentsReport,
        apply_feedback: impl Fn(&[ActionableFeedback]) -> bool,
        push_changes: impl FnOnce() -> anyhow::Result<()>,
    ) -> anyhow::Result<ReviewResponseResult> {
        if !report.needs_changes() {
            return Ok(ReviewResponseResult {
                success: true,
                summary: "No blocking reviews or unresolved threads".to_string(),
            });
        }

        self.response_attempts += 1;
        if self.response_attempts > self.max_response_attempts {
            return Ok(ReviewResponseResult {
                success: false,
                summary: format!(
                    "Exceeded max review response attempts ({})",
                    self.max_response_attempts
                ),
            });
        }

        let changes_made = apply_feedback(&report.actionable);
        if !changes_made {
            return Ok(ReviewResponseResult {
                success: false,
                summary: "No code changes could be made for review feedback".to_string(),
            });
        }

        push_changes()?;

        // Mark all processed threads
        for feedback in &report.actionable {
            self.mark_thread_processed(&feedback.thread_id);
        }

        Ok(ReviewResponseResult {
            success: true,
            summary: format!(
                "Addressed {} review comment(s) from {} blocking review(s), attempt {}/{}",
                report.actionable.len(),
                report.changes_requested_count,
                self.response_attempts,
                self.max_response_attempts
            ),
        })
    }

    /// Request re-review from reviewers after pushing fixes.
    pub async fn request_re_review(&self, reviewers: &[String]) -> anyhow::Result<()> {
        if reviewers.is_empty() {
            return Ok(());
        }

        let pr_str = self.pr_number.to_string();
        let reviewer_args: Vec<&str> = reviewers.iter().map(|s| s.as_str()).collect();
        let repo = self.repository.as_str();
        let mut args: Vec<&str> = vec!["pr", "edit", &pr_str, "--add-reviewer"];
        for r in &reviewer_args {
            args.push(r);
        }
        args.push("--repo");
        args.push(repo);

        let output = tokio::process::Command::new("gh")
            .args(&args)
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("gh pr edit failed: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("gh pr edit error: {stderr}"));
        }

        Ok(())
    }
}

/// Result of responding to review comments.
#[derive(Debug, Clone)]
pub struct ReviewResponseResult {
    pub success: bool,
    pub summary: String,
}
