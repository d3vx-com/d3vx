//! Task Factory Module
//!
//! Handles task creation, normalization, and classification.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::checkpoint::CheckpointManager;
use super::classifier::ExecutionClassifier;
use super::intake::{TaskIntake, TaskIntakeInput};
use super::phases::{Priority, Task};
use super::queue::TaskQueue;
use super::vex_manager::VexTaskHandle;

pub struct TaskFactory {
    intake: Arc<TaskIntake>,
    classifier: Arc<ExecutionClassifier>,
    checkpoint_manager: Arc<CheckpointManager>,
    queue: Arc<TaskQueue>,
    active_tasks: Arc<RwLock<HashMap<String, String>>>,
}

impl TaskFactory {
    pub fn new(
        intake: Arc<TaskIntake>,
        classifier: Arc<ExecutionClassifier>,
        checkpoint_manager: Arc<CheckpointManager>,
        queue: Arc<TaskQueue>,
        active_tasks: Arc<RwLock<HashMap<String, String>>>,
    ) -> Self {
        Self {
            intake,
            classifier,
            checkpoint_manager,
            queue,
            active_tasks,
        }
    }

    pub fn intake(&self) -> Arc<TaskIntake> {
        self.intake.clone()
    }

    pub fn classifier(&self) -> Arc<ExecutionClassifier> {
        self.classifier.clone()
    }

    pub async fn create_from_chat(
        &self,
        title: &str,
        instruction: &str,
        priority: Option<Priority>,
    ) -> Result<Task> {
        let input = TaskIntakeInput::from_chat(title, instruction);
        let input = if let Some(p) = priority {
            input.with_priority(p)
        } else {
            input
        };
        self.create_from_intake(input).await
    }

    pub async fn create_from_github_issue(
        &self,
        number: u64,
        repository: &str,
        author: &str,
        title: &str,
        body: &str,
    ) -> Result<Task> {
        let input = TaskIntakeInput::from_github_issue(number, repository, author, title, body);
        self.create_from_intake(input).await
    }

    pub async fn create_from_pr_comment(
        &self,
        pr_number: u64,
        comment_id: u64,
        repository: &str,
        author: &str,
        comment: &str,
    ) -> Result<Task> {
        let input =
            TaskIntakeInput::from_pr_comment(pr_number, comment_id, repository, author, comment);
        self.create_from_intake(input).await
    }

    pub async fn create_from_ci_failure(
        &self,
        pipeline_id: &str,
        branch: &str,
        commit_sha: &str,
        error_details: &str,
    ) -> Result<Task> {
        let input =
            TaskIntakeInput::from_ci_failure(pipeline_id, branch, commit_sha, error_details);
        self.create_from_intake(input).await
    }

    pub async fn create_from_automation(
        &self,
        automation_type: &str,
        trigger_id: &str,
        title: &str,
        instruction: &str,
    ) -> Result<Task> {
        let input =
            TaskIntakeInput::from_automation(automation_type, trigger_id, title, instruction);
        self.create_from_intake(input).await
    }

    pub async fn create_from_slash_command(
        &self,
        command: &str,
        args: Vec<String>,
        instruction: &str,
    ) -> Result<Task> {
        let input = TaskIntakeInput::from_slash_command(command, args, instruction);
        self.create_from_intake(input).await
    }

    pub async fn create_vex_task(
        &self,
        description: &str,
        project_path: &str,
        branch: Option<&str>,
    ) -> Result<VexTaskHandle> {
        let input =
            TaskIntakeInput::from_vex(description, project_path, branch.map(|s| s.to_string()));
        let task = self.create_from_intake(input).await?;
        let workspace_id = task.id.clone();
        let worktree_path = PathBuf::from(project_path)
            .join(".d3vx")
            .join(&workspace_id.replace("TASK-", "vex-"));

        self.active_tasks.write().await.insert(
            workspace_id.clone(),
            worktree_path.to_string_lossy().to_string(),
        );
        info!(
            "Created Vex task {} with workspace at {}",
            task.id,
            worktree_path.display()
        );

        Ok(VexTaskHandle {
            task_id: task.id,
            workspace_id,
            worktree_path,
        })
    }

    pub async fn create_from_intake(&self, input: TaskIntakeInput) -> Result<Task> {
        let warnings = self.intake.validate_intake(&input)?;
        for warning in &warnings {
            warn!("Intake validation warning: {}", warning);
        }

        let task = self.intake.normalize_to_task(input)?;
        let classification = self.classifier.classify(&task)?;
        info!(
            "Task {} classified as: {} - {}",
            task.id, classification.mode, classification.reasoning
        );

        let mut task = task;
        if let serde_json::Value::Object(ref mut map) = task.metadata {
            map.insert(
                "classification".to_string(),
                serde_json::to_value(&classification)?,
            );
        }

        self.checkpoint_manager
            .create_checkpoint(task.clone())
            .await?;

        if classification.mode.requires_task_record() {
            self.queue.add_task(task.clone()).await?;
        }

        info!("Created task: {} - {}", task.id, task.title);
        Ok(task)
    }
}
