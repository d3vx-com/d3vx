//! TaskAuthority trait definition and implementation for PipelineOrchestrator

use anyhow::Result;

use super::super::phases::{Priority, Task, TaskStatus};
use super::super::vex_manager::VexTaskHandle;
use super::orchestrator::PipelineOrchestrator;

/// Trait definition for task authority operations
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

#[async_trait::async_trait]
impl TaskAuthority for PipelineOrchestrator {
    async fn create_task_from_chat(&self, t: &str, i: &str, p: Option<Priority>) -> Result<Task> {
        self.task_factory.create_from_chat(t, i, p).await
    }
    async fn create_task_from_github_issue(
        &self,
        n: u64,
        r: &str,
        a: &str,
        t: &str,
        b: &str,
    ) -> Result<Task> {
        self.task_factory
            .create_from_github_issue(n, r, a, t, b)
            .await
    }
    async fn create_task_from_pr_comment(
        &self,
        pr: u64,
        cid: u64,
        r: &str,
        a: &str,
        c: &str,
    ) -> Result<Task> {
        self.task_factory
            .create_from_pr_comment(pr, cid, r, a, c)
            .await
    }
    async fn create_task_from_ci_failure(
        &self,
        pid: &str,
        b: &str,
        s: &str,
        e: &str,
    ) -> Result<Task> {
        self.task_factory.create_from_ci_failure(pid, b, s, e).await
    }
    async fn create_task_from_automation(
        &self,
        at: &str,
        tr: &str,
        t: &str,
        i: &str,
    ) -> Result<Task> {
        self.task_factory.create_from_automation(at, tr, t, i).await
    }
    async fn create_task_from_slash_command(
        &self,
        c: &str,
        a: Vec<String>,
        i: &str,
    ) -> Result<Task> {
        self.task_factory.create_from_slash_command(c, a, i).await
    }
    async fn create_vex_task(&self, d: &str, p: &str, b: Option<&str>) -> Result<VexTaskHandle> {
        self.vex_manager.create_task(d, p, b).await
    }
    async fn transition_task(&self, id: &str, s: TaskStatus) -> Result<Task> {
        self.queue_manager.transition_task(id, s).await
    }
    async fn cancel_task(&self, id: &str) -> Result<()> {
        self.queue_manager.cancel_task(id).await
    }
    async fn get_task(&self, id: &str) -> Option<Task> {
        self.queue_manager.get_task(id).await
    }
}
