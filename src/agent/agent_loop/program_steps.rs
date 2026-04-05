//! Program step execution: step controller, tool definition filtering, best-of-N delegation.

use tracing::debug;

use crate::agent::best_of_n::{BestOfNConfig, BestOfNExecutor};
use crate::agent::step_controller::StepControl;
use crate::agent::tool_coordinator::{CoordinatorToolDefinition, ToolExecutionResult};
use crate::providers::ContentBlock;
use crate::tools::ToolAccessValidator;

use super::types::{AgentEvent, AgentLoopError, ProgramStepOutcome};
use super::AgentLoop;

impl AgentLoop {
    /// Execute a programmatic step (step controller).
    pub(super) async fn execute_program_step(
        &self,
        step: StepControl,
        model: &str,
        system_prompt: &str,
        working_dir: &str,
        session_id: &str,
    ) -> Result<ProgramStepOutcome, AgentLoopError> {
        match step {
            StepControl::Continue | StepControl::Step | StepControl::StepAll => {
                Ok(ProgramStepOutcome::ProceedToProvider)
            }
            StepControl::WaitForInput | StepControl::End => Ok(ProgramStepOutcome::Stop),
            StepControl::ToolCall { tool, input } => {
                self.emit(AgentEvent::ToolStart {
                    id: "program-step".to_string(),
                    name: tool.clone(),
                });
                let result = self
                    .execute_tools(
                        vec![("program-step".to_string(), tool.clone(), input)],
                        working_dir,
                        session_id,
                    )
                    .await;

                for item in &result {
                    self.emit(AgentEvent::ToolEnd {
                        id: item.id.clone(),
                        name: item.name.clone(),
                        result: item.result.content.clone(),
                        is_error: item.result.is_error,
                        elapsed_ms: item.elapsed_ms,
                    });
                }

                let result_blocks: Vec<ContentBlock> = result
                    .iter()
                    .map(|r| {
                        if r.result.is_error {
                            ContentBlock::tool_error(&r.id, &r.result.content)
                        } else {
                            ContentBlock::tool_result(&r.id, &r.result.content)
                        }
                    })
                    .collect();
                self.conversation
                    .write()
                    .await
                    .add_user_blocks(result_blocks);
                self.append_step_controls(Self::extract_step_controls(&result))
                    .await;
                Ok(ProgramStepOutcome::Consumed)
            }
            StepControl::GenerateN {
                n,
                prompt,
                selector_prompt,
            } => {
                let best_prompt = match prompt {
                    Some(prompt) => prompt,
                    None => self.latest_user_prompt().await.unwrap_or_else(|| {
                        "Generate the strongest implementation for the current task.".to_string()
                    }),
                };

                let executor = BestOfNExecutor::with_config(
                    self.provider.clone(),
                    BestOfNConfig {
                        n,
                        selector_prompt: selector_prompt
                            .unwrap_or_else(|| BestOfNConfig::default().selector_prompt),
                        variant_model: Some(model.to_string()),
                        selector_model: Some(model.to_string()),
                        strip_reasoning: true,
                    },
                );

                let result = executor
                    .execute(&best_prompt, Some(system_prompt))
                    .await
                    .map_err(|e| AgentLoopError::LoopDetected(e.to_string()))?;

                let summary = format!(
                    "[SYSTEM] Best-of-{} selected candidate #{}.\n{}\n\nSelected content:\n{}",
                    n,
                    result.best_index + 1,
                    result
                        .selector_reasoning
                        .clone()
                        .unwrap_or_else(|| { "No selector reasoning provided.".to_string() }),
                    result.best_content
                );
                self.conversation.write().await.add_user_text(summary);
                Ok(ProgramStepOutcome::Consumed)
            }
        }
    }

    /// Extract step controls from tool execution results.
    pub(super) fn extract_step_controls(results: &[ToolExecutionResult]) -> Vec<StepControl> {
        results
            .iter()
            .filter_map(|result| result.result.metadata.get("step_control"))
            .filter_map(|value| serde_json::from_value::<StepControl>(value.clone()).ok())
            .collect()
    }

    /// Filter tool definitions by role, plan mode, and thinking support.
    pub(super) fn filter_tool_definitions(
        &self,
        tool_defs: Vec<CoordinatorToolDefinition>,
        role: crate::tools::AgentRole,
        plan_mode: bool,
        supports_native_thinking: bool,
        tool_validator: &ToolAccessValidator,
    ) -> Vec<CoordinatorToolDefinition> {
        tool_defs
            .into_iter()
            .filter(|d| {
                if !tool_validator.is_allowed(role, &d.name) {
                    debug!(tool_name = %d.name, role = ?role, "Tool filtered out for role");
                    return false;
                }
                if d.name == "Think" && supports_native_thinking {
                    debug!(tool_name = %d.name, "Legacy Think tool filtered out (native thinking available)");
                    return false;
                }
                if plan_mode {
                    let is_safe = matches!(
                        d.name.as_str(),
                        "Think" | "ReadFile" | "ListDir" | "SearchProject"
                            | "ViewContentChunk" | "SearchWeb" | "ViewWebpage"
                            | "ReadImage" | "ReadVideo" | "CheckUrl"
                    );
                    if !is_safe {
                        debug!(tool_name = %d.name, "Tool filtered out (plan mode active)");
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Convert tool definitions from internal to provider format.
    pub(super) fn convert_tool_definitions(
        defs: Vec<CoordinatorToolDefinition>,
    ) -> Vec<crate::providers::ToolDefinition> {
        defs.into_iter()
            .filter_map(|d| match serde_json::from_value(d.input_schema.clone()) {
                Ok(schema) => Some(crate::providers::ToolDefinition {
                    name: d.name,
                    description: d.description,
                    input_schema: schema,
                }),
                Err(_) => Some(crate::providers::ToolDefinition {
                    name: d.name,
                    description: d.description,
                    input_schema: crate::providers::ToolSchema {
                        schema_type: "object".to_string(),
                        properties: std::collections::HashMap::new(),
                        required: None,
                    },
                }),
            })
            .collect()
    }

    /// Build thinking configuration for the given model.
    pub(super) fn build_thinking_config(
        &self,
        model: &str,
        thinking_enabled: bool,
        thinking_budget: Option<u32>,
    ) -> Option<crate::providers::ThinkingConfig> {
        if let Some(info) = self.provider.model_info(model) {
            if info.supports_thinking && thinking_enabled {
                return Some(crate::providers::ThinkingConfig {
                    enabled: true,
                    budget_tokens: thinking_budget.or(info.default_thinking_budget),
                    reasoning_effort: Some(crate::providers::ReasoningEffort::High),
                });
            }
        }
        None
    }
}
