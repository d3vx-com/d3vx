//! Issue Tracker Client
//!
//! GitHub operations use the `gh` CLI; Linear is a future stub.

use tracing::{debug, warn};

use super::types::{ExternalIssue, IssueState, SyncError, TrackerKind};

/// Issue tracker client. Uses `gh issue` CLI for GitHub; Linear is stubbed.
pub struct IssueTracker {
    pub(crate) kind: TrackerKind,
    pub(crate) repo: Option<String>,
    #[allow(dead_code)] // Reserved for Linear integration
    pub(crate) linear_api_key: Option<String>,
}

impl IssueTracker {
    /// Create a GitHub-backed tracker targeting the given repo (e.g. "owner/repo").
    pub fn github(repo: String) -> Self {
        Self {
            kind: TrackerKind::Github,
            repo: Some(repo),
            linear_api_key: None,
        }
    }

    /// Create a Linear-backed tracker with an API key.
    pub fn linear(api_key: String) -> Self {
        Self {
            kind: TrackerKind::Linear,
            repo: None,
            linear_api_key: Some(api_key),
        }
    }

    /// Fetch open issues from the tracker.
    pub async fn list_open_issues(&self) -> Result<Vec<ExternalIssue>, SyncError> {
        match self.kind {
            TrackerKind::Github => self.list_github_issues().await,
            TrackerKind::Linear => {
                debug!("Linear issue listing not yet implemented");
                Err(SyncError::NotConfigured)
            }
        }
    }

    /// Find an existing open issue by exact title.
    pub async fn find_issue_by_title(
        &self,
        title: &str,
    ) -> Result<Option<ExternalIssue>, SyncError> {
        match self.kind {
            TrackerKind::Github => self.find_github_issue_by_title(title).await,
            TrackerKind::Linear => {
                debug!("Linear issue lookup not yet implemented");
                Err(SyncError::NotConfigured)
            }
        }
    }

    /// Create an issue in the tracker.
    ///
    /// Returns an existing issue if one with the same title is already open.
    pub async fn create_issue(
        &self,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<ExternalIssue, SyncError> {
        match self.kind {
            TrackerKind::Github => self.create_github_issue(title, body, labels).await,
            TrackerKind::Linear => {
                debug!("Linear issue creation not yet implemented");
                Err(SyncError::NotConfigured)
            }
        }
    }

    /// Update issue state (close / reopen).
    pub async fn update_state(&self, issue_id: &str, state: IssueState) -> Result<(), SyncError> {
        match self.kind {
            TrackerKind::Github => self.update_github_state(issue_id, state).await,
            TrackerKind::Linear => {
                debug!("Linear state update not yet implemented");
                Err(SyncError::NotConfigured)
            }
        }
    }

    /// Link a PR to an issue by adding a comment reference.
    pub async fn link_pr(&self, issue_id: &str, pr_number: u64) -> Result<(), SyncError> {
        match self.kind {
            TrackerKind::Github => {
                let repo = self.repo.as_deref().ok_or(SyncError::NotConfigured)?;
                let comment = format!("Linked to PR #{pr_number}");
                self.gh_command(&[
                    "issue", "comment", issue_id, "--body", &comment, "--repo", repo,
                ])
                .await?;
                debug!(issue = issue_id, pr = pr_number, "PR linked to issue");
                Ok(())
            }
            TrackerKind::Linear => Err(SyncError::NotConfigured),
        }
    }

    // -- GitHub implementation -----------------------------------------------

    async fn list_github_issues(&self) -> Result<Vec<ExternalIssue>, SyncError> {
        let repo = self.repo.as_deref().ok_or(SyncError::NotConfigured)?;

        let output = self
            .gh_command(&[
                "issue",
                "list",
                "--state",
                "open",
                "--limit",
                "100",
                "--json",
                "number,title,body,labels,state,assignees,url",
                "--repo",
                repo,
            ])
            .await?;

        let raw_issues: serde_json::Value = serde_json::from_str(&output)
            .map_err(|e| SyncError::ParseError(format!("GitHub issue list JSON: {e}")))?;

        let mut issues = Vec::new();

        if let Some(arr) = raw_issues.as_array() {
            for item in arr {
                let number = item["number"].as_u64();
                let title = item["title"].as_str().unwrap_or_default().to_string();
                let body = item["body"].as_str().map(String::from);

                let labels = item["labels"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|l| l["name"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let assignee = item["assignees"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|a| a["login"].as_str().map(String::from));

                let url = item["url"].as_str().map(String::from);
                let id = number.map(|n| n.to_string()).unwrap_or_default();

                issues.push(ExternalIssue {
                    id,
                    number,
                    title,
                    body,
                    state: IssueState::Open,
                    labels,
                    assignee,
                    url,
                    tracker: TrackerKind::Github,
                });
            }
        }

        debug!(count = issues.len(), "Fetched GitHub issues");
        Ok(issues)
    }

    async fn find_github_issue_by_title(
        &self,
        title: &str,
    ) -> Result<Option<ExternalIssue>, SyncError> {
        let issues = self.list_github_issues().await?;
        Ok(issues.into_iter().find(|i| i.title == title))
    }

    async fn create_github_issue(
        &self,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<ExternalIssue, SyncError> {
        if let Some(existing) = self.find_github_issue_by_title(title).await? {
            debug!(issue = existing.number, title = %title, "Found existing issue with same title, returning existing");
            return Ok(existing);
        }

        let repo = self.repo.as_deref().ok_or(SyncError::NotConfigured)?;

        let label_str = labels.join(",");

        let args: Vec<&str> = vec![
            "issue", "create", "--title", title, "--body", body, "--label", &label_str, "--repo",
            repo,
        ];

        let output = self.gh_command(&args).await?;

        // gh issue create outputs the issue URL
        let url = output.trim().to_string();
        let number = url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .and_then(|s| s.parse::<u64>().ok());

        debug!(?number, %title, "Created GitHub issue");

        Ok(ExternalIssue {
            id: number.map(|n| n.to_string()).unwrap_or_default(),
            number,
            title: title.to_string(),
            body: Some(body.to_string()),
            state: IssueState::Open,
            labels: labels.to_vec(),
            assignee: None,
            url: Some(url),
            tracker: TrackerKind::Github,
        })
    }

    async fn update_github_state(
        &self,
        issue_id: &str,
        state: IssueState,
    ) -> Result<(), SyncError> {
        let repo = self.repo.as_deref().ok_or(SyncError::NotConfigured)?;

        let state_arg = match state {
            IssueState::Closed | IssueState::Cancelled => "closed",
            IssueState::Open | IssueState::InProgress => "open",
        };

        self.gh_command(&[
            "issue", "edit", issue_id, "--state", state_arg, "--repo", repo,
        ])
        .await?;

        debug!(issue = issue_id, ?state, "Updated GitHub issue state");
        Ok(())
    }

    // -- CLI helper ----------------------------------------------------------

    async fn gh_command(&self, args: &[&str]) -> Result<String, SyncError> {
        let output = tokio::process::Command::new("gh")
            .args(args)
            .output()
            .await
            .map_err(|e| SyncError::Unavailable(format!("Failed to run gh: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(%stderr, "gh command failed");
            return Err(SyncError::ApiError(stderr.to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
