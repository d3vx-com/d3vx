//! OpenAI SSE Stream Parser
//!
//! Parses the `text/event-stream` format used by OpenAI's Chat Completions API.
//! This is simpler than Anthropic's format:
//! - `data: {json}` lines with a `[DONE]` sentinel.
//! - Each chunk contains `choices[0].delta` with partial content.

use crate::providers::{StopReason, StreamEvent, TokenUsage};
use serde::Deserialize;

/// Parser for OpenAI's SSE stream format.
pub struct OpenAISseParser {
    buffer: String,
    current_tool_id: Option<String>,
    current_tool_name: Option<String>,
    current_tool_args: String,
}

impl OpenAISseParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            current_tool_id: None,
            current_tool_name: None,
            current_tool_args: String::new(),
        }
    }

    /// Parse a chunk of bytes from the SSE stream, returning any complete events.
    pub fn parse(&mut self, bytes: &[u8]) -> Vec<StreamEvent> {
        let text = match std::str::from_utf8(bytes) {
            Ok(t) => t,
            Err(_) => return vec![],
        };

        self.buffer.push_str(text);
        let mut events = Vec::new();

        // Process complete lines
        while let Some(pos) = self.buffer.find("\n\n") {
            let line_block = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos + 2..].to_string();

            for line in line_block.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                if let Some(data) = line.strip_prefix("data: ") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        // Flush any pending tool call
                        if let Some(event) = self.flush_tool_call() {
                            events.push(event);
                        }
                        continue;
                    }

                    if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                        events.extend(self.translate_chunk(chunk));
                    }
                }
            }
        }

        // Also handle single newline-separated lines (some providers send them this way)
        if self.buffer.contains('\n') && !self.buffer.ends_with('\n') {
            // Still accumulating, don't process yet
        } else if self.buffer.contains('\n') {
            let remaining = std::mem::take(&mut self.buffer);
            for line in remaining.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                if let Some(data) = line.strip_prefix("data: ") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        if let Some(event) = self.flush_tool_call() {
                            events.push(event);
                        }
                        continue;
                    }
                    if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                        events.extend(self.translate_chunk(chunk));
                    }
                }
            }
        }

        events
    }

    fn translate_chunk(&mut self, chunk: ChatCompletionChunk) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        // Emit MessageStart on first chunk
        if chunk.choices.is_empty() {
            // Usage-only chunk (some providers send this at the end)
            if let Some(usage) = chunk.usage {
                events.push(StreamEvent::MessageEnd {
                    usage: TokenUsage {
                        input_tokens: usage.prompt_tokens.unwrap_or(0),
                        output_tokens: usage.completion_tokens.unwrap_or(0),
                        reasoning_tokens: usage
                            .completion_tokens_details
                            .as_ref()
                            .and_then(|d| d.reasoning_tokens)
                            .unwrap_or(0),
                        cache_read_tokens: usage
                            .prompt_tokens_details
                            .as_ref()
                            .and_then(|d| d.cached_tokens),
                        cache_write_tokens: None,
                    },
                    stop_reason: StopReason::EndTurn,
                });
            }
            return events;
        }

        for choice in &chunk.choices {
            let delta = &choice.delta;

            // Text content
            if let Some(ref content) = delta.content {
                if !content.is_empty() {
                    events.push(StreamEvent::TextDelta {
                        text: content.clone(),
                    });
                }
            }

            // Reasoning/thinking content (DeepSeek, o3, etc.)
            if let Some(ref reasoning) = delta.reasoning_content {
                if !reasoning.is_empty() {
                    events.push(StreamEvent::ThinkingDelta {
                        text: reasoning.clone(),
                    });
                }
            }

            // Tool calls
            if let Some(ref tool_calls) = delta.tool_calls {
                for tc in tool_calls {
                    if let Some(ref func) = tc.function {
                        // New tool call starting
                        if func.name.is_some() {
                            // Flush previous tool call if any
                            if let Some(event) = self.flush_tool_call() {
                                events.push(event);
                            }

                            let id = tc.id.clone().unwrap_or_default();
                            let name = func.name.clone().unwrap_or_default();

                            events.push(StreamEvent::ToolUseStart {
                                id: id.clone(),
                                name: name.clone(),
                            });

                            self.current_tool_id = Some(id);
                            self.current_tool_name = Some(name);
                            self.current_tool_args.clear();
                        }

                        // Accumulate arguments
                        if let Some(ref args) = func.arguments {
                            self.current_tool_args.push_str(args);
                            events.push(StreamEvent::ToolUseDelta {
                                input_json: args.clone(),
                            });
                        }
                    }
                }
            }

            // Finish reason
            if let Some(ref reason) = choice.finish_reason {
                // Flush any pending tool call
                if let Some(event) = self.flush_tool_call() {
                    events.push(event);
                }

                let stop_reason = match reason.as_str() {
                    "stop" => StopReason::EndTurn,
                    "tool_calls" => StopReason::ToolUse,
                    "length" => StopReason::MaxTokens,
                    "content_filter" => StopReason::StopSequence,
                    _ => StopReason::EndTurn,
                };

                let usage = chunk
                    .usage
                    .as_ref()
                    .map(|u| TokenUsage {
                        input_tokens: u.prompt_tokens.unwrap_or(0),
                        output_tokens: u.completion_tokens.unwrap_or(0),
                        reasoning_tokens: u
                            .completion_tokens_details
                            .as_ref()
                            .and_then(|d| d.reasoning_tokens)
                            .unwrap_or(0),
                        cache_read_tokens: u
                            .prompt_tokens_details
                            .as_ref()
                            .and_then(|d| d.cached_tokens),
                        cache_write_tokens: None,
                    })
                    .unwrap_or_default();

                events.push(StreamEvent::MessageEnd { usage, stop_reason });
            }
        }

        events
    }

    fn flush_tool_call(&mut self) -> Option<StreamEvent> {
        let id = self.current_tool_id.take()?;
        let name = self.current_tool_name.take()?;
        let args = std::mem::take(&mut self.current_tool_args);

        let input =
            serde_json::from_str(&args).unwrap_or(serde_json::Value::Object(Default::default()));

        Some(StreamEvent::ToolUseEnd { id, name, input })
    }
}

// ============================================================================
// OpenAI Response Types (for deserialization only)
// ============================================================================

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    #[serde(default)]
    usage: Option<ChunkUsage>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChunkToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ChunkToolCall {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ChunkFunction>,
}

#[derive(Debug, Deserialize)]
struct ChunkFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
    #[serde(default)]
    completion_tokens_details: Option<CompletionTokensDetails>,
}

#[derive(Debug, Deserialize)]
struct PromptTokensDetails {
    #[serde(default)]
    cached_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CompletionTokensDetails {
    #[serde(default)]
    reasoning_tokens: Option<u64>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_text_delta() {
        let mut parser = OpenAISseParser::new();
        let chunk =
            b"data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n";
        let events = parser.parse(chunk);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::TextDelta { text } if text == "Hello"));
    }

    #[test]
    fn test_parse_done_sentinel() {
        let mut parser = OpenAISseParser::new();
        let chunk = b"data: [DONE]\n\n";
        let events = parser.parse(chunk);
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_finish_reason() {
        let mut parser = OpenAISseParser::new();
        let chunk = b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n";
        let events = parser.parse(chunk);
        assert!(events.iter().any(|e| matches!(
            e,
            StreamEvent::MessageEnd {
                stop_reason: StopReason::EndTurn,
                ..
            }
        )));
    }

    #[test]
    fn test_parse_tool_call() {
        let mut parser = OpenAISseParser::new();
        // Tool call start
        let start = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"id\":\"call_123\",\"function\":{\"name\":\"bash\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n";
        let events = parser.parse(start);
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolUseStart { name, .. } if name == "bash")));

        // Tool call args
        let args = b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"function\":{\"arguments\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}}]},\"finish_reason\":null}]}\n\n";
        let events = parser.parse(args);
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolUseDelta { .. })));

        // Finish with tool_calls
        let finish = b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n";
        let events = parser.parse(finish);
        assert!(events
            .iter()
            .any(|e| matches!(e, StreamEvent::ToolUseEnd { name, .. } if name == "bash")));
    }

    #[test]
    fn test_parse_reasoning_content() {
        let mut parser = OpenAISseParser::new();
        let chunk = b"data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"Let me think...\"},\"finish_reason\":null}]}\n\n";
        let events = parser.parse(chunk);
        assert!(events.iter().any(
            |e| matches!(e, StreamEvent::ThinkingDelta { text } if text == "Let me think...")
        ));
    }
}
