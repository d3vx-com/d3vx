//! Stream processing helpers: error recovery, event parsing, usage tracking.

use futures::StreamExt;
use tracing::{debug, error, warn};

use crate::providers::{StopReason, StreamEvent, TokenUsage};

use super::types::AgentEvent;
use super::AgentLoop;

impl AgentLoop {
    /// Handle stream result, applying recovery logic on errors.
    /// Returns Ok(stream) on success, Err(should_continue) for retry cases.
    pub(super) async fn handle_stream_result(
        &self,
        stream_result: Result<crate::providers::StreamResult, crate::providers::ProviderError>,
    ) -> Result<crate::providers::StreamResult, bool> {
        match stream_result {
            Ok(s) => {
                let mut count = self.failure_count.write().await;
                *count = 0;
                Ok(s)
            }
            Err(e) => {
                error!(error = %e, "Failed to get stream from provider");

                let mut count_lock = self.failure_count.write().await;
                *count_lock += 1;
                let count = *count_lock;

                let action = self.recovery_strategy.next_action(count);
                warn!(action = ?action, failure_count = count, "Recovery escalation triggered");

                match action {
                    crate::recovery::EscalationLevel::Retry
                    | crate::recovery::EscalationLevel::Backoff => {
                        let delay = self.recovery_strategy.get_delay(count);
                        self.emit(AgentEvent::Error {
                            error: format!(
                                "Provider error (attempt {}). Retrying in {:?}...",
                                count, delay
                            ),
                        });
                        drop(count_lock);
                        tokio::time::sleep(delay).await;
                        Err(true)
                    }
                    crate::recovery::EscalationLevel::Human => {
                        self.emit(AgentEvent::Error {
                            error:
                                "Critical failure. Escalating to human intervention. Agent paused."
                                    .to_string(),
                        });
                        self.pause().await;
                        drop(count_lock);
                        self.wait_if_paused().await;
                        Err(true)
                    }
                    crate::recovery::EscalationLevel::Restore => {
                        self.emit(AgentEvent::Error {
                            error: "Attempting session restoration from last checkpoint..."
                                .to_string(),
                        });
                        drop(count_lock);
                        Err(true)
                    }
                    _ => {
                        self.emit(AgentEvent::Error {
                            error: format!("Max recovery attempts reached ({}). Aborting.", count),
                        });
                        Err(false)
                    }
                }
            }
        }
    }

    /// Process stream events and return extracted data.
    pub(super) async fn process_stream_events(
        &self,
        stream: &mut crate::providers::StreamResult,
        accumulated_text: &mut String,
    ) -> (
        String,
        StopReason,
        Vec<(String, String, serde_json::Value)>,
        bool,
    ) {
        let mut response_text = String::new();
        let mut stop_reason = StopReason::EndTurn;
        let mut pending_tool_calls: Vec<(String, String, serde_json::Value)> = Vec::new();
        let mut current_tool_json = String::new();
        let mut should_continue_after_error = false;

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => match event {
                    StreamEvent::TextDelta { text } => {
                        response_text.push_str(&text);
                        accumulated_text.push_str(&text);
                        self.emit(AgentEvent::Text { text });
                    }
                    StreamEvent::ThinkingDelta { text } => {
                        self.emit(AgentEvent::Thinking { text });
                    }
                    StreamEvent::ToolUseStart { id, name } => {
                        current_tool_json.clear();
                        self.emit(AgentEvent::ToolStart { id, name });
                    }
                    StreamEvent::ToolUseDelta { input_json } => {
                        current_tool_json.push_str(&input_json);
                        self.emit(AgentEvent::ToolInput { json: input_json });
                    }
                    StreamEvent::ToolUseEnd { id, name, input } => {
                        tracing::info!("ToolUseEnd received: tool={}, id={}", name, id);
                        pending_tool_calls.push((id, name, input));
                    }
                    StreamEvent::MessageEnd {
                        usage,
                        stop_reason: reason,
                    } => {
                        stop_reason = reason;
                        self.update_total_usage(&usage).await;
                        let total = self.total_usage.read().await.clone();
                        self.emit(AgentEvent::MessageEnd {
                            usage: total,
                            stop_reason,
                        });
                    }
                    StreamEvent::Error { error } => {
                        let error_msg = format!("{:?}", error);
                        error!(error = %error_msg, "Stream error event");
                        self.emit(AgentEvent::Error { error: error_msg });
                    }
                    StreamEvent::MessageStart {
                        id,
                        model: _,
                        usage,
                    } => {
                        debug!(message_id = %id, "Message started");
                        self.update_total_usage(&usage).await;
                    }
                },
                Err(e) => {
                    let error_msg = format!("{:?}", e);
                    warn!(error = %error_msg, "Stream connection error");

                    if e.is_retryable() {
                        let retry_marker = "\n\n[Network interruption. Retrying...]\n";
                        warn!("Stream error is retryable. Capturing partial response and continuing turn.");
                        should_continue_after_error = true;
                        response_text.push_str(retry_marker);
                        accumulated_text.push_str(retry_marker);
                        self.emit(AgentEvent::Text {
                            text: retry_marker.to_string(),
                        });
                    } else {
                        self.emit(AgentEvent::Error {
                            error: error_msg.clone(),
                        });
                    }
                    break;
                }
            }
        }

        (
            response_text,
            stop_reason,
            pending_tool_calls,
            should_continue_after_error,
        )
    }

    /// Update total token usage with new usage data.
    pub(super) async fn update_total_usage(&self, usage: &TokenUsage) {
        let mut total = self.total_usage.write().await;
        total.input_tokens += usage.input_tokens;
        total.output_tokens += usage.output_tokens;
        if let Some(cache_read) = usage.cache_read_tokens {
            total.cache_read_tokens = Some(total.cache_read_tokens.unwrap_or(0) + cache_read);
        }
        if let Some(cache_write) = usage.cache_write_tokens {
            total.cache_write_tokens = Some(total.cache_write_tokens.unwrap_or(0) + cache_write);
        }
    }
}
