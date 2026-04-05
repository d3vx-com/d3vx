//! Plan phase handler

use async_trait::async_trait;
use std::sync::Arc;

use super::types::{PhaseError, PhaseHandler, PhaseResult};
use crate::agent::AgentLoop;
use crate::pipeline::handlers::impl_spec::ImplementationSpec;
use crate::pipeline::phases::{Phase, PhaseContext, Task};
use crate::pipeline::prompts;

/// Plan phase handler
/// Creates implementation plan
pub struct PlanHandler;

impl PlanHandler {
    /// Create a new plan handler
    pub fn new() -> Self {
        Self
    }

    /// Generate the planning instruction for a task
    pub fn generate_instruction(&self, context: &PhaseContext) -> String {
        prompts::build_phase_instruction(
            Phase::Plan,
            &context.task.title,
            &context.task.instruction,
            &context.task.id,
            context.memory_context.as_deref(),
            context.agent_rules.as_deref(),
            context.ignore_instruction.as_deref(),
        )
    }
}

impl Default for PlanHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PhaseHandler for PlanHandler {
    fn phase(&self) -> Phase {
        Phase::Plan
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
                PhaseResult::success("Plan phase prepared (dry-run)").with_metadata(
                    serde_json::json!({
                        "instruction": instruction,
                        "plan_file": format!(".d3vx/plan-{}.json", task.id)
                    }),
                ),
            );
        };

        let _system_prompt = prompts::get_system_prompt(Phase::Plan);

        agent.clear_history().await;
        agent.add_user_message(&instruction).await;

        let result = agent.run().await?;

        // After the plan agent finishes, load the plan JSON from disk
        // (the agent is instructed to write it there) into an ImplementationSpec.
        let plan_file = format!(".d3vx/plan-{}.json", task.id);
        let spec_path = std::path::Path::new(&context.worktree_path).join(&plan_file);
        let spec = ImplementationSpec::load(&spec_path).ok();
        let spec_path_str = spec_path.to_string_lossy().to_string();

        let metadata = serde_json::json!({
            "instruction": instruction,
            "plan_file": plan_file,
            "spec_file": spec_path_str,
            "iterations": result.iterations,
            "tool_calls": result.tool_calls,
            "input_tokens": result.usage.input_tokens,
            "output_tokens": result.usage.output_tokens,
        });

        Ok(PhaseResult::success(result.text).with_metadata(metadata))
    }
}
