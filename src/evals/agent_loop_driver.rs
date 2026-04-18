//! Bridge between the evaluation harness and a real
//! [`AgentLoop`](crate::agent::AgentLoop).
//!
//! Kept in a dedicated module so `runner.rs` stays agent-agnostic (the
//! trait lives there; the trait has zero `agent::` imports). Integrators
//! that want to run real agents against tasks construct this adapter;
//! tests that only care about the harness shape keep using mock drivers.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;

use super::environment::EvalEnvironment;
use super::metrics::AgentMetrics;
use super::runner::{AgentDriver, DriverError};
use super::task::EvalTask;
use crate::agent::AgentLoop;

/// Drives a shared [`AgentLoop`] across eval tasks.
///
/// Per-task behaviour:
///
/// 1. Overrides the agent's `working_dir` to the eval environment's
///    workspace so tools see a clean sandbox.
/// 2. Clears conversation history so prior tasks don't leak into the
///    current one.
/// 3. Adds the task instruction as the user message.
/// 4. Runs the agent loop and maps the result to [`AgentMetrics`].
/// 5. Treats a safety-stop (doom loop / budget exhausted) as a driver
///    error — the grader then reports it as a harness failure, not a
///    pass based on half-done work.
pub struct AgentLoopDriver {
    agent: Arc<AgentLoop>,
}

impl AgentLoopDriver {
    pub fn new(agent: Arc<AgentLoop>) -> Self {
        Self { agent }
    }

    /// The agent handle, in case callers need to subscribe to events
    /// or inspect config before the first run.
    pub fn agent(&self) -> &Arc<AgentLoop> {
        &self.agent
    }

    async fn point_at_workspace(&self, workspace: &Path) {
        let mut cfg = self.agent.config.write().await;
        cfg.working_dir = workspace.to_string_lossy().to_string();
    }
}

#[async_trait]
impl AgentDriver for AgentLoopDriver {
    async fn run(
        &self,
        task: &EvalTask,
        env: &EvalEnvironment,
    ) -> Result<AgentMetrics, DriverError> {
        self.point_at_workspace(&env.workspace_path).await;
        self.agent.clear_history().await;
        self.agent.add_user_message(&task.instruction).await;

        let result = self
            .agent
            .run()
            .await
            .map_err(|e| DriverError::new(e.to_string()))?;

        // A safety-stopped agent didn't complete its work. Surface as a
        // driver error so the grader treats it as a harness failure,
        // never a pass. Without this, a doom-loop detection after a
        // partial file write would still "pass" a file_exists grader.
        if let Some(reason) = result.safety_stop_reason() {
            return Err(DriverError::new(format!(
                "agent stopped for safety: {reason}"
            )));
        }

        Ok(AgentMetrics::empty()
            .with_iterations(result.iterations)
            .with_tool_calls(result.tool_calls))
    }
}
