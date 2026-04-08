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
use super::super::pr_lifecycle::{PrLifecycleManager, PrMetadata, PrState};
use super::super::queue::{QueueStats, TaskQueue};
use super::super::queue_manager;
use super::super::recovery_manager;
use super::super::scheduler::{self, ExecutionGuard};
use super::super::task_factory;
use super::super::timeout::TimeoutManager;
use super::super::vex_manager::VexManager;
use super::super::worker_pool::WorkerPool;
use super::config::OrchestratorConfig;
use super::reaction_bridge::{execute_outcome, ReactionBridge, ReactionOutcome};
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
    pub(crate) pr_manager: Arc<PrLifecycleManager>,
    /// PRs currently tracked for lifecycle monitoring (task_id → PrMetadata).
    pub(crate) tracked_prs: Arc<RwLock<HashMap<String, PrMetadata>>>,
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

        let pr_manager = Arc::new(PrLifecycleManager::new(
            config.github.as_ref().and_then(|g| g.repository.clone()),
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
            pr_manager,
            tracked_prs: Arc::new(RwLock::new(HashMap::new())),
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
    ///
    /// Also checks for PR URLs in task metadata and starts tracking
    /// them for lifecycle monitoring (CI, reviews, mergeability).
    pub async fn post_process_results(&self, results: &[PipelineRunResult]) {
        for result in results {
            // Check for PR URL in task metadata — track it if found
            if let Some(pr_url) = result
                .task
                .metadata
                .get("github_sync")
                .and_then(|v| v.get("pull_request_url"))
                .and_then(|v| v.as_str())
            {
                if let Some((_repo, pr_num)) = super::super::post_pr::parse_pr_url(pr_url) {
                    let mut metadata =
                        PrMetadata::new(&format!(".d3vx/worktrees/{}", result.task.id));
                    metadata.pr_number = Some(pr_num);
                    metadata.url = Some(pr_url.to_string());
                    metadata.state = PrState::Open;
                    metadata.title = result.task.title.clone();
                    self.track_pr(&result.task.id, metadata).await;
                }
            }

            if result.success {
                // Notify on successful task completion
                if let Some(ref config) = self.reaction_bridge.notify_config {
                    if config.on_task_done {
                        send_task_notification(config, &result.task.title, "completed", "success");
                    }
                }
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

    /// Track a PR for lifecycle monitoring.
    ///
    /// Call this after a PR is created. The daemon loop will periodically
    /// call `audit_active_prs` to check CI/reviews/mergeability.
    pub async fn track_pr(&self, task_id: &str, metadata: PrMetadata) {
        let mut tracked = self.tracked_prs.write().await;
        info!("Tracking PR for task {}: {:?}", task_id, metadata.state);
        tracked.insert(task_id.to_string(), metadata);
    }

    /// Audit all tracked PRs: refresh CI, reviews, mergeability.
    ///
    /// Call this from the daemon loop on each tick. For each tracked PR:
    /// - Refreshes state via `PrLifecycleManager.refresh()`
    /// - Reacts to state transitions (CI failed → reaction, approved → merge, etc.)
    /// - Removes PRs that reach terminal states (Merged, Closed)
    pub async fn audit_active_prs(&self) {
        let mut tracked = self.tracked_prs.write().await;
        if tracked.is_empty() {
            return;
        }

        let task_ids: Vec<String> = tracked.keys().cloned().collect();
        for task_id in &task_ids {
            let Some(metadata) = tracked.get_mut(task_id) else {
                continue;
            };

            if metadata.pr_number.is_none() {
                continue;
            }

            let old_state = metadata.state;

            if let Err(e) = self.pr_manager.refresh(metadata).await {
                warn!("PR refresh failed for task {}: {}", task_id, e);
                continue;
            }

            if metadata.state != old_state {
                info!(
                    "PR for task {} transitioned: {:?} → {:?}",
                    task_id, old_state, metadata.state
                );
                self.handle_pr_state_change(task_id, metadata, old_state)
                    .await;
            }

            // Remove terminal states
            if matches!(metadata.state, PrState::Merged | PrState::Closed) {
                tracked.remove(task_id);
            }
        }
    }

    /// Handle PR state transitions by triggering appropriate actions.
    async fn handle_pr_state_change(
        &self,
        task_id: &str,
        metadata: &PrMetadata,
        _old_state: PrState,
    ) {
        match metadata.state {
            PrState::CiFailed => {
                info!("PR CI failed for task {}, emitting reaction event", task_id);
                let _ = self.requeue_task(task_id).await;
            }
            PrState::ChangesRequested => {
                info!(
                    "Changes requested on PR for task {}, emitting reaction event",
                    task_id
                );
            }
            PrState::Mergeable => {
                info!("PR is mergeable for task {}, attempting merge", task_id);
                if let Err(e) = self.pr_manager.merge(&mut metadata.clone()).await {
                    warn!("Auto-merge failed for task {}: {}", task_id, e);
                }
            }
            PrState::Merged => {
                info!("PR merged for task {}, notifying", task_id);
                if let Some(ref config) = self.reaction_bridge.notify_config {
                    if config.on_mergeable {
                        send_task_notification(
                            config,
                            &format!("Task {} — PR merged", task_id),
                            "merged",
                            "success",
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

impl Default for PipelineOrchestrator {
    fn default() -> Self {
        unimplemented!("Use ::new()")
    }
}

/// Fire-and-forget task notification helper.
fn send_task_notification(
    config: &crate::config::NotificationsConfig,
    task_title: &str,
    status: &str,
    type_name: &str,
) {
    use crate::utils::notify::{self, NotificationOptions};
    let opts = NotificationOptions {
        title: format!("d3vx: Task {}", status),
        body: task_title.to_string(),
        type_name: type_name.to_string(),
    };
    let config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = notify::notify(opts, &config).await {
            warn!("Task notification failed: {}", e);
        }
    });
}
