//! SDD executor — runs decomposed child tasks through subagents
//!
//! Each child agent gets a constrained instruction (its own scope, not the
//! full plan). The executor respects the dependency graph: no child runs
//! until its dependencies have completed.

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;

use super::types::{ChildResult, SddError, SddState};
use crate::agent::{SubAgentHandle, SubAgentStatus};
use crate::pipeline::decomposition::{DecompositionPlan, ExecutionStrategy};

/// Abstraction over how child agents are spawned.
/// Lets us test SddExecutor without a real AgentLoop/SubAgentManager.
#[async_trait::async_trait]
pub trait AgentProvider: Send + Sync {
    async fn spawn_child(&self, label: &str, instruction: &str) -> Result<SubAgentHandle>;
}

/// Executes a decomposition plan through child subagents
pub struct SddExecutor {
    provider: Arc<dyn AgentProvider>,
    /// Max seconds to wait for each subagent
    #[allow(dead_code)]
    subagent_timeout_secs: u64,
}

impl SddExecutor {
    pub fn new(provider: Arc<dyn AgentProvider>, subagent_timeout_secs: u64) -> Self {
        Self {
            provider,
            subagent_timeout_secs,
        }
    }

    /// Execute all children according to their dependency graph.
    pub async fn execute(
        &self,
        plan: &DecompositionPlan,
        session: &mut super::types::SddSession,
    ) -> Result<Vec<ChildResult>, SddError> {
        if plan.children.is_empty() {
            return Ok(Vec::new());
        }

        session
            .transition(SddState::ChildrenExecuting)
            .map_err(|e| SddError::ChildExecution(e.to_string()))?;

        let results = match plan.execution_strategy {
            ExecutionStrategy::Parallel | ExecutionStrategy::LimitedParallel(_) => {
                self.execute_parallel(plan).await
            }
            ExecutionStrategy::DependencyOrder => self.execute_with_deps(plan).await,
            ExecutionStrategy::Sequential => self.execute_sequential(plan).await,
        }?;

        let any_failed = results.iter().any(|r| !r.success);
        if any_failed {
            session
                .transition(SddState::Failed)
                .map_err(|e| SddError::ChildExecution(e.to_string()))?;
            return Err(SddError::ChildExecution(
                "One or more child agents failed".to_string(),
            ));
        }

        session
            .transition(SddState::ChildrenComplete)
            .map_err(|e| SddError::ChildExecution(e.to_string()))?;
        Ok(results)
    }

    async fn execute_parallel(
        &self,
        plan: &DecompositionPlan,
    ) -> Result<Vec<ChildResult>, SddError> {
        let mut handles = Vec::new();
        for child in &plan.children {
            let handle = self
                .provider
                .spawn_child(&child.key, &child.instruction)
                .await
                .map_err(|e| SddError::ChildExecution(format!("spawn failed: {e}")))?;
            handles.push((child.clone(), handle));
        }

        self.collect_results(handles).await
    }

    async fn execute_with_deps(
        &self,
        plan: &DecompositionPlan,
    ) -> Result<Vec<ChildResult>, SddError> {
        let mut results = Vec::new();
        let mut completed_keys = HashSet::new();

        // First pass: run children whose deps are satisfied
        for child in &plan.children {
            let deps_satisfied = child.depends_on.iter().all(|dep| {
                completed_keys.contains(dep)
                    || completed_keys
                        .contains(&dep.strip_prefix("child-").unwrap_or(dep).to_string())
            });

            if !deps_satisfied {
                continue;
            }

            let handle = self
                .provider
                .spawn_child(&child.key, &child.instruction)
                .await
                .map_err(|e| SddError::ChildExecution(format!("spawn failed: {e}")))?;

            let result = self.wait_for_result(&child.key, handle).await;
            if result.success {
                completed_keys.insert(child.key.clone());
            }
            results.push(result);
        }

        // Second pass: retry skipped children (deps may now be met)
        for child in &plan.children {
            if results.iter().any(|r| r.key == child.key) {
                continue;
            }
            let handle = self
                .provider
                .spawn_child(&child.key, &child.instruction)
                .await
                .map_err(|e| SddError::ChildExecution(format!("spawn failed: {e}")))?;
            let result = self.wait_for_result(&child.key, handle).await;
            if result.success {
                completed_keys.insert(child.key.clone());
            }
            results.push(result);
        }

        Ok(results)
    }

    async fn execute_sequential(
        &self,
        plan: &DecompositionPlan,
    ) -> Result<Vec<ChildResult>, SddError> {
        let mut results = Vec::new();
        for child in &plan.children {
            let handle = self
                .provider
                .spawn_child(&child.key, &child.instruction)
                .await
                .map_err(|e| SddError::ChildExecution(format!("spawn failed: {e}")))?;
            results.push(self.wait_for_result(&child.key, handle).await);
        }
        Ok(results)
    }

    async fn collect_results(
        &self,
        handles: Vec<(
            crate::pipeline::decomposition::ChildTaskDefinition,
            SubAgentHandle,
        )>,
    ) -> Result<Vec<ChildResult>, SddError> {
        let mut results = Vec::new();
        for (child, handle) in handles {
            results.push(self.wait_for_result(&child.key, handle).await);
        }
        Ok(results)
    }

    async fn wait_for_result(&self, key: &str, handle: SubAgentHandle) -> ChildResult {
        let success = handle.status == SubAgentStatus::Completed;
        ChildResult {
            key: key.to_string(),
            success,
            summary: handle.result.clone(),
            error: handle.error.clone(),
            files_changed: Vec::new(),
        }
    }
}
