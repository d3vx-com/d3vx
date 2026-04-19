//! End-to-end eval execution: provision → run driver → grade → report.
//!
//! Stays runtime-agnostic via the [`AgentDriver`] trait. Tests use a
//! mock driver; production integrations pass a driver that wraps the
//! real agent loop. The runner itself knows nothing about LLM providers
//! or agent internals — it measures wall-clock, enforces an optional
//! per-task timeout, and records whatever metrics the driver surfaces.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::time::timeout;

use super::environment::EvalEnvironment;
use super::grader::GradeOutcome;
use super::metrics::AgentMetrics;
use super::result::{EvalReport, EvalResult};
use super::task::EvalTask;

/// A driver that runs an agent against a provisioned eval workspace.
///
/// Implementations are expected to point the agent at
/// `env.workspace_path` as its working directory and use
/// `task.instruction` as the prompt. Everything else — provider choice,
/// tool registration, streaming — is up to the driver.
#[async_trait]
pub trait AgentDriver: Send + Sync {
    /// Execute the task in the given environment. Must return only when
    /// the agent has finished (or hit an internal terminal state). The
    /// runner enforces any wall-clock timeout externally.
    async fn run(
        &self,
        task: &EvalTask,
        env: &EvalEnvironment,
    ) -> Result<AgentMetrics, DriverError>;
}

/// Error reported by a driver — opaque to the runner, surfaced to the
/// grader as a harness-level failure.
#[derive(Debug)]
pub struct DriverError {
    pub message: String,
}

impl DriverError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DriverError {}

/// Runs eval tasks against a driver, grading each and producing a report.
pub struct EvalRunner {
    workspace_root: PathBuf,
    keep_on_failure: bool,
}

impl EvalRunner {
    /// Create a runner that provisions environments under
    /// `workspace_root`. Passing failure workspaces are cleaned up by
    /// default; set [`EvalRunner::keep_on_failure`] so operators can
    /// inspect what went wrong.
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            keep_on_failure: true,
        }
    }

    /// Pass `false` to delete workspaces even when the task fails.
    /// Default is `true` — operators usually need the evidence.
    pub fn keep_on_failure(mut self, keep: bool) -> Self {
        self.keep_on_failure = keep;
        self
    }

    /// Run a single task end-to-end.
    pub async fn run<D: AgentDriver>(&self, task: &EvalTask, driver: &D) -> EvalResult {
        let started = Instant::now();

        // 1. Provision
        let env = match EvalEnvironment::provision(task, &self.workspace_root) {
            Ok(e) => e,
            Err(e) => {
                return EvalResult::harness_failure(
                    &task.id,
                    task.display_name(),
                    format!("environment provisioning failed: {e}"),
                    started.elapsed().as_millis() as u64,
                );
            }
        };

        // 2. Run driver (with optional wall-clock timeout)
        let metrics = match run_with_optional_timeout(driver, task, &env).await {
            Ok(m) => m,
            Err(fail) => {
                let r = EvalResult::harness_failure(
                    &task.id,
                    task.display_name(),
                    fail,
                    started.elapsed().as_millis() as u64,
                );
                self.cleanup(env, false);
                return r;
            }
        };

        // 3. Grade
        let outcomes: Vec<GradeOutcome> =
            task.graders.iter().map(|g| g.grade(&env)).collect();
        let duration_ms = started.elapsed().as_millis() as u64;
        let mut result =
            EvalResult::success(&task.id, task.display_name(), outcomes, duration_ms);
        if let Some(c) = metrics.cost_usd {
            result = result.with_cost(c);
        }
        if let Some(i) = metrics.iterations {
            result = result.with_iterations(i);
        }
        if let Some(t) = metrics.tool_calls {
            result = result.with_tool_calls(t);
        }

        // 4. Cleanup if desired
        self.cleanup(env, result.passed);
        result
    }

    /// Run a batch of tasks sequentially. For parallel runs, callers
    /// spawn their own tokio tasks around `run` — the runner itself
    /// makes no parallelism assumptions.
    pub async fn run_batch<D: AgentDriver>(
        &self,
        tasks: &[EvalTask],
        driver: &D,
    ) -> EvalReport {
        let mut results = Vec::with_capacity(tasks.len());
        for task in tasks {
            results.push(self.run(task, driver).await);
        }
        EvalReport::new(results)
    }

    fn cleanup(&self, env: EvalEnvironment, passed: bool) {
        if passed || !self.keep_on_failure {
            let _ = env.cleanup();
        }
    }
}

async fn run_with_optional_timeout<D: AgentDriver>(
    driver: &D,
    task: &EvalTask,
    env: &EvalEnvironment,
) -> Result<AgentMetrics, String> {
    let fut = driver.run(task, env);
    match task.timeout_secs {
        Some(secs) => match timeout(Duration::from_secs(secs), fut).await {
            Ok(Ok(m)) => Ok(m),
            Ok(Err(e)) => Err(format!("driver error: {e}")),
            Err(_) => Err(format!("task timed out after {secs}s")),
        },
        None => fut
            .await
            .map_err(|e| format!("driver error: {e}")),
    }
}
