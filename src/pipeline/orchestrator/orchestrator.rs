//! Pipeline orchestrator struct and main implementation

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::super::checkpoint::CheckpointManager;
use super::super::classifier::ExecutionClassifier;
use super::super::engine::{PipelineEngine, PipelineRunResult};
use super::super::github;
use super::super::intake::TaskIntake;
use super::super::metrics;
use super::super::ownership::OwnershipManager;
use super::super::phases::{PhaseContext, Task, TaskStatus};
use super::super::queue::{QueueStats, TaskQueue};
use super::super::queue_manager;
use super::super::recovery_manager;
use super::super::scheduler::{self, ExecutionGuard};
use super::super::task_factory;
use super::super::timeout::TimeoutManager;
use super::super::vex_manager::VexManager;
use super::super::worker_pool::WorkerPool;
use super::config::OrchestratorConfig;
use super::reaction_bridge::{ReactionBridge, ReactionOutcome, execute_outcome};
use crate::agent::{AgentLoop, SubAgentManager};

pub struct PipelineOrchestrator {
    config: OrchestratorConfig,
    pub(crate) engine: Arc<PipelineEngine>,
    pub(crate) queue: Arc<TaskQueue>,
    pub(crate) metrics: Arc<metrics::MetricsCollector>,
    pub(crate) checkpoint_manager: Arc<CheckpointManager>,
    pub(crate) active_tasks: Arc<RwLock<HashMap<String, String>>>,
    pub(crate) agent: RwLock<Option<Arc<AgentLoop>>>,
    pub(crate) worker_pool: Arc<WorkerPool>,
    pub(crate) scheduler: Arc<scheduler::TaskScheduler>,
    pub(crate) queue_manager: Arc<queue_manager::QueueManager>,
    pub(crate) task_factory: Arc<task_factory::TaskFactory>,
    pub(crate) recovery_manager: Arc<recovery_manager::RecoveryManager>,
    pub(crate) github_manager: Arc<github::GitHubManager>,
    pub(crate) vex_manager: Arc<VexManager>,
    pub(crate) subagent_manager: Arc<SubAgentManager>,
    pub(crate) ownership_manager: Arc<OwnershipManager>,
    pub(crate) reaction_bridge: Arc<ReactionBridge>,
}

impl PipelineOrchestrator {
    pub async fn new(
        config: OrchestratorConfig,
        db: Option<crate::store::database::DatabaseHandle>,
    ) -> Result<Self> {
        let checkpoint_manager = Arc::new(CheckpointManager::new(&config.checkpoint_dir));
        checkpoint_manager.initialize().await?;

        let engine = Arc::new(PipelineEngine::with_config(config.pipeline.clone()));
        let queue = Arc::new(TaskQueue::with_orchestrator_enforcement());
        let metrics = Arc::new(metrics::MetricsCollector::new(config.cost_tracker.clone()));
        let worker_pool = Arc::new(WorkerPool::new(config.worker_pool.clone()));
        worker_pool.add_worker("default-worker").await?;

        let active_tasks = Arc::new(RwLock::new(HashMap::new()));
        let active_leases = Arc::new(RwLock::new(HashMap::new()));
        let ownership_manager = Arc::new(OwnershipManager::new());

        let queue_manager = Arc::new(queue_manager::QueueManager::with_ownership(
            queue.clone(),
            checkpoint_manager.clone(),
            worker_pool.clone(),
            active_tasks.clone(),
            active_leases.clone(),
            ownership_manager.clone(),
        ));

        let scheduler = Arc::new(scheduler::TaskScheduler::new(
            worker_pool.clone(),
            queue.clone(),
            engine.clone(),
            checkpoint_manager.clone(),
            active_tasks.clone(),
        ));

        let task_factory = Arc::new(task_factory::TaskFactory::new(
            Arc::new(TaskIntake::new(&config.task_id_prefix)),
            Arc::new(ExecutionClassifier::with_config(config.classifier.clone())),
            checkpoint_manager.clone(),
            queue.clone(),
            active_tasks.clone(),
        ));

        let crash_detector = Arc::new(crate::recovery::CrashDetector {
            check_interval: Duration::from_secs(30 * 60),
            max_idle_time: Duration::from_secs(60 * 60),
        });

        let recovery_manager = Arc::new(recovery_manager::RecoveryManager::new(
            active_tasks.clone(),
            queue_manager.clone(),
            crash_detector,
            db,
        ));

        let github_manager = Arc::new(github::GitHubManager::new(task_factory.clone()));
        let vex_manager = Arc::new(VexManager::new(
            active_tasks.clone(),
            task_factory.clone(),
            queue_manager.clone(),
            scheduler.clone(),
            queue.clone(),
        ));

        let subagent_manager = Arc::new(SubAgentManager::new());
        SubAgentManager::start_cleanup_task(
            subagent_manager.clone(),
            config.subagent.cleanup.clone(),
        );

        let reaction_bridge = Arc::new(ReactionBridge::new(
            super::super::reaction::ReactionConfig::default(),
        ));

        Ok(Self {
            config,
            engine,
            queue,
            metrics,
            checkpoint_manager,
            active_tasks,
            agent: RwLock::new(None),
            worker_pool,
            scheduler,
            queue_manager,
            task_factory,
            recovery_manager,
            github_manager,
            vex_manager,
            subagent_manager,
            ownership_manager,
            reaction_bridge,
        })
    }

    pub async fn set_agent(&self, agent: Arc<AgentLoop>) {
        *self.agent.write().await = Some(agent);
    }

    pub async fn add_worker(&self, name: &str) -> Result<()> {
        self.worker_pool
            .add_worker(name)
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub async fn start_crash_watchdog(self: Arc<Self>) {
        self.recovery_manager.clone().start_watchdog().await;
    }

    // Getters
    pub fn task_factory(&self) -> Arc<task_factory::TaskFactory> {
        self.task_factory.clone()
    }
    pub fn queue_manager(&self) -> Arc<queue_manager::QueueManager> {
        self.queue_manager.clone()
    }
    pub fn scheduler(&self) -> Arc<scheduler::TaskScheduler> {
        self.scheduler.clone()
    }
    pub fn github_manager(&self) -> Arc<github::GitHubManager> {
        self.github_manager.clone()
    }
    pub fn vex_manager(&self) -> Arc<VexManager> {
        self.vex_manager.clone()
    }
    pub fn subagent_manager(&self) -> Arc<SubAgentManager> {
        self.subagent_manager.clone()
    }
    pub fn checkpoint_manager(&self) -> Arc<CheckpointManager> {
        self.checkpoint_manager.clone()
    }
    pub fn metrics(&self) -> Arc<metrics::MetricsCollector> {
        self.metrics.clone()
    }
    pub fn intake(&self) -> Arc<TaskIntake> {
        self.task_factory.intake()
    }
    pub fn classifier(&self) -> Arc<ExecutionClassifier> {
        self.task_factory.classifier()
    }
    pub fn worker_pool(&self) -> Arc<WorkerPool> {
        self.worker_pool.clone()
    }
    pub fn ownership_manager(&self) -> Arc<OwnershipManager> {
        self.ownership_manager.clone()
    }
    pub fn engine(&self) -> Arc<PipelineEngine> {
        self.engine.clone()
    }
    pub fn queue(&self) -> Arc<TaskQueue> {
        self.queue.clone()
    }

    // Task Execution logic delegating to managers
    pub async fn execute_task(
        &self,
        task: Task,
        context: PhaseContext,
    ) -> Result<PipelineRunResult> {
        info!("Executing task: {}", task.id);
        let lease = self
            .worker_pool
            .acquire_worker(&task.id)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        let _guard = ExecutionGuard::new(self.worker_pool.clone(), task.id.clone(), lease);

        self.active_tasks
            .write()
            .await
            .insert(task.id.clone(), context.worktree_path.clone());
        self.queue
            .update_status(&task.id, TaskStatus::InProgress)
            .await?;

        let checkpoint = match self.checkpoint_manager.load_checkpoint(&task.id).await? {
            Some(cp) => cp,
            None => {
                self.checkpoint_manager
                    .create_checkpoint(task.clone())
                    .await?
            }
        };

        let mut timeout_manager = TimeoutManager::with_config(self.config.timeout.clone());
        let agent_opt = self.agent.read().await.clone();

        let result = if let Some(agent) = agent_opt {
            let result_future = async {
                self.engine
                    .run_with_agent(task.clone(), context, agent)
                    .await
                    .map_err(|e| super::super::handlers::PhaseError::Other(e.to_string()))
            };
            timeout_manager
                .execute_with_timeout(task.phase, result_future)
                .await
                .map_err(|e| anyhow::anyhow!(e))?
        } else {
            warn!("No agent configured - dry-run");
            self.engine
                .run(task, context)
                .await
                .map_err(|e| anyhow::anyhow!(e))?
        };

        let mut checkpoint = checkpoint;
        for (phase, phase_result) in &result.phase_results {
            checkpoint.add_phase_result(*phase, phase_result.clone());
        }
        self.checkpoint_manager
            .update_checkpoint(&checkpoint)
            .await?;
        self.metrics.record_run_result(&result).await?;

        Ok(result)
    }

    pub async fn dispatch_tasks_parallel(
        &self,
        max_parallel: usize,
    ) -> Result<Vec<PipelineRunResult>> {
        self.scheduler
            .dispatch_parallel(
                max_parallel,
                self.config.github.clone(),
                self.agent.read().await.clone(),
            )
            .await
    }

    // Statistics
    pub async fn queue_stats(&self) -> QueueStats {
        self.queue.stats().await
    }
    pub async fn worker_pool_stats(&self) -> super::super::worker_pool::WorkerPoolStats {
        self.worker_pool.stats().await
    }
    pub async fn cost_stats(&self) -> super::super::cost_tracker::CostStats {
        self.metrics.get_stats().await
    }
    pub async fn active_tasks_list(&self) -> Vec<(String, String)> {
        self.active_tasks
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub async fn pause_all(&self) -> Result<()> {
        let active = self.active_tasks.read().await.clone();
        for (id, _) in active {
            self.queue.update_status(&id, TaskStatus::Queued).await?;
        }
        Ok(())
    }

    pub async fn recover_interrupted_tasks(&self) -> Result<Vec<Task>> {
        self.queue_manager
            .recover_interrupted_tasks(self.config.enable_auto_recovery)
            .await
    }

    pub async fn get_next_task(&self) -> Option<Task> {
        self.queue.get_next().await
    }

    pub async fn process_github_event(&self, event: github::GitHubEvent) -> Result<Option<Task>> {
        self.github_manager.process_event(event).await
    }
    pub async fn start_github_poller(&self, config: github::GitHubConfig) -> Result<()> {
        self.github_manager.start_poller(config).await
    }

    pub async fn patch_task_metadata(
        &self,
        task_id: &str,
        patch: serde_json::Value,
    ) -> Result<Task> {
        self.queue_manager.patch_task_metadata(task_id, patch).await
    }

    /// Run reaction engine on pipeline results.
    ///
    /// Call this after `dispatch_tasks_parallel` returns. It converts
    /// failed tasks into `ReactionEvent`s and executes the outcomes
    /// (re-queue, cancel, escalate, etc.).
    pub async fn post_process_results(&self, results: &[PipelineRunResult]) {
        for result in results {
            if result.success {
                continue;
            }
            let outcome = self.reaction_bridge.on_task_completed(result).await;
            if !matches!(outcome, ReactionOutcome::NoAction) {
                execute_outcome(self, &outcome).await;
            }
        }
    }

    /// Re-queue a failed task for another attempt.
    pub async fn requeue_task(&self, task_id: &str) -> Result<()> {
        self.queue
            .update_status(task_id, TaskStatus::Queued)
            .await
            .map_err(|e| anyhow::anyhow!("re-queue failed: {}", e))?;
        Ok(())
    }

    /// Cancel a task by moving it to failed.
    pub async fn cancel_task(&self, task_id: &str) -> Result<()> {
        self.queue
            .update_status(task_id, TaskStatus::Failed)
            .await
            .map_err(|e| anyhow::anyhow!("cancel failed: {}", e))?;
        Ok(())
    }
}

impl Default for PipelineOrchestrator {
    fn default() -> Self {
        unimplemented!("Use ::new()")
    }
}
