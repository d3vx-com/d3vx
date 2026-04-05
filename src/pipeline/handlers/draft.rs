//! Draft phase handler

use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

use super::types::{PhaseError, PhaseHandler, PhaseResult};
use crate::agent::AgentLoop;
use crate::pipeline::phases::{Phase, PhaseContext, Task};
use crate::pipeline::prompts;

/// Draft phase handler
/// Generates implementation drafts (unified diffs)
pub struct DraftHandler;

impl DraftHandler {
    /// Create a new draft handler
    pub fn new() -> Self {
        Self
    }

    /// Generate the drafting instruction for a task
    pub fn generate_instruction(&self, context: &PhaseContext) -> String {
        prompts::build_phase_instruction(
            Phase::Draft,
            &context.task.title,
            &context.task.instruction,
            &context.task.id,
            context.memory_context.as_deref(),
            context.agent_rules.as_deref(),
            context.ignore_instruction.as_deref(),
        )
    }
}

impl Default for DraftHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PhaseHandler for DraftHandler {
    fn phase(&self) -> Phase {
        Phase::Draft
    }

    async fn execute(
        &self,
        task: &Task,
        context: &PhaseContext,
        agent: Option<Arc<AgentLoop>>,
    ) -> Result<PhaseResult, PhaseError> {
        self.can_execute(task)?;

        let instruction = self.generate_instruction(context);

        let Some(agent) = agent else {
            return Ok(
                PhaseResult::success("Draft phase prepared (dry-run)").with_metadata(
                    serde_json::json!({
                        "instruction": instruction,
                        "draft_file": format!(".d3vx/draft-{}.patch", task.id)
                    }),
                ),
            );
        };

        let system_prompt = prompts::get_system_prompt(Phase::Draft);

        agent.clear_history().await;
        agent.add_user_message(&instruction).await;
        agent.set_system_prompt(system_prompt).await;

        let result = agent.run().await?;

        let metadata = serde_json::json!({
            "instruction": instruction,
            "draft_file": format!(".d3vx/draft-{}.patch", task.id),
            "iterations": result.iterations,
            "tool_calls": result.tool_calls,
            "input_tokens": result.usage.input_tokens,
            "output_tokens": result.usage.output_tokens,
        });

        let _patch_path = Path::new(&context.worktree_path)
            .join(".d3vx")
            .join(format!("draft-{}.patch", task.id));
        Ok(PhaseResult::success(result.text).with_metadata(metadata))
    }
}
