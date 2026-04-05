//! Intake types
//!
//! Defines the `TaskSource` and `TaskIntakeInput` types used for normalizing
//! various trigger sources into consistent task records.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::super::phases::{Phase, Priority};

/// Source of a task trigger
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskSource {
    /// Interactive chat message
    Chat,
    /// GitHub issue
    GitHubIssue {
        /// Issue number
        number: u64,
        /// Repository (owner/repo)
        repository: String,
        /// Author
        author: String,
    },
    /// GitHub PR comment
    GitHubPRComment {
        /// PR number
        pr_number: u64,
        /// Comment ID
        comment_id: u64,
        /// Repository
        repository: String,
        /// Author
        author: String,
    },
    /// CI failure trigger
    CIFailure {
        /// Pipeline ID
        pipeline_id: String,
        /// Branch where failure occurred
        branch: String,
        /// Commit SHA
        commit_sha: String,
    },
    /// Vex autonomous execution task
    Vex {
        /// Project path where vex was triggered
        project_path: String,
        /// Optional branch name for the worktree
        branch: Option<String>,
    },
    /// Automated trigger (scheduled, webhook, etc.)
    Automation {
        /// Automation type
        automation_type: String,
        /// Trigger identifier
        trigger_id: String,
    },
    /// Slash command from CLI
    SlashCommand {
        /// Command name (e.g., "/implement")
        command: String,
        /// Arguments provided
        args: Vec<String>,
    },
    /// Direct API call
    Direct,
}

impl TaskSource {
    /// Get a human-readable label for this source
    pub fn label(&self) -> String {
        match self {
            TaskSource::Chat => "Chat".to_string(),
            TaskSource::GitHubIssue {
                number, repository, ..
            } => {
                format!("GitHub Issue #{} ({})", number, repository)
            }
            TaskSource::GitHubPRComment {
                pr_number,
                repository,
                ..
            } => {
                format!("PR #{} Comment ({})", pr_number, repository)
            }
            TaskSource::CIFailure {
                pipeline_id,
                branch,
                ..
            } => {
                format!("CI Failure ({}/{})", pipeline_id, branch)
            }
            TaskSource::Vex { project_path, .. } => {
                format!("Vex ({})", project_path)
            }
            TaskSource::Automation {
                automation_type, ..
            } => {
                format!("Automation ({})", automation_type)
            }
            TaskSource::SlashCommand { command, .. } => {
                format!("Slash Command ({})", command)
            }
            TaskSource::Direct => "Direct".to_string(),
        }
    }

    /// Check if this source supports priority inference
    pub fn supports_priority_inference(&self) -> bool {
        matches!(
            self,
            TaskSource::GitHubIssue { .. } | TaskSource::CIFailure { .. }
        )
    }

    /// Check if this source requires external validation
    pub fn requires_external_validation(&self) -> bool {
        matches!(
            self,
            TaskSource::GitHubIssue { .. }
                | TaskSource::GitHubPRComment { .. }
                | TaskSource::CIFailure { .. }
        )
    }
}

/// Raw input for creating a task from various sources
#[derive(Debug, Clone)]
pub struct TaskIntakeInput {
    /// Source of the task
    pub source: TaskSource,
    /// Task title/summary
    pub title: String,
    /// Detailed instruction
    pub instruction: String,
    /// Optional priority override
    pub priority: Option<Priority>,
    /// Optional initial phase
    pub initial_phase: Option<Phase>,
    /// Optional metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Dependencies on other tasks
    pub depends_on: Vec<String>,
}

impl TaskIntakeInput {
    /// Create a new intake input from a chat message
    pub fn from_chat(title: impl Into<String>, instruction: impl Into<String>) -> Self {
        Self {
            source: TaskSource::Chat,
            title: title.into(),
            instruction: instruction.into(),
            priority: None,
            initial_phase: None,
            metadata: HashMap::new(),
            tags: Vec::new(),
            depends_on: Vec::new(),
        }
    }

    /// Create intake from a GitHub issue
    pub fn from_github_issue(
        number: u64,
        repository: impl Into<String>,
        author: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            source: TaskSource::GitHubIssue {
                number,
                repository: repository.into(),
                author: author.into(),
            },
            title: format!("Issue #{}: {}", number, title.into()),
            instruction: body.into(),
            priority: None,
            initial_phase: None,
            metadata: HashMap::new(),
            tags: vec!["github".to_string(), "issue".to_string()],
            depends_on: Vec::new(),
        }
    }

    /// Create intake from a PR comment
    pub fn from_pr_comment(
        pr_number: u64,
        comment_id: u64,
        repository: impl Into<String>,
        author: impl Into<String>,
        comment: impl Into<String>,
    ) -> Self {
        Self {
            source: TaskSource::GitHubPRComment {
                pr_number,
                comment_id,
                repository: repository.into(),
                author: author.into(),
            },
            title: format!("PR #{} Review Request", pr_number),
            instruction: comment.into(),
            priority: Some(Priority::High),
            initial_phase: None,
            metadata: HashMap::new(),
            tags: vec!["github".to_string(), "pr-review".to_string()],
            depends_on: Vec::new(),
        }
    }

    /// Create intake from CI failure
    pub fn from_ci_failure(
        pipeline_id: impl Into<String>,
        branch: impl Into<String>,
        commit_sha: impl Into<String>,
        error_details: impl Into<String>,
    ) -> Self {
        Self {
            source: TaskSource::CIFailure {
                pipeline_id: pipeline_id.into(),
                branch: branch.into(),
                commit_sha: commit_sha.into(),
            },
            title: "CI Failure Investigation".to_string(),
            instruction: error_details.into(),
            priority: Some(Priority::Critical),
            initial_phase: Some(Phase::Research),
            metadata: HashMap::new(),
            tags: vec!["ci".to_string(), "bugfix".to_string()],
            depends_on: Vec::new(),
        }
    }

    /// Create intake from automation
    pub fn from_automation(
        automation_type: impl Into<String>,
        trigger_id: impl Into<String>,
        title: impl Into<String>,
        instruction: impl Into<String>,
    ) -> Self {
        Self {
            source: TaskSource::Automation {
                automation_type: automation_type.into(),
                trigger_id: trigger_id.into(),
            },
            title: title.into(),
            instruction: instruction.into(),
            priority: None,
            initial_phase: None,
            metadata: HashMap::new(),
            tags: vec!["automation".to_string()],
            depends_on: Vec::new(),
        }
    }

    /// Create intake from slash command
    pub fn from_slash_command(
        command: impl Into<String>,
        args: Vec<String>,
        instruction: impl Into<String>,
    ) -> Self {
        let cmd = command.into();
        Self {
            source: TaskSource::SlashCommand {
                command: cmd.clone(),
                args: args.clone(),
            },
            title: format!("{} {}", cmd, args.join(" ")).trim().to_string(),
            instruction: instruction.into(),
            priority: None,
            initial_phase: None,
            metadata: HashMap::new(),
            tags: vec!["slash-command".to_string()],
            depends_on: Vec::new(),
        }
    }

    /// Create intake from Vex autonomous execution trigger
    pub fn from_vex(
        description: impl Into<String>,
        project_path: impl Into<String>,
        branch: Option<String>,
    ) -> Self {
        Self {
            source: TaskSource::Vex {
                project_path: project_path.into(),
                branch: branch.clone(),
            },
            title: description.into(),
            instruction: String::new(), // Vex tasks get instruction from classifier
            priority: Some(Priority::High), // Vex tasks are typically high priority
            initial_phase: Some(Phase::Plan),
            metadata: HashMap::new(),
            tags: vec!["vex".to_string(), "autonomous".to_string()],
            depends_on: Vec::new(),
        }
    }

    /// Set priority override
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Set initial phase
    pub fn with_initial_phase(mut self, phase: Phase) -> Self {
        self.initial_phase = Some(phase);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Add dependencies
    pub fn with_dependencies(mut self, depends_on: Vec<String>) -> Self {
        self.depends_on = depends_on;
        self
    }
}
