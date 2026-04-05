//! Implement phase handler with QA fix loop support

use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info, warn};

use super::types::{PhaseError, PhaseHandler, PhaseResult};
use crate::agent::AgentLoop;
use crate::pipeline::commander::ValidationRunner;
use crate::pipeline::handlers::impl_spec::ImplementationSpec;
use crate::pipeline::phases::{Phase, PhaseContext, Task};
use crate::pipeline::prompts;
use crate::pipeline::qa_loop::{PendingFinding, QALoop, QAState};
use crate::pipeline::validation_summary::ValidationSummary;

pub struct ImplementHandler {
    run_validation: bool,
}

impl ImplementHandler {
    pub fn new() -> Self {
        Self {
            run_validation: true,
        }
    }

    pub fn with_validation(enabled: bool) -> Self {
        Self {
            run_validation: enabled,
        }
    }

    pub(crate) fn generate_instruction(&self, context: &PhaseContext) -> String {
        let mut instruction = prompts::build_phase_instruction(
            Phase::Implement,
            &context.task.title,
            &context.task.instruction,
            &context.task.id,
            context.memory_context.as_deref(),
            context.agent_rules.as_deref(),
            context.ignore_instruction.as_deref(),
        );

        // If there's a spec from the Plan phase, inject it as primary context.
        // This provides focused, structured context without the full conversation history.
        let plan_file = format!(".d3vx/plan-{}.json", context.task.id);
        let spec_path = Path::new(&context.worktree_path).join(&plan_file);
        if let Ok(spec) = ImplementationSpec::load(&spec_path) {
            let spec_block = spec.to_instruction_block();
            instruction.push_str("\n\n## Implementation Spec (Primary Context)\n\n");
            instruction.push_str(&spec_block);
            info!("Injected implementation spec as primary context");
        }

        instruction
    }

    fn generate_fix_instruction(
        &self,
        context: &PhaseContext,
        findings: &[PendingFinding],
    ) -> String {
        let findings_text = findings
            .iter()
            .map(|f| {
                format!(
                    "- [{}] {}: {}{}",
                    f.severity,
                    f.category,
                    f.title,
                    f.suggestion
                        .as_ref()
                        .map(|s| format!("\n  Fix: {}", s))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "{}\n\n## Previous Review Found Issues - Must Fix\n\nThe following blocking issues were identified:\n\n{}\n\nFix each issue thoroughly. Run validation after fixes to confirm resolution.",
            self.generate_instruction(context),
            findings_text
        )
    }

    fn load_qa_loop(&self, task: &Task) -> Option<QALoop> {
        QALoop::from_metadata(task.id.clone(), &task.metadata)
    }

    async fn run_validation(&self, worktree_path: &str) -> Option<ValidationSummary> {
        if !self.run_validation {
            return None;
        }

        let runner = ValidationRunner::new(Path::new(worktree_path).to_path_buf());
        let results = runner.run_all().await;
        if results.is_empty() {
            return None;
        }
        Some(ValidationSummary::from_results(results))
    }
}

impl Default for ImplementHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ImplementHandler {
    fn clone(&self) -> Self {
        Self {
            run_validation: self.run_validation,
        }
    }
}

#[async_trait]
impl PhaseHandler for ImplementHandler {
    fn phase(&self) -> Phase {
        Phase::Implement
    }

    async fn execute(
        &self,
        task: &Task,
        context: &PhaseContext,
        agent: Option<Arc<AgentLoop>>,
    ) -> Result<PhaseResult, PhaseError> {
        self.can_execute(task)?;

        let mut qa_loop = self.load_qa_loop(task);
        let pending_fixes = qa_loop
            .as_ref()
            .filter(|qa| qa.state == QAState::AwaitingFix)
            .map(|qa| qa.pending_findings.clone())
            .unwrap_or_default();

        let is_fix_mode = !pending_fixes.is_empty();

        let instruction = if is_fix_mode {
            info!(
                task_id = %task.id,
                fixes = pending_fixes.len(),
                "Running in fix mode with pending findings"
            );
            self.generate_fix_instruction(context, &pending_fixes)
        } else {
            self.generate_instruction(context)
        };

        let Some(agent) = agent else {
            let mut metadata = serde_json::json!({
                "instruction": instruction,
                "plan_file": format!(".d3vx/plan-{}.json", task.id),
            });
            if let Some(ref qa) = qa_loop {
                metadata["qa_loop"] = qa.to_metadata();
            }
            return Ok(
                PhaseResult::success("Implement phase prepared (dry-run)").with_metadata(metadata)
            );
        };

        let _system_prompt = prompts::get_system_prompt(Phase::Implement);

        agent.clear_history().await;
        agent.add_user_message(&instruction).await;

        let patch_path = Path::new(&context.worktree_path)
            .join(".d3vx")
            .join(format!("draft-{}.patch", task.id));
        if patch_path.exists() {
            info!("Applying draft patch from {}", patch_path.display());
            let patch_output = std::process::Command::new("git")
                .arg("apply")
                .arg(&patch_path)
                .current_dir(&context.worktree_path)
                .output();

            match patch_output {
                Ok(output) if output.status.success() => {
                    info!("Successfully applied patch");
                }
                Ok(output) => {
                    warn!(
                        "Failed to apply patch: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                Err(e) => {
                    error!("Failed to execute git apply: {}", e);
                }
            }
        }

        let result = agent.run().await?;

        let validation_summary = self.run_validation(&context.worktree_path).await;

        let mut metadata = serde_json::json!({
            "instruction": instruction,
            "plan_file": format!(".d3vx/plan-{}.json", task.id),
            "iterations": result.iterations,
            "tool_calls": result.tool_calls,
            "input_tokens": result.usage.input_tokens,
            "output_tokens": result.usage.output_tokens,
            "patch_applied": patch_path.exists(),
            "fix_mode": is_fix_mode,
        });

        if let Some(summary) = &validation_summary {
            if let Ok(summary_json) = serde_json::to_value(summary) {
                metadata["validation_summary"] = summary_json;
            }
        }

        if let Some(ref mut qa) = qa_loop {
            if is_fix_mode {
                qa.start_fix();
                qa.record_fix_result(validation_summary.as_ref(), None);

                let validation_status = validation_summary
                    .as_ref()
                    .map(|v| format!("{:?}: {}/{} passed", v.confidence, v.passed, v.total))
                    .unwrap_or_else(|| "none".to_string());

                info!(
                    task_id = %task.id,
                    iteration = qa.iteration(),
                    pending_fixes = qa.pending_findings.len(),
                    "Fix phase completed, validation: {}",
                    validation_status
                );

                if let Some(ref validation) = validation_summary {
                    if validation.confidence.blocks_merge() {
                        warn!(
                            task_id = %task.id,
                            confidence = ?validation.confidence,
                            "Validation blocks merge - needs attention"
                        );
                        qa.update_from_validation(validation);
                    }
                }
            }

            let merge_readiness = qa.evaluate_merge_readiness();
            info!(
                task_id = %task.id,
                merge_ready = merge_readiness.ready,
                blockers = merge_readiness.reasons.len(),
                "Implement phase computed merge readiness"
            );

            metadata["qa_loop"] = qa.to_metadata();
            metadata["qa_status"] =
                serde_json::to_value(&qa.current_status()).unwrap_or(serde_json::json!({}));
            metadata["merge_readiness"] =
                serde_json::to_value(&merge_readiness).unwrap_or(serde_json::json!({}));
        }

        let confidence_text = validation_summary
            .as_ref()
            .map(|v| format!("{:?}", v.confidence))
            .unwrap_or_else(|| "none".to_string());

        let fix_result_message = if is_fix_mode {
            format!(
                "Fix completed. {} pending fix(es) addressed. Validation: {}",
                pending_fixes.len(),
                confidence_text
            )
        } else {
            result.text.clone()
        };

        if is_fix_mode {
            Ok(PhaseResult::success(fix_result_message).with_metadata(metadata))
        } else {
            Ok(PhaseResult::success(result.text).with_metadata(metadata))
        }
    }
}
