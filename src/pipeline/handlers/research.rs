//! Research phase handler

use async_trait::async_trait;
use std::sync::Arc;

use super::types::{check_agent_safety, PhaseError, PhaseHandler, PhaseResult};
use crate::agent::AgentLoop;
use crate::pipeline::phases::{Phase, PhaseContext, Task};
use crate::pipeline::prompts;

/// Research phase handler
/// Analyzes requirements, gathers context
pub struct ResearchHandler;

impl ResearchHandler {
    /// Create a new research handler
    pub fn new() -> Self {
        Self
    }

    /// Generate the research instruction for a task
    pub fn generate_instruction(&self, context: &PhaseContext) -> String {
        prompts::build_phase_instruction(
            Phase::Research,
            &context.task.title,
            &context.task.instruction,
            &context.task.id,
            context.memory_context.as_deref(),
            context.agent_rules.as_deref(),
            context.ignore_instruction.as_deref(),
        )
    }
}

impl Default for ResearchHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PhaseHandler for ResearchHandler {
    fn phase(&self) -> Phase {
        Phase::Research
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
                PhaseResult::success("Research phase prepared (dry-run)").with_metadata(
                    serde_json::json!({
                        "instruction": instruction,
                        "research_file": format!(".d3vx/research-{}.md", task.id)
                    }),
                ),
            );
        };

        let system_prompt = prompts::get_system_prompt(Phase::Research);

        agent.clear_history().await;
        agent.add_user_message(&instruction).await;
        agent.set_system_prompt(system_prompt).await;

        let result = check_agent_safety(agent.run().await?)?;

        let metadata = serde_json::json!({
            "instruction": instruction,
            "research_file": format!(".d3vx/research-{}.md", task.id),
            "iterations": result.iterations,
            "tool_calls": result.tool_calls,
            "input_tokens": result.usage.input_tokens,
            "output_tokens": result.usage.output_tokens,
        });

        Ok(PhaseResult::success(result.text).with_metadata(metadata))
    }
}
