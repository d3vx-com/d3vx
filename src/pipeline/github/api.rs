//! GitHub API HTTP Client
//!
//! Provides methods for interacting with the GitHub REST API:
//! fetching issues, creating issues, creating PRs, and posting comments.

use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};

use super::types::{
    CreateCommentRequest, CreateIssueRequest, CreatePullRequestRequest, GitHubConfig, GitHubIssue,
    GitHubIssueResponse, GitHubPullRequest, GitHubPullRequestResponse,
};

// ═══════════════════════════════════════════════════════════════════════════════
// API Client
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone)]
pub struct GitHubApiClient {
    client: reqwest::Client,
    api_base_url: String,
}

impl GitHubApiClient {
    pub fn from_config(config: &GitHubConfig) -> Result<Self> {
        let token = std::env::var(&config.token_env)
            .map_err(|_| anyhow::anyhow!("GitHub token env {} is not set", config.token_env))?;

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("d3vx-terminal"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            api_base_url: config.api_base_url.trim_end_matches('/').to_string(),
        })
    }

    pub async fn fetch_open_issues(
        &self,
        repository: &str,
        since: Option<DateTime<Utc>>,
        labels: &[String],
    ) -> Result<Vec<GitHubIssue>> {
        let url = format!("{}/repos/{}/issues", self.api_base_url, repository);
        let mut request = self.client.get(&url).query(&[
            ("state", "open"),
            ("per_page", "100"),
            ("sort", "updated"),
            ("direction", "desc"),
        ]);

        if !labels.is_empty() {
            request = request.query(&[("labels", labels.join(","))]);
        }
        if let Some(since) = since {
            request = request.query(&[("since", since.to_rfc3339())]);
        }

        let response = request.send().await?.error_for_status()?;
        let items: Vec<GitHubIssueResponse> = response.json().await?;

        Ok(items
            .into_iter()
            .filter(|item| item.pull_request.is_none())
            .map(|item| GitHubIssue {
                repository: repository.to_string(),
                number: item.number,
                title: item.title,
                body: item.body,
                state: item.state,
                labels: item.labels.into_iter().map(|l| l.name).collect(),
                author: item.user.login,
                created_at: item.created_at,
                updated_at: item.updated_at,
            })
            .collect())
    }

    pub async fn create_issue(
        &self,
        repository: &str,
        title: &str,
        body: &str,
        labels: Vec<String>,
    ) -> Result<GitHubIssue> {
        let url = format!("{}/repos/{}/issues", self.api_base_url, repository);
        let response = self
            .client
            .post(&url)
            .json(&CreateIssueRequest {
                title,
                body,
                labels,
            })
            .send()
            .await?
            .error_for_status()?;

        let item: GitHubIssueResponse = response.json().await?;
        Ok(GitHubIssue {
            repository: repository.to_string(),
            number: item.number,
            title: item.title,
            body: item.body,
            state: item.state,
            labels: item.labels.into_iter().map(|l| l.name).collect(),
            author: item.user.login,
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
    }

    pub async fn create_pull_request(
        &self,
        repository: &str,
        title: &str,
        head: &str,
        base: &str,
        body: &str,
    ) -> Result<GitHubPullRequest> {
        let url = format!("{}/repos/{}/pulls", self.api_base_url, repository);
        let response = self
            .client
            .post(&url)
            .json(&CreatePullRequestRequest {
                title,
                head,
                base,
                body,
            })
            .send()
            .await?
            .error_for_status()?;

        let pr: GitHubPullRequestResponse = response.json().await?;
        Ok(GitHubPullRequest {
            number: pr.number,
            html_url: pr.html_url,
            state: pr.state,
        })
    }

    pub async fn find_open_pull_request(
        &self,
        repository: &str,
        head_branch: &str,
        base_branch: &str,
    ) -> Result<Option<GitHubPullRequest>> {
        let owner = repository
            .split('/')
            .next()
            .ok_or_else(|| anyhow::anyhow!("invalid repository format: {}", repository))?;
        let head = format!("{}:{}", owner, head_branch);
        let url = format!("{}/repos/{}/pulls", self.api_base_url, repository);
        let response = self
            .client
            .get(&url)
            .query(&[
                ("state", "open"),
                ("head", head.as_str()),
                ("base", base_branch),
            ])
            .send()
            .await?
            .error_for_status()?;

        let prs: Vec<serde_json::Value> = response.json().await?;
        let Some(first) = prs.into_iter().find(|pr| {
            pr.get("head")
                .and_then(|head| head.get("ref"))
                .and_then(|r| r.as_str())
                == Some(head_branch)
        }) else {
            return Ok(None);
        };

        let pr: GitHubPullRequestResponse = serde_json::from_value(first)?;
        Ok(Some(GitHubPullRequest {
            number: pr.number,
            html_url: pr.html_url,
            state: pr.state,
        }))
    }

    pub async fn create_issue_comment(
        &self,
        repository: &str,
        issue_number: u64,
        body: &str,
    ) -> Result<()> {
        let url = format!(
            "{}/repos/{}/issues/{}/comments",
            self.api_base_url, repository, issue_number
        );
        self.client
            .post(&url)
            .json(&CreateCommentRequest { body })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
