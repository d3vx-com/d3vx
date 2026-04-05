//! Orchestrator configuration and trait definitions

use std::path::{Path, PathBuf};

use anyhow::Result;

use super::super::phases::{Priority, Task, TaskStatus};
use super::super::vex_manager::VexTaskHandle;

// ============================================================================
// TRAIT DEFINITIONS
// ============================================================================

#[async_trait::async_trait]
pub trait TaskAuthority: Send + Sync {
    async fn create_task_from_chat(
        &self,
        title: &str,
        instruction: &str,
        priority: Option<Priority>,
    ) -> Result<Task>;
    async fn create_task_from_github_issue(
        &self,
        number: u64,
        repository: &str,
        author: &str,
        title: &str,
        body: &str,
    ) -> Result<Task>;
    async fn create_task_from_pr_comment(
        &self,
        pr_number: u64,
        comment_id: u64,
        repository: &str,
        author: &str,
        comment: &str,
    ) -> Result<Task>;
    async fn create_task_from_ci_failure(
        &self,
        pipeline_id: &str,
        branch: &str,
        commit_sha: &str,
        error_details: &str,
    ) -> Result<Task>;
    async fn create_task_from_automation(
        &self,
        automation_type: &str,
        trigger_id: &str,
        title: &str,
        instruction: &str,
    ) -> Result<Task>;
    async fn create_task_from_slash_command(
        &self,
        command: &str,
        args: Vec<String>,
        instruction: &str,
    ) -> Result<Task>;
    async fn create_vex_task(
        &self,
        description: &str,
        project_path: &str,
        branch: Option<&str>,
    ) -> Result<VexTaskHandle>;
    async fn transition_task(&self, task_id: &str, new_status: TaskStatus) -> Result<Task>;
    async fn cancel_task(&self, task_id: &str) -> Result<()>;
    async fn get_task(&self, task_id: &str) -> Option<Task>;
}

// ============================================================================
// CONFIGURATION
// ============================================================================

#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub pipeline: super::super::engine::PipelineConfig,
    pub cost_tracker: super::super::cost_tracker::CostTrackerConfig,
    pub timeout: super::super::timeout::TimeoutConfig,
    pub checkpoint_dir: PathBuf,
    pub max_concurrent_tasks: usize,
    pub enable_auto_recovery: bool,
    pub task_id_prefix: String,
    pub worker_pool: super::super::worker_pool::WorkerPoolConfig,
    pub classifier: super::super::classifier::ClassifierConfig,
    pub github: Option<crate::config::GitHubIntegration>,
    pub max_parallel: usize,
    pub subagent: crate::config::types::SubAgentConfig,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            pipeline: Default::default(),
            cost_tracker: Default::default(),
            timeout: Default::default(),
            checkpoint_dir: PathBuf::from(".d3vx/checkpoints"),
            max_concurrent_tasks: 3,
            enable_auto_recovery: true,
            task_id_prefix: "TASK".to_string(),
            worker_pool: Default::default(),
            classifier: Default::default(),
            github: None,
            max_parallel: 4,
            subagent: Default::default(),
        }
    }
}

impl OrchestratorConfig {
    pub fn with_checkpoint_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.checkpoint_dir = dir.as_ref().to_path_buf();
        self
    }
    pub fn with_max_concurrent_tasks(mut self, max: usize) -> Self {
        self.max_concurrent_tasks = max;
        self
    }
    pub fn without_auto_recovery(mut self) -> Self {
        self.enable_auto_recovery = false;
        self
    }
    pub fn with_task_id_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.task_id_prefix = prefix.into();
        self
    }
    pub fn with_github(mut self, github: Option<crate::config::GitHubIntegration>) -> Self {
        self.github = github;
        self
    }
}
