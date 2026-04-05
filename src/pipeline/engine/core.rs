//! Pipeline engine core: struct, constructors, and handler management

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::debug;

use super::super::handlers::{create_handler, PhaseHandler, PhaseResult};
use super::super::phases::{Phase, PhaseContext, Task, TaskStatus};
use super::config::{PhaseCallback, PipelineConfig, StatusCallback};
use crate::agent::AgentLoop;

/// Result of running a task through the pipeline
#[derive(Debug)]
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
    pub(super) handlers: RwLock<HashMap<Phase, Arc<dyn PhaseHandler>>>,
    /// Engine configuration
    pub(super) config: PipelineConfig,
    /// Callbacks for phase completion events
    pub(super) phase_callbacks: RwLock<Vec<PhaseCallback>>,
    /// Callbacks for status change events
    pub(super) status_callbacks: RwLock<Vec<StatusCallback>>,
    /// Optional agent loop for executing phases
    pub(super) agent: Option<Arc<AgentLoop>>,
}

impl PipelineEngine {
    /// Create a new pipeline engine with default handlers
    pub fn new() -> Self {
        let mut engine = Self {
            handlers: RwLock::new(HashMap::new()),
            config: PipelineConfig::default(),
            phase_callbacks: RwLock::new(Vec::new()),
            status_callbacks: RwLock::new(Vec::new()),
            agent: None,
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

    /// Set the agent for the pipeline engine
    pub fn set_agent(&mut self, agent: Arc<AgentLoop>) {
        self.agent = Some(agent);
    }

    /// Get a reference to the agent
    pub fn agent(&self) -> Option<&Arc<AgentLoop>> {
        self.agent.as_ref()
    }

    /// Register a phase handler (async)
    pub async fn register_handler(&self, handler: Box<dyn PhaseHandler>) {
        let phase = handler.phase();
        let mut handlers = self.handlers.write().await;
        handlers.insert(phase, Arc::from(handler));
        debug!("Registered handler for phase: {}", phase);
    }

    /// Register a handler using a boxed implementation (sync)
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

    /// Advance a task to the next phase without full execution
    pub fn advance_phase(&self, task: &mut Task) -> bool {
        let advanced = task.advance_phase();
        if advanced {
            debug!("Advanced task {} to phase: {}", task.id, task.phase);
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
    pub(super) async fn notify_phase_complete(
        &self,
        task: &Task,
        phase: Phase,
        result: &PhaseResult,
    ) {
        let callbacks = self.phase_callbacks.read().await;
        for callback in callbacks.iter() {
            callback(task, phase, result);
        }
    }

    /// Notify status change callbacks
    pub(super) async fn notify_status_change(&self, task: &Task) {
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
