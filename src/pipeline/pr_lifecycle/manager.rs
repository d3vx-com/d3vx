//! PR Lifecycle Manager
//!
//! Drives `gh` CLI commands and parses JSON output for PR lifecycle automation.

use tracing::{debug, warn};

use super::types::{CheckConclusion, PrError, PrMetadata, PrState, ReviewInfo, ReviewState};

/// PR lifecycle manager -- drives `gh` CLI commands and parses JSON output.
pub struct PrLifecycleManager {
    pub(crate) repo: Option<String>,
}

impl PrLifecycleManager {
    pub fn new(repo: Option<String>) -> Self {
        Self { repo }
    }

    /// Find an existing open PR for a branch.
    pub async fn find_pr_by_branch(&self, branch: &str) -> Result<Option<PrMetadata>, PrError> {
        let repo_flag = self.repo_flag();

        let mut args: Vec<&str> = vec![
            "pr",
            "list",
            "--head",
            branch,
            "--state",
            "open",
            "--json",
            "number,title,body,url,state",
            "--limit",
            "1",
        ];
        if let Some(ref flag) = repo_flag {
            args.extend_from_slice(&["--repo", flag]);
        }

        let output = self.gh_command(&args).await?;

        let prs: serde_json::Value = serde_json::from_str(&output)
            .map_err(|e| PrError::ParseError(format!("PR list JSON: {e}")))?;

        if let Some(arr) = prs.as_array() {
            if let Some(pr) = arr.first() {
                let pr_number = pr["number"].as_u64();
                let url = pr["url"].as_str().map(String::from);
                let title = pr["title"].as_str().unwrap_or_default().to_string();
                let body = pr["body"].as_str().map(String::from);

                debug!(pr_number = pr_number, branch = %branch, "Found existing PR for branch");

                return Ok(Some(PrMetadata {
                    pr_number,
                    branch: branch.to_string(),
                    state: PrState::Open,
                    title,
                    body,
                    url,
                    ..Default::default()
                }));
            }
        }

        Ok(None)
    }

    /// Create a PR for the given branch using `gh pr create`.
    ///
    /// Returns early if an open PR already exists for the branch.
    pub async fn create_pr(&self, meta: &mut PrMetadata) -> Result<(), PrError> {
        if let Some(existing) = self.find_pr_by_branch(&meta.branch).await? {
            meta.pr_number = existing.pr_number;
            meta.url = existing.url;
            meta.state = PrState::Open;
            debug!(
                pr_number = ?meta.pr_number,
                branch = %meta.branch,
                "PR already exists for branch, skipping create"
            );
            return Ok(());
        }

        let repo_flag = self.repo_flag();

        let mut args: Vec<&str> = vec![
            "pr",
            "create",
            "--head",
            &meta.branch,
            "--title",
            &meta.title,
        ];
        if let Some(body) = &meta.body {
            args.extend_from_slice(&["--body", body.as_str()]);
        } else {
            args.extend_from_slice(&["--body", ""]);
        }
        if let Some(flag) = &repo_flag {
            args.extend_from_slice(&["--repo", flag]);
        }

        let output = self.gh_command(&args).await?;

        let url = output.trim().to_string();
        meta.url = Some(url.clone());
        meta.state = PrState::Open;

        if let Some(num_str) = url.trim_end_matches('/').rsplit('/').next() {
            meta.pr_number = num_str.parse::<u64>().ok();
        }

        debug!(pr_number = ?meta.pr_number, branch = %meta.branch, "PR created");
        Ok(())
    }

    /// Check CI status using `gh pr checks`.
    pub async fn check_ci(&self, meta: &mut PrMetadata) -> Result<(), PrError> {
        let pr_ref = self.pr_ref(meta)?;
        let repo_flag = self.repo_flag();

        let mut args: Vec<&str> = vec!["pr", "checks", &pr_ref, "--output", "json"];
        if let Some(flag) = &repo_flag {
            args.extend_from_slice(&["--repo", flag]);
        }

        let output = self.gh_command(&args).await?;

        let checks: serde_json::Value = serde_json::from_str(&output)
            .map_err(|e| PrError::ParseError(format!("CI checks JSON: {e}")))?;

        meta.ci_checks.clear();

        if let Some(arr) = checks.as_array() {
            for check in arr {
                let name = check["name"].as_str().unwrap_or("unknown").to_string();
                let state = check["state"].as_str().unwrap_or("pending");
                let url = check["link"].as_str().map(String::from);

                let conclusion = match state {
                    "success" | "completed" => CheckConclusion::Success,
                    "failure" | "failed" => CheckConclusion::Failure,
                    "pending" | "queued" => CheckConclusion::Pending,
                    "neutral" => CheckConclusion::Neutral,
                    "cancelled" => CheckConclusion::Cancelled,
                    "timed_out" | "timeout" => CheckConclusion::TimedOut,
                    "action_required" => CheckConclusion::ActionRequired,
                    _ => CheckConclusion::Pending,
                };

                meta.ci_checks.push(super::types::CiStatus {
                    check_name: name,
                    status: conclusion,
                    url,
                });
            }
        }

        if meta.ci_passed() {
            meta.state = PrState::CiPassed;
        } else if meta.ci_failed() {
            meta.state = PrState::CiFailed;
        } else if !meta.ci_checks.is_empty() {
            meta.state = PrState::CiRunning;
        }

        debug!(checks = meta.ci_checks.len(), "CI status refreshed");
        Ok(())
    }

    /// Check review status using `gh pr view`.
    pub async fn check_reviews(&self, meta: &mut PrMetadata) -> Result<(), PrError> {
        let pr_ref = self.pr_ref(meta)?;
        let repo_flag = self.repo_flag();

        let mut args: Vec<&str> = vec!["pr", "view", &pr_ref, "--json", "reviews,state"];
        if let Some(flag) = &repo_flag {
            args.extend_from_slice(&["--repo", flag]);
        }

        let output = self.gh_command(&args).await?;

        let view: serde_json::Value = serde_json::from_str(&output)
            .map_err(|e| PrError::ParseError(format!("PR view JSON: {e}")))?;

        meta.reviews.clear();

        if let Some(reviews) = view["reviews"].as_array() {
            for review in reviews {
                let reviewer = review["author"]["login"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let state_str = review["state"].as_str().unwrap_or("PENDING");
                let body = review["body"].as_str().map(String::from);

                let state = match state_str {
                    "APPROVED" => ReviewState::Approved,
                    "CHANGES_REQUESTED" => ReviewState::ChangesRequested,
                    "COMMENTED" => ReviewState::Commented,
                    "DISMISSED" => ReviewState::Dismissed,
                    _ => ReviewState::Pending,
                };

                meta.reviews.push(ReviewInfo {
                    reviewer,
                    state,
                    body,
                });
            }
        }

        // Derive PR-level state from reviews
        if meta.has_approved_review() {
            meta.state = PrState::Approved;
        } else if meta.has_changes_requested() {
            meta.state = PrState::ChangesRequested;
        } else if !meta.reviews.is_empty() {
            meta.state = PrState::ReviewPending;
        }

        debug!(reviews = meta.reviews.len(), "Reviews refreshed");
        Ok(())
    }

    /// Check mergeability via `gh pr view --json mergeable`.
    pub async fn check_mergeable(&self, meta: &mut PrMetadata) -> Result<(), PrError> {
        let pr_ref = self.pr_ref(meta)?;
        let repo_flag = self.repo_flag();

        let mut args: Vec<&str> = vec!["pr", "view", &pr_ref, "--json", "mergeable"];
        if let Some(flag) = &repo_flag {
            args.extend_from_slice(&["--repo", flag]);
        }

        let output = self.gh_command(&args).await?;

        let view: serde_json::Value = serde_json::from_str(&output)
            .map_err(|e| PrError::ParseError(format!("Mergeable JSON: {e}")))?;

        let mergeable = view["mergeable"].as_str().unwrap_or("UNKNOWN");
        meta.mergeable = Some(matches!(mergeable, "MERGEABLE" | "true"));

        if meta.is_mergeable() {
            meta.state = PrState::Mergeable;
        }

        debug!(mergeable = ?meta.mergeable, "Mergeability checked");
        Ok(())
    }

    /// Merge the PR using `gh pr merge`.
    pub async fn merge(&self, meta: &mut PrMetadata) -> Result<(), PrError> {
        let pr_ref = self.pr_ref(meta)?;
        let repo_flag = self.repo_flag();

        let mut args: Vec<&str> = vec!["pr", "merge", &pr_ref, "--squash", "--auto"];
        if let Some(flag) = &repo_flag {
            args.extend_from_slice(&["--repo", flag]);
        }

        self.gh_command(&args).await?;
        meta.state = PrState::Merged;

        debug!(pr_number = ?meta.pr_number, "PR merged");
        Ok(())
    }

    /// Full lifecycle refresh -- updates CI, reviews, and mergeability.
    pub async fn refresh(&self, meta: &mut PrMetadata) -> Result<(), PrError> {
        if meta.pr_number.is_none() {
            return Ok(());
        }

        // Run checks in sequence -- each mutates `meta`
        self.check_ci(meta).await?;
        self.check_reviews(meta).await?;
        self.check_mergeable(meta).await?;

        debug!(state = ?meta.state, "PR metadata refreshed");
        Ok(())
    }

    // -- helpers ---------------------------------------------------------------

    pub(crate) fn repo_flag(&self) -> Option<String> {
        self.repo.clone()
    }

    pub(crate) fn pr_ref(&self, meta: &PrMetadata) -> Result<String, PrError> {
        meta.pr_number
            .map(|n| n.to_string())
            .ok_or(PrError::CommandFailed("No PR number set".into()))
    }

    /// Execute a `gh` CLI command and return stdout.
    async fn gh_command(&self, args: &[&str]) -> Result<String, PrError> {
        let output = tokio::process::Command::new("gh")
            .args(args)
            .output()
            .await
            .map_err(|e| PrError::CliNotAvailable(format!("Failed to run gh: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(%stderr, "gh command failed");
            return Err(PrError::CommandFailed(stderr.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
