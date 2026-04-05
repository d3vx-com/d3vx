//! PR CI Fix Loop
//!
//! After a task completes and a PR is raised, this module monitors CI status
//! and automatically coordinates fix attempts when checks fail — looping until
//! green or max attempts exceeded.
//!
//! ## Flow
//!
//! ```text
//! PR Created → Monitor CI → [failure → trigger fix → push → re-check] → green
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::pipeline::CIStatus;

/// CI check status for a single check run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckStatusDetail {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

/// Overall CI status for a PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestCIStatus {
    pub pr_number: u64,
    pub repository: String,
    pub head_sha: String,
    pub overall: CIStatus,
    pub checks: Vec<CheckStatusDetail>,
    pub failing_checks: Vec<String>,
}

impl PullRequestCIStatus {
    pub fn is_green(&self) -> bool {
        self.overall == CIStatus::Success
    }

    pub fn is_failed(&self) -> bool {
        self.overall == CIStatus::Failure || self.overall == CIStatus::Error
    }
}

/// Configuration for the CI fix loop.
#[derive(Debug, Clone)]
pub struct CiFixConfig {
    /// Maximum fix attempts before escalation
    pub max_fix_attempts: u32,
    /// Base backoff duration between checks
    pub base_backoff: Duration,
    /// Minutes between CI status polls
    pub poll_interval_secs: u64,
}

impl Default for CiFixConfig {
    fn default() -> Self {
        Self {
            max_fix_attempts: 3,
            base_backoff: Duration::from_secs(60),
            poll_interval_secs: 120,
        }
    }
}

/// Result after running the CI fix loop.
#[derive(Debug, Clone)]
pub struct CiFixResult {
    pub final_status: CIStatus,
    pub fix_attempts: u32,
    pub is_green: bool,
    pub remaining_failures: Vec<String>,
    pub summary: String,
}

/// Tracks CI status and orchestrates fix loops for a PR.
pub struct CiFixLoop {
    config: CiFixConfig,
    repository: String,
    pr_number: u64,
    fix_map: HashMap<String, u32>,
}

impl CiFixLoop {
    pub fn new(config: CiFixConfig, repository: String, pr_number: u64) -> Self {
        Self {
            config,
            repository,
            pr_number,
            fix_map: HashMap::new(),
        }
    }

    /// Fetch CI status using `gh pr checks`.
    pub async fn fetch_ci_status(&self) -> anyhow::Result<PullRequestCIStatus> {
        let output = tokio::process::Command::new("gh")
            .args([
                "pr",
                "checks",
                &self.pr_number.to_string(),
                "--repo",
                &self.repository,
                "--json",
                "name,status,conclusion",
            ])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("gh pr checks failed: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("gh pr checks error: {stderr}"));
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut checks = Vec::new();
        let mut failing = Vec::new();
        let mut any_failed = false;
        let mut all_done = true;

        if let Some(arr) = json.as_array() {
            for item in arr {
                let name = item["name"].as_str().unwrap_or("unknown").to_string();
                let conclusion = item["conclusion"].as_str().map(|s| s.to_string());
                let status = item["status"].as_str().unwrap_or("queued").to_string();

                if conclusion.as_deref() != Some("success") {
                    all_done = false;
                }

                if matches!(conclusion.as_deref(), Some("failure")) {
                    failing.push(name.clone());
                    any_failed = true;
                }

                checks.push(CheckStatusDetail {
                    name,
                    status,
                    conclusion,
                });
            }
        }

        // Get PR head SHA
        let sha_output = tokio::process::Command::new("gh")
            .args([
                "pr",
                "view",
                &self.pr_number.to_string(),
                "--repo",
                &self.repository,
                "--json",
                "headRefOid",
            ])
            .output()
            .await?;

        let head_sha = if sha_output.status.success() {
            serde_json::from_slice::<serde_json::Value>(&sha_output.stdout)
                .ok()
                .and_then(|v| v["headRefOid"].as_str().map(|s| s.to_string()))
                .unwrap_or_default()
        } else {
            String::new()
        };

        let overall = if failing.is_empty() && checks.is_empty() {
            CIStatus::Pending
        } else if any_failed {
            CIStatus::Failure
        } else if all_done && !any_failed {
            CIStatus::Success
        } else {
            CIStatus::Pending
        };

        Ok(PullRequestCIStatus {
            pr_number: self.pr_number,
            repository: self.repository.clone(),
            head_sha,
            overall,
            checks,
            failing_checks: failing,
        })
    }

    /// Fetch comments on the PR that might contain review feedback.
    pub async fn fetch_pr_comments(&self) -> anyhow::Result<Vec<PrComment>> {
        let output = tokio::process::Command::new("gh")
            .args([
                "pr",
                "view",
                &self.pr_number.to_string(),
                "--repo",
                &self.repository,
                "--json",
                "comments",
            ])
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("gh pr view failed: {e}"))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut comments = Vec::new();

        if let Some(arr) = json["comments"].as_array() {
            for item in arr {
                let author = item["author"]["login"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let body = item["body"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let id = item["id"].as_u64().unwrap_or(0);

                comments.push(PrComment { id, author, body });
            }
        }

        Ok(comments)
    }

    /// Run the CI fix loop.
    ///
    /// `on_fix_needed` is called for each fix attempt with the failing check
    /// names. It should apply fixes (push to the PR branch) and return true
    /// if changes were made.
    pub async fn run(&mut self, mut on_fix_needed: impl FnMut(Vec<String>) -> bool) -> CiFixResult {
        let mut fix_attempts = 0;

        loop {
            let status = match self.fetch_ci_status().await {
                Ok(s) => s,
                Err(e) => {
                    return CiFixResult {
                        final_status: CIStatus::Error,
                        fix_attempts,
                        is_green: false,
                        remaining_failures: vec![format!("CI fetch failed: {e}")],
                        summary: format!("CI status fetch error: {e}"),
                    };
                }
            };

            if status.is_green() {
                return CiFixResult {
                    final_status: CIStatus::Success,
                    fix_attempts,
                    is_green: true,
                    remaining_failures: Vec::new(),
                    summary: format!(
                        "PR #{} CI green after {} fix attempt(s)",
                        self.pr_number, fix_attempts
                    ),
                };
            }

            fix_attempts += 1;
            if fix_attempts > self.config.max_fix_attempts {
                return CiFixResult {
                    final_status: CIStatus::Failure,
                    fix_attempts,
                    is_green: false,
                    remaining_failures: status.failing_checks.clone(),
                    summary: format!(
                        "PR #{} exhausted {} fix attempts: {}",
                        self.pr_number,
                        self.config.max_fix_attempts,
                        status.failing_checks.join(", ")
                    ),
                };
            }

            // Signal that a fix is needed
            let changes_made = on_fix_needed(status.failing_checks.clone());
            if !changes_made {
                return CiFixResult {
                    final_status: CIStatus::Failure,
                    fix_attempts,
                    is_green: false,
                    remaining_failures: status.failing_checks.clone(),
                    summary: format!(
                        "PR #{} fix handler reported no changes on attempt {fix_attempts}/{max}",
                        self.pr_number,
                        max = self.config.max_fix_attempts
                    ),
                };
            }

            // Track attempt count
            for check in &status.failing_checks {
                *self.fix_map.entry(check.clone()).or_insert(0) += 1;
            }

            // Backoff before next check
            let backoff = self.config.base_backoff * 2u32.pow(fix_attempts - 1);
            tokio::time::sleep(backoff).await;
        }
    }
}

/// A comment on a PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrComment {
    pub id: u64,
    pub author: String,
    pub body: String,
}

impl PrComment {
    /// Check if this looks like a review comment requesting changes.
    pub fn is_change_request(&self) -> bool {
        let lower = self.body.to_lowercase();
        lower.contains("change")
            || lower.contains("fix")
            || lower.contains("update")
            || lower.contains("nit")
            || lower.contains("suggestion")
    }
}
