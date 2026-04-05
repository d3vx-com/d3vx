//! SSE Stream Parser for Anthropic API
//!
//! Parses Server-Sent Events (SSE) from Anthropic's streaming API.
//!
//! # SSE Format
//!
//! ```text
//! event: message_start
//! data: {"type":"message_start","message":{...}}
//!
//! event: content_block_delta
//! data: {"type":"content_block_delta","index":0,"delta":{...}}
//!
//! event: message_stop
//! data: {}
//! ```
//!
//! Events are separated by double newlines (\n\n).

use super::types::{AnthropicStreamEvent, ContentBlockStart, ContentDelta};
use crate::providers::{ProviderError, StopReason, StreamEvent, TokenUsage};
use std::collections::HashMap;

/// SSE event parsed from the stream.
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// Event type (e.g., "message_start", "content_block_delta")
    pub event_type: String,
    /// Event data (JSON string)
    pub data: String,
}

/// Parser for Server-Sent Events.
///
/// Handles incremental parsing of SSE streams, buffering partial data
/// until complete events are received.
#[derive(Debug, Default)]
pub struct SseParser {
    /// Buffer for incomplete data
    buffer: Vec<u8>,
    /// Current event type being parsed
    current_event: Option<String>,
    /// Current event data lines
    current_data: Vec<String>,
}

impl SseParser {
    /// Create a new SSE parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a chunk of SSE data and return complete events.
    ///
    /// This method handles:
    /// - Buffering partial data
    /// - Parsing event and data lines
    /// - Combining multi-line data values
    pub fn parse(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        let mut events = Vec::new();

        // Append new data to buffer
        self.buffer.extend_from_slice(chunk);

        // Trace raw chunk for debugging empty responses
        tracing::trace!("SSE chunk received: {} bytes", chunk.len());
        if tracing::enabled!(tracing::Level::TRACE) {
            tracing::trace!("Raw SSE chunk: {:?}", String::from_utf8_lossy(chunk));
        }

        // Process complete lines
        while let Some(line_end) = self.find_line_end() {
            let line_bytes: Vec<u8> = self.buffer.drain(..=line_end).collect();
            let line = String::from_utf8_lossy(&line_bytes);
            let line = line.trim_end();

            tracing::trace!("SSE line parsed: {:?}", line);

            // Empty line signals end of event
            if line.is_empty() {
                if let Some(event_type) = self.current_event.take() {
                    let data = self.current_data.join("\n");
                    self.current_data.clear();

                    events.push(SseEvent { event_type, data });
                }
                continue;
            }

            // Parse field: value format
            if let Some((field, value)) = self.parse_field(line) {
                match field {
                    "event" => {
                        self.current_event = Some(value.to_string());
                    }
                    "data" => {
                        self.current_data.push(value.to_string());
                    }
                    "id" | "retry" => {
                        // Ignore these fields for now
                    }
                    _ => {
                        // Unknown field, ignore
                    }
                }
            }
        }

        events
    }

    /// Find the end of the next line in the buffer.
    fn find_line_end(&self) -> Option<usize> {
        self.buffer.iter().position(|&b| b == b'\n')
    }

    /// Parse a "field: value" line.
    fn parse_field<'a>(&self, line: &'a str) -> Option<(&'a str, &'a str)> {
        let colon_pos = line.find(':')?;
        let field = &line[..colon_pos];
        let value = &line[colon_pos + 1..];
        let value = value.trim_start(); // Remove leading space after colon
        Some((field, value))
    }
}

/// Convert Anthropic SSE events to unified StreamEvents.
///
/// Tracks state for assembling tool_use blocks from streaming deltas.
pub struct EventTranslator {
    /// Currently active tool use blocks (index -> (id, name, accumulated_json))
    tool_blocks: HashMap<u32, (String, String, String)>,
}

impl EventTranslator {
    /// Create a new event translator.
    pub fn new() -> Self {
        Self {
            tool_blocks: HashMap::new(),
        }
    }

    /// Translate an Anthropic stream event to unified StreamEvents.
    ///
    /// Returns multiple events for some Anthropic events (e.g., content_block_stop
    /// may emit a tool_use_end event).
    pub fn translate(&mut self, event: AnthropicStreamEvent) -> Vec<StreamEvent> {
        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                vec![StreamEvent::MessageStart {
                    id: message.id,
                    model: message.model,
                    usage: TokenUsage {
                        input_tokens: message.usage.input_tokens,
                        output_tokens: message.usage.output_tokens,
                        reasoning_tokens: 0,
                        cache_read_tokens: message.usage.cache_read_input_tokens,
                        cache_write_tokens: message.usage.cache_creation_input_tokens,
                    },
                }]
            }

            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                match content_block {
                    ContentBlockStart::ToolUse { id, name } => {
                        // Track this tool use block
                        self.tool_blocks
                            .insert(index, (id.clone(), name.clone(), String::new()));

                        vec![StreamEvent::ToolUseStart { id, name }]
                    }
                    ContentBlockStart::Text { .. } | ContentBlockStart::Thinking { .. } => {
                        // Text and thinking blocks don't emit start events
                        Vec::new()
                    }
                }
            }

            AnthropicStreamEvent::ContentBlockDelta { index, delta } => {
                match delta {
                    ContentDelta::TextDelta { text } => {
                        vec![StreamEvent::TextDelta { text }]
                    }
                    ContentDelta::ThinkingDelta { thinking } => {
                        vec![StreamEvent::ThinkingDelta { text: thinking }]
                    }
                    ContentDelta::InputJsonDelta { partial_json } => {
                        // Accumulate JSON for this tool use
                        if let Some((_, _, ref mut json)) = self.tool_blocks.get_mut(&index) {
                            json.push_str(&partial_json);
                        }

                        vec![StreamEvent::ToolUseDelta {
                            input_json: partial_json,
                        }]
                    }
                }
            }

            AnthropicStreamEvent::ContentBlockStop { index } => {
                // If we have a tool use block at this index, emit tool_use_end
                if let Some((id, name, json)) = self.tool_blocks.remove(&index) {
                    let input = serde_json::from_str(&json)
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                    vec![StreamEvent::ToolUseEnd { id, name, input }]
                } else {
                    Vec::new()
                }
            }

            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                let stop_reason = match delta.stop_reason.as_deref() {
                    Some("end_turn") => StopReason::EndTurn,
                    Some("tool_use") => StopReason::ToolUse,
                    Some("max_tokens") => StopReason::MaxTokens,
                    Some("stop_sequence") => StopReason::StopSequence,
                    _ => StopReason::EndTurn,
                };

                vec![StreamEvent::MessageEnd {
                    usage: TokenUsage {
                        input_tokens: 0, // Input tokens come from message_start
                        output_tokens: usage.output_tokens,
                        reasoning_tokens: 0, // Anthropic includes thinking in output_tokens
                        cache_read_tokens: None,
                        cache_write_tokens: None,
                    },
                    stop_reason,
                }]
            }

            AnthropicStreamEvent::MessageStop => {
                // No event needed - message_end already sent
                Vec::new()
            }

            AnthropicStreamEvent::Ping => {
                // Ignore ping events
                Vec::new()
            }

            AnthropicStreamEvent::Error { error } => {
                vec![StreamEvent::Error {
                    error: ProviderError::StreamError(format!(
                        "{}: {}",
                        error.error_type, error.message
                    )),
                }]
            }
        }
    }
}

impl Default for EventTranslator {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a single SSE event from raw event type and data.
///
/// Returns `None` if the data is not valid JSON.
pub fn parse_anthropic_event(event_type: &str, data: &str) -> Option<AnthropicStreamEvent> {
    if data.is_empty() || data == "{}" {
        // Handle empty data (e.g., message_stop)
        return match event_type {
            "message_stop" => Some(AnthropicStreamEvent::MessageStop),
            "ping" => Some(AnthropicStreamEvent::Ping),
            _ => None,
        };
    }

    serde_json::from_str(data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_event() {
        let mut parser = SseParser::new();

        let chunk = b"event: message_start\ndata: {\"type\":\"message_start\"}\n\n";
        let events = parser.parse(chunk);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "message_start");
        assert_eq!(events[0].data, r#"{"type":"message_start"}"#);
    }

    #[test]
    fn test_parse_multiple_events() {
        let mut parser = SseParser::new();

        let chunk = b"event: message_start\ndata: {\"type\":\"message_start\"}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\"}\n\n";
        let events = parser.parse(chunk);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "message_start");
        assert_eq!(events[1].event_type, "content_block_delta");
    }

    #[test]
    fn test_parse_chunked_event() {
        let mut parser = SseParser::new();

        // Send partial data
        let chunk1 = b"event: message_start\ndata: {\"type\":";
        let events1 = parser.parse(chunk1);
        assert!(events1.is_empty()); // Not complete yet

        // Send rest of data
        let chunk2 = b"\"message_start\"}\n\n";
        let events2 = parser.parse(chunk2);
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].event_type, "message_start");
    }

    #[test]
    fn test_parse_multiline_data() {
        let mut parser = SseParser::new();

        // SSE allows multiple data lines that get joined with newlines
        let chunk = b"event: message\ndata: line1\ndata: line2\n\n";
        let events = parser.parse(chunk);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2");
    }

    #[test]
    fn test_translate_message_start() {
        let mut translator = EventTranslator::new();

        let json = r#"{
            "type": "message_start",
            "message": {
                "id": "msg_123",
                "type": "message",
                "role": "assistant",
                "model": "claude-sonnet-4-20250514",
                "content": [],
                "stop_reason": null,
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 0
                }
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();
        let stream_events = translator.translate(event);

        assert_eq!(stream_events.len(), 1);
        match &stream_events[0] {
            StreamEvent::MessageStart {
                id,
                model,
                usage: _,
            } => {
                assert_eq!(id, "msg_123");
                assert_eq!(model, "claude-sonnet-4-20250514");
            }
            _ => panic!("Expected MessageStart"),
        }
    }

    #[test]
    fn test_translate_text_delta() {
        let mut translator = EventTranslator::new();

        let json = r#"{
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "text_delta",
                "text": "Hello"
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(json).unwrap();
        let stream_events = translator.translate(event);

        assert_eq!(stream_events.len(), 1);
        match &stream_events[0] {
            StreamEvent::TextDelta { text } => {
                assert_eq!(text, "Hello");
            }
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_translate_tool_use() {
        let mut translator = EventTranslator::new();

        // Tool use start
        let start_json = r#"{
            "type": "content_block_start",
            "index": 0,
            "content_block": {
                "type": "tool_use",
                "id": "tool_123",
                "name": "read_file"
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(start_json).unwrap();
        let events = translator.translate(event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolUseStart { id, name } => {
                assert_eq!(id, "tool_123");
                assert_eq!(name, "read_file");
            }
            _ => panic!("Expected ToolUseStart"),
        }

        // Tool use delta
        let delta_json = r#"{
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "input_json_delta",
                "partial_json": "{\"path\":"
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(delta_json).unwrap();
        let events = translator.translate(event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolUseDelta { input_json } => {
                assert_eq!(input_json, "{\"path\":");
            }
            _ => panic!("Expected ToolUseDelta"),
        }

        // More delta
        let delta_json2 = r#"{
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "input_json_delta",
                "partial_json": " \"/src/main.rs\"}"
            }
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(delta_json2).unwrap();
        translator.translate(event);

        // Tool use stop
        let stop_json = r#"{
            "type": "content_block_stop",
            "index": 0
        }"#;

        let event: AnthropicStreamEvent = serde_json::from_str(stop_json).unwrap();
        let events = translator.translate(event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolUseEnd { id, name, input } => {
                assert_eq!(id, "tool_123");
                assert_eq!(name, "read_file");
                assert_eq!(input["path"], "/src/main.rs");
            }
            _ => panic!("Expected ToolUseEnd"),
        }
    }
}
