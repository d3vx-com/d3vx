//! Docs phase handler

use async_trait::async_trait;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::types::{PhaseError, PhaseHandler, PhaseResult};
use crate::agent::AgentLoop;
use crate::pipeline::docs_completeness::{DocsCompleteness, DocsCompletenessEvaluator};
use crate::pipeline::phases::{Phase, PhaseContext, Task};
use crate::pipeline::prompts;
use crate::pipeline::qa_loop::QALoop;

/// Docs phase handler
/// Generates documentation
pub struct DocsHandler;

impl DocsHandler {
    /// Create a new docs handler
    pub fn new() -> Self {
        Self
    }

    /// Generate the docs instruction for a task
    pub fn generate_instruction(&self, context: &PhaseContext) -> String {
        prompts::build_phase_instruction(
            Phase::Docs,
            &context.task.title,
            &context.task.instruction,
            &context.task.id,
            context.memory_context.as_deref(),
            context.agent_rules.as_deref(),
            context.ignore_instruction.as_deref(),
        )
    }

    /// Get changed files from git in the worktree
    fn get_changed_files(worktree_path: &str) -> Vec<String> {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(worktree_path)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let files: Vec<String> = stdout
                    .lines()
                    .filter_map(|line| {
                        if line.len() > 3 {
                            Some(line[3..].to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                debug!(
                    worktree = worktree_path,
                    count = files.len(),
                    "Got changed files from git"
                );
                files
            }
            Ok(output) => {
                warn!(
                    "git status failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                Vec::new()
            }
            Err(e) => {
                warn!(error = %e, "Failed to run git status");
                Vec::new()
            }
        }
    }

    fn load_qa_loop(&self, task: &Task) -> Option<QALoop> {
        QALoop::from_metadata(task.id.clone(), &task.metadata)
    }
}

impl Default for DocsHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PhaseHandler for DocsHandler {
    fn phase(&self) -> Phase {
        Phase::Docs
    }

    async fn execute(
        &self,
        task: &Task,
        context: &PhaseContext,
        agent: Option<Arc<AgentLoop>>,
    ) -> Result<PhaseResult, PhaseError> {
        self.can_execute(task)?;

        let instruction = self.generate_instruction(context);
        let mut qa_loop = self.load_qa_loop(task);

        let Some(agent) = agent else {
            let docs = DocsCompleteness::not_evaluated(Some(task.id.clone()));
            let docs_metadata = serde_json::to_value(&docs).unwrap_or(serde_json::json!({}));
            let mut metadata = serde_json::json!({
                "docs_completeness": docs_metadata,
            });
            if let Some(ref qa) = qa_loop {
                metadata["qa_loop"] = qa.to_metadata();
            }
            return Ok(
                PhaseResult::success("Docs phase prepared (dry-run)").with_metadata(metadata)
            );
        };

        let _system_prompt = prompts::get_system_prompt(Phase::Docs);

        agent.clear_history().await;
        agent.add_user_message(&instruction).await;

        let result = agent.run().await?;

        let evaluator =
            DocsCompletenessEvaluator::new(Path::new(&context.worktree_path).to_path_buf());

        let changed_files = Self::get_changed_files(&context.worktree_path);
        let docs = evaluator.evaluate(&changed_files, &instruction);

        info!(
            task_id = %task.id,
            docs_status = ?docs.status,
            docs_required = docs.docs_required,
            docs_satisfied = docs.satisfied,
            "Docs evaluation complete"
        );

        if let Some(ref mut qa) = qa_loop {
            qa.record_docs_result(&docs);
            let merge_readiness = qa.evaluate_merge_readiness();
            info!(
                task_id = %task.id,
                merge_ready = merge_readiness.ready,
                blockers = merge_readiness.reasons.len(),
                "Docs phase computed merge readiness"
            );
        }

        let docs_metadata = serde_json::to_value(&docs).unwrap_or(serde_json::json!({}));
        let mut metadata = serde_json::json!({
            "instruction": instruction,
            "docs_completeness": docs_metadata,
            "iterations": result.iterations,
            "tool_calls": result.tool_calls,
            "input_tokens": result.usage.input_tokens,
            "output_tokens": result.usage.output_tokens,
        });

        if let Some(ref qa) = qa_loop {
            let merge_readiness = qa.evaluate_merge_readiness();
            metadata["qa_loop"] = qa.to_metadata();
            metadata["qa_status"] =
                serde_json::to_value(&qa.current_status()).unwrap_or(serde_json::json!({}));
            metadata["merge_readiness"] =
                serde_json::to_value(&merge_readiness).unwrap_or(serde_json::json!({}));
        }

        Ok(PhaseResult::success(result.text).with_metadata(metadata))
    }
}
