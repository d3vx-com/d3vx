//! Main agent loop execution: the run() method and its core iteration logic.

use tracing::{debug, info, warn};

use crate::agent::cost::calculate_cost_providers;
use crate::agent::state::{AgentState, StateTransitionReason};
use crate::providers::{ContentBlock, StopReason};
use crate::tools::ToolAccessValidator;

use super::types::{AgentEvent, AgentLoopError, AgentResult, ProgramStepOutcome};
use super::AgentLoop;

impl AgentLoop {
    /// Check and enforce budget limits.
    /// Returns true if budget exceeded and loop should stop.
    async fn check_budget(&self, model: &str) -> bool {
        let Some(ref budget) = self.budget_config else {
            return false;
        };

        if !budget.enabled {
            return false;
        }

        let total_usage = self.total_usage.read().await.clone();
        let cost = calculate_cost_providers(&total_usage, model);
        *self.session_cost.write().await = cost;

        let per_session = budget.per_session;
        let session_id = self.config.read().await.session_id.clone();

        if cost >= budget.pause_at * per_session {
            warn!(
                session_id = %session_id,
                cost = cost,
                budget = per_session,
                "Budget exhausted — stopping agent loop"
            );
            self.emit(AgentEvent::Error {
                error: format!(
                    "Budget exhausted: ${:.2} spent of ${:.2} session limit",
                    cost, per_session
                ),
            });
            return true;
        }

        if cost >= budget.warn_at * per_session {
            warn!(
                session_id = %session_id,
                cost = cost,
                budget = per_session,
                "Budget warning: {:.0}% of session limit reached",
                (cost / per_session * 100.0)
            );
        }

        false
    }

    /// Run the agent loop with the current conversation.
    ///
    /// This method:
    /// 1. Sends messages to the provider
    /// 2. Receives streaming response
    /// 3. Handles tool_use blocks by executing tools
    /// 4. Appends results back to conversation
    /// 5. Continues until end_turn or max iterations
    pub async fn run(&self) -> Result<AgentResult, AgentLoopError> {
        let mut iterations = 0u32;
        let mut tool_calls = 0u32;
        let mut accumulated_text = String::new();
        let mut task_completed = false;
        let mut budget_exhausted = false;
        let mut doom_loop_detected = false;

        // Cache config values to avoid repeated lock acquisition
        let (
            session_id,
            model,
            max_iterations,
            system_prompt,
            working_dir,
            is_subagent,
            role,
            thinking_enabled,
            thinking_budget,
            plan_mode,
        ) = {
            let config = self.config.read().await;
            (
                config.session_id.clone(),
                config.model.clone(),
                config.max_iterations,
                config.system_prompt.clone(),
                config.working_dir.clone(),
                config.is_subagent,
                config.role,
                config.thinking_enabled,
                config.thinking_budget,
                config.plan_mode,
            )
        };

        let tool_validator = ToolAccessValidator::new();

        debug!(
            session_id = %session_id,
            model = %model,
            max_iterations = max_iterations,
            role = ?role,
            plan_mode = plan_mode,
            "Starting agent loop"
        );

        while iterations < max_iterations {
            self.wait_if_paused().await;

            if let Some(step) = self.next_program_step().await {
                match self
                    .execute_program_step(step, &model, &system_prompt, &working_dir, &session_id)
                    .await?
                {
                    ProgramStepOutcome::ProceedToProvider => {}
                    ProgramStepOutcome::Consumed => continue,
                    ProgramStepOutcome::Stop => break,
                }
            }

            self.state_tracker
                .activate(StateTransitionReason::ActivityDetected)
                .await;

            iterations += 1;

            let messages = {
                let conv = self.conversation.read().await;
                conv.get_messages()
            };

            // Build tool definitions filtered by role & capabilities
            let tool_defs = self.tools.get_tool_definitions().await;
            let supports_native_thinking = self
                .provider
                .model_info(&model)
                .map(|i| i.supports_thinking && thinking_enabled)
                .unwrap_or(false);

            let filtered_tool_defs = self.filter_tool_definitions(
                tool_defs,
                role,
                plan_mode,
                supports_native_thinking,
                &tool_validator,
            );

            let provider_tools = Self::convert_tool_definitions(filtered_tool_defs);
            let thinking = self.build_thinking_config(&model, thinking_enabled, thinking_budget);

            let request = crate::providers::MessagesRequest {
                model: model.clone(),
                messages,
                system_prompt: Some(system_prompt.clone()),
                tools: provider_tools,
                max_tokens: None,
                temperature: None,
                thinking,
                prompt_caching: true,
            };

            // Stream response with retries
            let stream_result = self.send_with_retry(request.clone()).await;

            let mut stream = match self.handle_stream_result(stream_result).await {
                Ok(s) => s,
                Err(should_continue) => {
                    if should_continue {
                        continue;
                    }
                    continue;
                }
            };

            // Process stream events
            let (response_text, stop_reason, pending_tool_calls, should_continue_after_error) =
                self.process_stream_events(&mut stream, &mut accumulated_text)
                    .await;

            // Build assistant message
            if !response_text.is_empty() || !pending_tool_calls.is_empty() {
                let mut content_blocks: Vec<ContentBlock> = Vec::new();
                if !response_text.is_empty() {
                    content_blocks.push(ContentBlock::text(&response_text));
                }
                for (id, name, input) in &pending_tool_calls {
                    if name == "complete_task" {
                        task_completed = true;
                    }
                    content_blocks.push(ContentBlock::tool_use(id, name, input.clone()));
                }
                self.conversation
                    .write()
                    .await
                    .add_assistant_blocks(content_blocks);
            }

            // Check stop condition
            if !should_continue_after_error
                && (stop_reason != StopReason::ToolUse || pending_tool_calls.is_empty())
            {
                if is_subagent && !task_completed {
                    let reminder = "You ended without calling complete_task. If you made code changes, verify them first (tests, type-check, etc.) as appropriate for the task scope, then call `complete_task` with a summary. For trivial tasks (creating files, simple lookups), just call `complete_task` directly.";
                    let reminder_block = ContentBlock::text(reminder);
                    self.conversation
                        .write()
                        .await
                        .add_user_blocks(vec![reminder_block]);
                    continue;
                }
                break;
            }

            // Execute tool calls
            self.state_tracker
                .transition_to(
                    AgentState::ToolExecution,
                    StateTransitionReason::ActivityDetected,
                )
                .await;

            tracing::info!("Executing {} tool calls", pending_tool_calls.len());
            for (id, name, input) in &pending_tool_calls {
                tracing::info!("Executing tool: {} ({})", name, id);

                // Check for doom loop patterns. Record for every tool call so
                // the detector sees the full pattern. If any call trips the
                // threshold, flag the iteration — we still let the loop body
                // finish scanning so users see warnings for every offender,
                // but we skip tool execution and break the outer loop below.
                if let Ok(mut detector) = self.doom_detector.lock() {
                    if let Some(warning) = detector.record(name, input) {
                        warn!(
                            "Doom loop detected: tool '{}' repeated {} times. {}",
                            warning.tool, warning.repeats, warning.suggestion
                        );
                        self.emit(AgentEvent::Error {
                            error: format!(
                                "Loop detected: {} called {} times. {}",
                                warning.tool, warning.repeats, warning.suggestion
                            ),
                        });
                        doom_loop_detected = true;
                    }
                }
            }

            // Honour the detector: don't execute a batch we know is looping.
            // Stop cleanly with `doom_loop_detected` set so callers can
            // distinguish runaway-stop from normal completion.
            if doom_loop_detected {
                break;
            }

            let tool_results = self
                .execute_tools(pending_tool_calls, &working_dir, &session_id)
                .await;
            tracing::info!("Tool execution completed, {} results", tool_results.len());

            self.append_step_controls(Self::extract_step_controls(&tool_results))
                .await;

            for result in &tool_results {
                self.emit(AgentEvent::ToolEnd {
                    id: result.id.clone(),
                    name: result.name.clone(),
                    result: result.result.content.clone(),
                    is_error: result.result.is_error,
                    elapsed_ms: result.elapsed_ms,
                });
            }

            let result_blocks: Vec<ContentBlock> = tool_results
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

            tool_calls += tool_results.len() as u32;
            debug!(
                iteration = iterations,
                tool_calls = tool_results.len(),
                "Iteration completed"
            );

            // Check budget and stop if exceeded
            if self.check_budget(&model).await {
                budget_exhausted = true;
                break;
            }

            // Auto-compact if approaching context limit
            self.auto_compact_if_needed().await;
        }

        if iterations >= max_iterations {
            warn!(max_iterations = max_iterations, "Agent hit max iterations");
            self.emit(AgentEvent::Error {
                error: format!(
                    "Reached {} iterations. Pausing to avoid runaway costs.",
                    max_iterations
                ),
            });
        }

        let total_usage = self.total_usage.read().await.clone();

        self.emit(AgentEvent::IterationEnd {
            iteration: self.state_tracker.get_iterations().await,
            usage: total_usage.clone(),
        });

        self.state_tracker.complete().await;

        self.emit(AgentEvent::Done {
            final_text: accumulated_text.clone(),
            iterations,
            tool_calls,
            total_usage: total_usage.clone(),
        });

        self.emit(AgentEvent::Finished {
            iterations,
            tool_calls,
            total_usage: total_usage.clone(),
        });

        info!(
            session_id = %session_id,
            iterations = iterations,
            tool_calls = tool_calls,
            input_tokens = total_usage.input_tokens,
            output_tokens = total_usage.output_tokens,
            "Agent loop completed"
        );

        Ok(AgentResult {
            text: accumulated_text,
            usage: total_usage,
            tool_calls,
            iterations,
            task_completed,
            budget_exhausted,
            doom_loop_detected,
        })
    }
}
