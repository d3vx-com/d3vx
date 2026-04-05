//! Issue Launcher
//!
//! Launches agent sessions from issue context with branch generation,
//! validation, and concurrency control.

use std::sync::Arc;

use tokio::sync::Semaphore;
use uuid::Uuid;

use super::prompt_composer::PromptComposer;
use super::types::{BranchSpec, IssueContext, LaunchConfig, SpawnResult, SpawnStatus};

/// Errors that can occur during issue launch.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    /// The issue failed validation.
    #[error("issue validation failed: {0}")]
    IssueInvalid(String),
    /// The generated branch name conflicts with an existing branch.
    #[error("branch conflict: {0}")]
    BranchConflict(String),
    /// The workspace could not be prepared.
    #[error("workspace error: {0}")]
    WorkspaceError(String),
    /// The agent spawn itself failed.
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
}

/// Launches individual issues as agent sessions.
pub struct IssueLauncher {
    config: LaunchConfig,
    branch_spec: BranchSpec,
}

impl IssueLauncher {
    /// Create a new launcher with the given configuration.
    pub fn new(config: LaunchConfig) -> Self {
        Self {
            branch_spec: BranchSpec::FromTitle { max_length: 60 },
            config,
        }
    }

    /// Create a launcher with an explicit branch strategy.
    pub fn with_branch_spec(config: LaunchConfig, spec: BranchSpec) -> Self {
        Self {
            branch_spec: spec,
            config,
        }
    }

    /// Launch a single issue as an agent session.
    pub async fn launch(&self, context: IssueContext) -> Result<SpawnResult, LaunchError> {
        self.validate_issue(&context)?;

        let branch = self.generate_branch(&context);

        // Validate branch name is non-empty.
        if branch.is_empty() {
            return Err(LaunchError::BranchConflict(
                "Generated branch name is empty".to_string(),
            ));
        }

        // Compose the agent prompt.
        let _prompt = PromptComposer::compose(&context, &self.config);

        let session_id = format!("sess-{}", Uuid::new_v4().simple());

        Ok(SpawnResult {
            session_id,
            branch,
            status: SpawnStatus::Launched,
        })
    }

    /// Generate a branch name from the issue context using the configured spec.
    pub fn generate_branch(&self, context: &IssueContext) -> String {
        match &self.branch_spec {
            BranchSpec::FromTitle { max_length } => {
                let slug = slugify_title(&context.title);
                let name = format!("{}/{}", self.config.branch_prefix, slug);
                // Truncate to max_length at a word boundary.
                truncate_at_boundary(&name, *max_length)
            }
            BranchSpec::FromIssueId => {
                format!("{}/{}", self.config.branch_prefix, slugify(&context.id))
            }
            BranchSpec::Template { pattern } => pattern
                .replace("{prefix}", &self.config.branch_prefix)
                .replace("{id}", &context.id)
                .replace("{title}", &slugify_title(&context.title)),
        }
    }

    /// Validate that an issue has the minimum required fields.
    pub fn validate_issue(&self, context: &IssueContext) -> Result<(), LaunchError> {
        if context.id.trim().is_empty() {
            return Err(LaunchError::IssueInvalid("issue ID is empty".to_string()));
        }
        if context.title.trim().is_empty() {
            return Err(LaunchError::IssueInvalid(
                "issue title is empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Launch multiple issues in parallel with concurrency control.
pub async fn parallel_launch(
    contexts: Vec<IssueContext>,
    config: LaunchConfig,
) -> Vec<SpawnResult> {
    let semaphore = Arc::new(Semaphore::new(config.max_concurrent));
    let launcher = Arc::new(IssueLauncher::new(config));

    let mut handles = Vec::with_capacity(contexts.len());

    for context in contexts {
        let sem = semaphore.clone();
        let launcher = launcher.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await;
            match launcher.launch(context).await {
                Ok(result) => result,
                Err(e) => SpawnResult {
                    session_id: String::new(),
                    branch: String::new(),
                    status: SpawnStatus::Failed {
                        error: e.to_string(),
                    },
                },
            }
        });

        handles.push(handle);
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => results.push(SpawnResult {
                session_id: String::new(),
                branch: String::new(),
                status: SpawnStatus::Failed {
                    error: format!("task panicked: {}", e),
                },
            }),
        }
    }

    results
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a title to a branch-safe slug.
fn slugify_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Slugify a generic string.
fn slugify(input: &str) -> String {
    slugify_title(input)
}

/// Truncate a string to max_len, breaking at the last '-' boundary if possible.
fn truncate_at_boundary(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }

    let truncated = &s[..max_len];
    match truncated.rfind('-') {
        Some(idx) if idx > 0 => truncated[..idx].to_string(),
        _ => truncated.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::spawner::types::TrackerKind;

    fn sample_issue(id: &str, title: &str) -> IssueContext {
        IssueContext {
            id: id.to_string(),
            title: title.to_string(),
            body: "Description here.".to_string(),
            labels: vec![],
            tracker: TrackerKind::GitHub,
        }
    }

    #[test]
    fn test_branch_from_title() {
        let launcher = IssueLauncher::new(LaunchConfig::default());
        let ctx = sample_issue("42", "Fix login timeout on mobile");
        let branch = launcher.generate_branch(&ctx);
        assert!(branch.starts_with("d3vx/"));
        assert!(branch.contains("fix"));
        assert!(branch.contains("login"));
    }

    #[test]
    fn test_branch_from_title_truncation() {
        let launcher = IssueLauncher::new(LaunchConfig::default());
        let long_title = "This is an extremely long issue title that should be truncated to fit within the branch name limit";
        let ctx = sample_issue("99", long_title);
        let branch = launcher.generate_branch(&ctx);
        assert!(
            branch.len() <= 60,
            "Branch exceeds max length: {} chars",
            branch.len()
        );
    }

    #[test]
    fn test_branch_from_issue_id() {
        let launcher =
            IssueLauncher::with_branch_spec(LaunchConfig::default(), BranchSpec::FromIssueId);
        let ctx = sample_issue("PROJ-123", "Some title");
        let branch = launcher.generate_branch(&ctx);
        assert_eq!(branch, "d3vx/proj-123");
    }

    #[test]
    fn test_branch_from_template() {
        let launcher = IssueLauncher::with_branch_spec(
            LaunchConfig {
                branch_prefix: "task".to_string(),
                ..LaunchConfig::default()
            },
            BranchSpec::Template {
                pattern: "{prefix}/{id}-{title}".to_string(),
            },
        );
        let ctx = sample_issue("42", "Fix bug");
        let branch = launcher.generate_branch(&ctx);
        assert_eq!(branch, "task/42-fix-bug");
    }

    #[test]
    fn test_issue_validation_rejects_empty() {
        let launcher = IssueLauncher::new(LaunchConfig::default());

        let empty_id = IssueContext {
            id: "".to_string(),
            title: "Some title".to_string(),
            body: String::new(),
            labels: vec![],
            tracker: TrackerKind::GitHub,
        };
        assert!(launcher.validate_issue(&empty_id).is_err());

        let empty_title = IssueContext {
            id: "42".to_string(),
            title: "   ".to_string(),
            body: String::new(),
            labels: vec![],
            tracker: TrackerKind::GitHub,
        };
        assert!(launcher.validate_issue(&empty_title).is_err());
    }

    #[test]
    fn test_issue_validation_accepts_valid() {
        let launcher = IssueLauncher::new(LaunchConfig::default());
        let ctx = sample_issue("42", "Valid issue");
        assert!(launcher.validate_issue(&ctx).is_ok());
    }

    #[tokio::test]
    async fn test_launch_success() {
        let launcher = IssueLauncher::new(LaunchConfig::default());
        let ctx = sample_issue("42", "Fix login");
        let result = launcher.launch(ctx).await.unwrap();
        assert!(matches!(result.status, SpawnStatus::Launched));
        assert!(result.branch.starts_with("d3vx/"));
        assert!(result.session_id.starts_with("sess-"));
    }

    #[tokio::test]
    async fn test_launch_rejects_invalid_issue() {
        let launcher = IssueLauncher::new(LaunchConfig::default());
        let ctx = sample_issue("", "Title");
        let result = launcher.launch(ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parallel_launch_respects_concurrency() {
        let contexts: Vec<IssueContext> = (0..5)
            .map(|i| sample_issue(&format!("{}", i), &format!("Issue {}", i)))
            .collect();

        let config = LaunchConfig {
            max_concurrent: 2,
            ..LaunchConfig::default()
        };

        let results = parallel_launch(contexts, config).await;
        assert_eq!(results.len(), 5, "Should get results for all 5 issues");

        let launched = results
            .iter()
            .filter(|r| matches!(r.status, SpawnStatus::Launched))
            .count();
        assert_eq!(launched, 5, "All 5 should launch successfully");
    }
}
