//! Pipeline engine implementation
//!
//! Core engine that orchestrates phase execution for tasks.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::super::handlers::{create_handler, PhaseError, PhaseHandler, PhaseResult};
use super::super::phases::{Phase, PhaseContext, Task, TaskStatus};
use super::super::resume::ResumeManager;
use super::super::resume::SessionSnapshot;
use super::super::snapshot_policy::{SnapshotConfig, SnapshotPolicy, SnapshotTrigger};
use super::config::{PhaseCallback, PipelineConfig, StatusCallback};
use crate::agent::AgentLoop;

/// Result of running a task through the pipeline
#[derive(Debug, Clone)]
pub struct PipelineRunResult {
    /// The final task state
    pub task: Task,
    /// Results from each phase
    pub phase_results: HashMap<Phase, PhaseResult>,
    /// Whether the pipeline completed successfully
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

impl PipelineRunResult {
    /// Create a successful result
    pub fn success(task: Task, phase_results: HashMap<Phase, PhaseResult>) -> Self {
        Self {
            task,
            phase_results,
            success: true,
            error: None,
        }
    }

    /// Create a failed result
    pub fn failure(task: Task, error: impl Into<String>) -> Self {
        Self {
            task,
            phase_results: HashMap::new(),
            success: false,
            error: Some(error.into()),
        }
    }
}

/// The main pipeline engine
pub struct PipelineEngine {
    /// Registered phase handlers
    handlers: RwLock<HashMap<Phase, Arc<dyn PhaseHandler>>>,
    /// Engine configuration
    config: PipelineConfig,
    /// Callbacks for phase completion events
    phase_callbacks: RwLock<Vec<PhaseCallback>>,
    /// Callbacks for status change events
    status_callbacks: RwLock<Vec<StatusCallback>>,
    /// Optional agent loop for executing phases
    agent: Option<Arc<AgentLoop>>,
    /// Automatic session snapshot policy
    snapshot_policy: RwLock<SnapshotPolicy>,
    /// Optional function to build a snapshot from current state
    snapshot_builder: Option<Arc<dyn Fn() -> SessionSnapshot + Send + Sync>>,
}

impl PipelineEngine {
    /// Create a new pipeline engine with default handlers
    pub fn new() -> Self {
        Self::with_resumable_snapshots(None)
    }

    /// Create a new engine with optional ResumeManager for auto-snapshots.
    ///
    /// When a `ResumeManager` is provided, the engine will automatically
    /// take session snapshots at phase boundaries and on errors.
    pub fn with_resumable_snapshots(resume_manager: Option<ResumeManager>) -> Self {
        let mut engine = Self {
            handlers: RwLock::new(HashMap::new()),
            config: PipelineConfig::default(),
            phase_callbacks: RwLock::new(Vec::new()),
            status_callbacks: RwLock::new(Vec::new()),
            agent: None,
            snapshot_policy: RwLock::new(SnapshotPolicy::new(
                SnapshotConfig::default(),
                resume_manager,
            )),
            snapshot_builder: None,
        };

        // Register default handlers
        for phase in Phase::all() {
            engine.register_handler_sync(create_handler(*phase));
        }

        engine
    }

    /// Create a new pipeline engine with custom configuration
    pub fn with_config(config: PipelineConfig) -> Self {
        let mut engine = Self::new();
        engine.config = config;
        engine
    }

    /// Create a new pipeline engine with an agent
    pub fn with_agent(agent: Arc<AgentLoop>) -> Self {
        let mut engine = Self::new();
        engine.agent = Some(agent);
        engine
    }

    /// Create a new pipeline engine with configuration and agent
    pub fn with_config_and_agent(config: PipelineConfig, agent: Arc<AgentLoop>) -> Self {
        let mut engine = Self::with_agent(agent);
        engine.config = config;
        engine
    }

    /// Create a new engine with config, agent, and resumable snapshots.
    pub fn with_config_agent_and_snapshots(
        config: PipelineConfig,
        agent: Arc<AgentLoop>,
        resume_manager: Option<ResumeManager>,
    ) -> Self {
        let mut engine = Self::with_resumable_snapshots(resume_manager);
        engine.config = config;
        engine.agent = Some(agent);
        engine
    }

    /// Set the agent for the pipeline engine
    pub fn set_agent(&mut self, agent: Arc<AgentLoop>) {
        self.agent = Some(agent);
    }

    /// Get a reference to the agent
    pub fn agent(&self) -> Option<&Arc<AgentLoop>> {
        self.agent.as_ref()
    }

    /// Set the snapshot builder function.
    ///
    /// This closure builds a SessionSnapshot from the current engine state.
    /// When a snapshot trigger fires, it calls this builder to capture
    /// the current conversation and task state.
    pub fn set_snapshot_builder(
        &mut self,
        builder: impl Fn() -> SessionSnapshot + Send + Sync + 'static,
    ) {
        self.snapshot_builder = Some(Arc::new(builder));
    }

    /// Register a phase handler
    pub async fn register_handler(&self, handler: Box<dyn PhaseHandler>) {
        let phase = handler.phase();
        let mut handlers = self.handlers.write().await;
        handlers.insert(phase, Arc::from(handler));
        debug!("Registered handler for phase: {}", phase);
    }

    /// Register a handler using a boxed implementation
    pub fn register_handler_sync(&mut self, handler: Box<dyn PhaseHandler>) {
        let phase = handler.phase();
        self.handlers.get_mut().insert(phase, Arc::from(handler));
        debug!("Registered handler for phase: {}", phase);
    }

    /// Get a handler for a phase
    pub async fn get_handler(&self, phase: Phase) -> Option<Arc<dyn PhaseHandler>> {
        let handlers = self.handlers.read().await;
        handlers.get(&phase).cloned()
    }

    /// Add a phase completion callback
    pub async fn on_phase_complete(&self, callback: PhaseCallback) {
        let mut callbacks = self.phase_callbacks.write().await;
        callbacks.push(callback);
    }

    /// Add a status change callback
    pub async fn on_status_change(&self, callback: StatusCallback) {
        let mut callbacks = self.status_callbacks.write().await;
        callbacks.push(callback);
    }

    /// Run a task through the pipeline
    pub async fn run(
        &self,
        mut task: Task,
        context: PhaseContext,
    ) -> Result<PipelineRunResult, PhaseError> {
        info!("Starting pipeline for task: {}", task.id);
        debug!(
            "Task: {} - Phase: {} - Status: {}",
            task.title, task.phase, task.status
        );

        // Validate task is in a runnable state
        if task.status.is_terminal() {
            return Err(PhaseError::InvalidTransition {
                from: task.status.to_string(),
                to: TaskStatus::InProgress.to_string(),
            });
        }

        // Mark task as in progress
        task.set_status(TaskStatus::InProgress);
        self.notify_status_change(&task).await;

        let mut phase_results = HashMap::new();

        // Execute phases from current phase to completion
        loop {
            let current_phase = task.phase;
            info!("Executing phase: {} for task: {}", current_phase, task.id);

            // Get handler for current phase
            let handler = match self.get_handler(current_phase).await {
                Some(h) => h,
                None => {
                    error!("No handler registered for phase: {}", current_phase);
                    task.set_status(TaskStatus::Failed);
                    self.notify_status_change(&task).await;
                    return Err(PhaseError::ConfigError {
                        message: format!("No handler for phase: {}", current_phase),
                    });
                }
            };

            // Execute the phase with agent if available
            let result = match handler.execute(&task, &context, self.agent.clone()).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Phase {} failed: {}", current_phase, e);

                    // Snapshot on failure for crash recovery
                    if let Some(ref builder) = self.snapshot_builder {
                        let snapshot = builder();
                        let failed_trigger =
                            SnapshotTrigger::PhaseError(current_phase, e.to_string());
                        let mut policy = self.snapshot_policy.write().await;
                        let _ = policy
                            .should_snapshot(&failed_trigger, Some(&snapshot))
                            .await;
                    }

                    // Check if we can retry
                    if task.can_retry() {
                        task.increment_retry();
                        warn!(
                            "Retrying phase {} (attempt {}/{})",
                            current_phase, task.retry_count, task.max_retries
                        );
                        continue;
                    }

                    task.set_status(TaskStatus::Failed);
                    self.notify_status_change(&task).await;
                    return Err(e);
                }
            };

            // Store the result
            phase_results.insert(current_phase, result.clone());

            // Take automatic session snapshot if configured
            if let Some(ref builder) = self.snapshot_builder {
                let snapshot = builder();
                let trigger = SnapshotTrigger::PhaseComplete(current_phase);
                let mut policy = self.snapshot_policy.write().await;
                policy.note_checkpoint(&trigger);
                if policy.should_snapshot(&trigger, Some(&snapshot)).await {
                    info!("Auto-snapshot saved after phase: {}", current_phase);
                }
            }

            // Notify callbacks
            self.notify_phase_complete(&task, current_phase, &result)
                .await;

            // If phase failed, mark task as failed
            if !result.success {
                error!("Phase {} reported failure", current_phase);
                task.set_status(TaskStatus::Failed);
                self.notify_status_change(&task).await;

                return Ok(PipelineRunResult {
                    task,
                    phase_results,
                    success: false,
                    error: result.errors.first().cloned(),
                });
            }

            // Auto-commit if configured
            if self.config.auto_commit {
                if let Some(commit_hash) = &result.commit_hash {
                    debug!("Phase {} committed: {}", current_phase, commit_hash);
                }
            }

            // Check if this is the final phase
            if current_phase.is_final() {
                info!("Pipeline completed for task: {}", task.id);
                task.set_status(TaskStatus::Completed);
                self.notify_status_change(&task).await;
                break;
            }

            // Advance to next phase
            if !task.advance_phase() {
                // Should not happen since we checked is_final(), but be safe
                warn!("Could not advance phase from {}", current_phase);
                break;
            }

            debug!("Advanced to phase: {}", task.phase);
        }

        Ok(PipelineRunResult::success(task, phase_results))
    }

    /// Run only specific phases for a task
    pub async fn run_phases(
        &self,
        mut task: Task,
        context: PhaseContext,
        phases: &[Phase],
    ) -> Result<PipelineRunResult, PhaseError> {
        info!("Running phases {:?} for task: {}", phases, task.id);

        if phases.is_empty() {
            return Ok(PipelineRunResult::success(task, HashMap::new()));
        }

        // Set task to first requested phase if needed
        if task.phase != phases[0] {
            task.set_phase(phases[0]);
        }

        task.set_status(TaskStatus::InProgress);
        self.notify_status_change(&task).await;

        let mut phase_results = HashMap::new();

        for phase in phases {
            info!("Executing phase: {} for task: {}", phase, task.id);
            task.set_phase(*phase);

            let handler = match self.get_handler(*phase).await {
                Some(h) => h,
                None => {
                    task.set_status(TaskStatus::Failed);
                    self.notify_status_change(&task).await;
                    return Err(PhaseError::ConfigError {
                        message: format!("No handler for phase: {}", phase),
                    });
                }
            };

            let result = handler.execute(&task, &context, self.agent.clone()).await?;
            phase_results.insert(*phase, result.clone());

            self.notify_phase_complete(&task, *phase, &result).await;

            if !result.success {
                task.set_status(TaskStatus::Failed);
                self.notify_status_change(&task).await;

                return Ok(PipelineRunResult {
                    task,
                    phase_results,
                    success: false,
                    error: result.errors.first().cloned(),
                });
            }
        }

        task.set_status(TaskStatus::Completed);
        self.notify_status_change(&task).await;

        Ok(PipelineRunResult::success(task, phase_results))
    }

    /// Run a task with a specific agent (one-time override)
    pub async fn run_with_agent(
        &self,
        mut task: Task,
        context: PhaseContext,
        agent: Arc<AgentLoop>,
    ) -> Result<PipelineRunResult, PhaseError> {
        info!("Starting pipeline for task: {} with custom agent", task.id);
        debug!(
            "Task: {} - Phase: {} - Status: {}",
            task.title, task.phase, task.status
        );

        // Validate task is in a runnable state
        if task.status.is_terminal() {
            return Err(PhaseError::InvalidTransition {
                from: task.status.to_string(),
                to: TaskStatus::InProgress.to_string(),
            });
        }

        // Mark task as in progress
        task.set_status(TaskStatus::InProgress);
        self.notify_status_change(&task).await;

        let mut phase_results = HashMap::new();

        // Execute phases from current phase to completion
        loop {
            let current_phase = task.phase;
            info!("Executing phase: {} for task: {}", current_phase, task.id);

            // Get handler for current phase
            let handler = match self.get_handler(current_phase).await {
                Some(h) => h,
                None => {
                    error!("No handler registered for phase: {}", current_phase);
                    task.set_status(TaskStatus::Failed);
                    self.notify_status_change(&task).await;
                    return Err(PhaseError::ConfigError {
                        message: format!("No handler for phase: {}", current_phase),
                    });
                }
            };

            // Execute the phase with the provided agent
            let result = match handler.execute(&task, &context, Some(agent.clone())).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Phase {} failed: {}", current_phase, e);

                    if task.can_retry() {
                        task.increment_retry();
                        warn!(
                            "Retrying phase {} (attempt {}/{})",
                            current_phase, task.retry_count, task.max_retries
                        );
                        continue;
                    }

                    task.set_status(TaskStatus::Failed);
                    self.notify_status_change(&task).await;
                    return Err(e);
                }
            };

            phase_results.insert(current_phase, result.clone());
            self.notify_phase_complete(&task, current_phase, &result)
                .await;

            if !result.success {
                error!("Phase {} reported failure", current_phase);
                task.set_status(TaskStatus::Failed);
                self.notify_status_change(&task).await;

                return Ok(PipelineRunResult {
                    task,
                    phase_results,
                    success: false,
                    error: result.errors.first().cloned(),
                });
            }

            if self.config.auto_commit {
                if let Some(commit_hash) = &result.commit_hash {
                    debug!("Phase {} committed: {}", current_phase, commit_hash);
                }
            }

            if current_phase.is_final() {
                info!("Pipeline completed for task: {}", task.id);
                task.set_status(TaskStatus::Completed);
                self.notify_status_change(&task).await;
                break;
            }

            if !task.advance_phase() {
                warn!("Could not advance phase from {}", current_phase);
                break;
            }

            debug!("Advanced to phase: {}", task.phase);
        }

        Ok(PipelineRunResult::success(task, phase_results))
    }

    /// Advance a task to the next phase without full execution
    pub fn advance_phase(&self, task: &mut Task) -> bool {
        let advanced = task.advance_phase();
        if advanced {
            info!("Advanced task {} to phase: {}", task.id, task.phase);
        }
        advanced
    }

    /// Get the current configuration
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: PipelineConfig) {
        self.config = config;
    }

    /// Notify phase completion callbacks
    async fn notify_phase_complete(&self, task: &Task, phase: Phase, result: &PhaseResult) {
        let callbacks = self.phase_callbacks.read().await;
        for callback in callbacks.iter() {
            callback(task, phase, result);
        }
    }

    /// Notify status change callbacks
    async fn notify_status_change(&self, task: &Task) {
        let callbacks = self.status_callbacks.read().await;
        for callback in callbacks.iter() {
            callback(task, task.status);
        }
    }
}

impl Default for PipelineEngine {
    fn default() -> Self {
        Self::new()
    }
}
