//! Mock `AgentDriver` implementations used by the runner tests.
//!
//! Kept in a sibling helpers file so `runner_tests.rs` stays focused on
//! assertions and fits within the 300-line guideline.

use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::evals::environment::EvalEnvironment;
use crate::evals::metrics::AgentMetrics;
use crate::evals::runner::{AgentDriver, DriverError};
use crate::evals::task::EvalTask;

/// Driver that creates a named file and reports metrics.
pub struct CreateFileDriver {
    pub file_name: String,
    pub cost: f64,
    pub iterations: u32,
    pub tool_calls: u32,
}

#[async_trait]
impl AgentDriver for CreateFileDriver {
    async fn run(
        &self,
        _task: &EvalTask,
        env: &EvalEnvironment,
    ) -> Result<AgentMetrics, DriverError> {
        fs::write(env.workspace_path.join(&self.file_name), "done")
            .map_err(|e| DriverError::new(e.to_string()))?;
        Ok(AgentMetrics::empty()
            .with_cost(self.cost)
            .with_iterations(self.iterations)
            .with_tool_calls(self.tool_calls))
    }
}

/// Driver that always fails with a configurable message.
pub struct FailingDriver {
    pub message: String,
}

#[async_trait]
impl AgentDriver for FailingDriver {
    async fn run(
        &self,
        _task: &EvalTask,
        _env: &EvalEnvironment,
    ) -> Result<AgentMetrics, DriverError> {
        Err(DriverError::new(self.message.clone()))
    }
}

/// Driver that sleeps forever; exercised by the timeout test.
pub struct SleepingDriver;

#[async_trait]
impl AgentDriver for SleepingDriver {
    async fn run(
        &self,
        _task: &EvalTask,
        _env: &EvalEnvironment,
    ) -> Result<AgentMetrics, DriverError> {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        Ok(AgentMetrics::empty())
    }
}

/// Driver that counts invocations — confirms batch runs call it per
/// task. Also creates the artifact the default passing task expects.
pub struct CountingDriver {
    pub calls: Arc<AtomicU32>,
}

#[async_trait]
impl AgentDriver for CountingDriver {
    async fn run(
        &self,
        _task: &EvalTask,
        env: &EvalEnvironment,
    ) -> Result<AgentMetrics, DriverError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        fs::write(env.workspace_path.join("ok.txt"), "done").ok();
        Ok(AgentMetrics::empty())
    }
}
