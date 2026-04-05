//! Ideation Phase Handler
//!
//! Before committing to a plan, the ideation phase explores alternatives,
//! evaluates trade-offs, and surfaces clarifying questions.
//!
//! ## Flow
//!
//! ```text
//! Research → Ideation → Plan → ...
//! ```

use async_trait::async_trait;
use std::sync::Arc;

use super::types::{PhaseError, PhaseHandler, PhaseResult};
use crate::agent::AgentLoop;
use crate::pipeline::phases::{Phase, PhaseContext, Task};
use crate::pipeline::prompts;

/// An alternative approach explored during ideation
#[derive(Debug, Clone)]
pub struct Alternative {
    /// Human-readable name
    pub name: String,
    /// Brief description of the approach
    pub description: String,
    /// Advantages
    pub pros: Vec<String>,
    /// Disadvantages
    pub cons: Vec<String>,
    /// Estimated effort: Low, Medium, High
    pub effort: String,
    /// Risk level: Low, Medium, High
    pub risk: String,
}

/// A question surfaced during ideation
#[derive(Debug, Clone)]
pub struct ClarifyingQuestion {
    pub question: String,
    pub why_it_matters: String,
    pub answered: bool,
    pub answer: Option<String>,
}

/// The outcome of the ideation phase
#[derive(Debug, Clone)]
pub struct IdeationOutcome {
    pub alternatives: Vec<Alternative>,
    pub recommended_index: usize,
    pub reasoning: String,
    pub clarifying_questions: Vec<ClarifyingQuestion>,
}

/// Handler for the ideation phase.
pub struct IdeationHandler;

impl IdeationHandler {
    pub fn new() -> Self {
        Self
    }

    /// Generate the instruction for ideation.
    fn ideation_prompt(&self, task: &Task, context: &PhaseContext) -> String {
        format!(
            r#"You are in the Ideation phase for task: {task_id}

## Task
**{title}**

{instruction}

## Goal
Before committing to an implementation plan, explore multiple valid approaches.
Consider trade-offs in complexity, risk, maintainability, and developer experience.

## Output Format
Produce a comparison of at least 2 and at most 4 viable approaches.
For each approach, include name, one-sentence description, 2-4 pros, 2-4 cons,
estimated effort (Low / Medium / High), and risk level.

Then recommend one approach with clear reasoning and surface up to 3
clarifying questions if there is ambiguity that materially affects the choice.

## Worktree
Project root: {project_root}
Worktree: {worktree}"#,
            task_id = task.id,
            title = task.title,
            instruction = task.instruction,
            project_root = context.project_root,
            worktree = context.worktree_path,
        )
    }
}

#[async_trait]
impl PhaseHandler for IdeationHandler {
    fn phase(&self) -> Phase {
        Phase::Ideation
    }

    async fn execute(
        &self,
        task: &Task,
        context: &PhaseContext,
        agent: Option<Arc<AgentLoop>>,
    ) -> Result<PhaseResult, PhaseError> {
        let prompt = self.ideation_prompt(task, context);

        let Some(agent_loop) = agent else {
            return Ok(
                PhaseResult::success("Ideation dry-run: no agent provided").with_metadata(
                    serde_json::json!({
                        "instruction": prompt,
                    }),
                ),
            );
        };

        let _system_prompt = prompts::get_system_prompt(Phase::Ideation);

        agent_loop.clear_history().await;
        agent_loop.add_user_message(&prompt).await;

        let result = agent_loop
            .run()
            .await
            .map_err(|e| PhaseError::ExecutionFailed {
                message: format!("agent failed during ideation: {e}"),
            })?;

        let summary = format!(
            "Ideation complete: {}",
            result.text.chars().take(120).collect::<String>()
        );

        Ok(
            PhaseResult::success(summary).with_metadata(serde_json::json!({
                "iterations": result.iterations,
                "tool_calls": result.tool_calls,
            })),
        )
    }

    fn name(&self) -> &'static str {
        "Ideation"
    }
}

impl Default for IdeationHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ideation_handler_creation() {
        let handler = IdeationHandler::new();
        assert_eq!(handler.phase(), Phase::Ideation);
        assert_eq!(handler.name(), "Ideation");
    }
}
