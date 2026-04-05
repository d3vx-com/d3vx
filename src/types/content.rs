//! Content block types for message content
//!
//! Content blocks represent structured content within messages.
//! This includes text, images, tool use/results, and thinking blocks.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Image source for image content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    /// The type of image source (currently only "base64")
    #[serde(rename = "type")]
    pub source_type: String,
    /// The media type (e.g., "image/png", "image/jpeg")
    pub media_type: String,
    /// The base64-encoded image data
    pub data: String,
}

impl ImageSource {
    /// Create a new base64 image source
    pub fn base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            source_type: "base64".to_string(),
            media_type: media_type.into(),
            data: data.into(),
        }
    }
}

/// A content block within a message
///
/// Content blocks are the structured units that make up message content.
/// They support various types including text, images, tool interactions, and thinking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content
    Text { text: String },

    /// Thinking/reasoning content (extended thinking models)
    Thinking { thinking: String },

    /// Image content
    Image { source: ImageSource },

    /// Tool use request from the assistant
    ToolUse {
        /// Unique identifier for this tool use
        id: String,
        /// Name of the tool to invoke
        name: String,
        /// Input parameters for the tool
        input: Value,
    },

    /// Tool result from tool execution
    ToolResult {
        /// ID of the tool use this is responding to
        tool_use_id: String,
        /// The result content
        content: String,
        /// Whether this result represents an error
        #[serde(default)]
        is_error: bool,
    },
}

impl ContentBlock {
    /// Create a new text content block
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create a new thinking content block
    pub fn thinking(thinking: impl Into<String>) -> Self {
        Self::Thinking {
            thinking: thinking.into(),
        }
    }

    /// Create a new image content block
    pub fn image(source: ImageSource) -> Self {
        Self::Image { source }
    }

    /// Create a new tool use content block
    pub fn tool_use(id: impl Into<String>, name: impl Into<String>, input: Value) -> Self {
        Self::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }

    /// Create a new tool result content block
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    /// Create a new tool error result content block
    pub fn tool_error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: true,
        }
    }

    /// Check if this is a text block
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. })
    }

    /// Check if this is a thinking block
    pub fn is_thinking(&self) -> bool {
        matches!(self, Self::Thinking { .. })
    }

    /// Check if this is a tool use block
    pub fn is_tool_use(&self) -> bool {
        matches!(self, Self::ToolUse { .. })
    }

    /// Check if this is a tool result block
    pub fn is_tool_result(&self) -> bool {
        matches!(self, Self::ToolResult { .. })
    }

    /// Get text content if this is a text block
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }

    /// Get thinking content if this is a thinking block
    pub fn as_thinking(&self) -> Option<&str> {
        match self {
            Self::Thinking { thinking } => Some(thinking),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_text_block_serialization() {
        let block = ContentBlock::text("Hello, world!");
        let json = serde_json::to_string(&block).unwrap();

        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""text":"Hello, world!""#));
    }

    #[test]
    fn test_thinking_block_serialization() {
        let block = ContentBlock::thinking("Thinking...");
        let json = serde_json::to_string(&block).unwrap();

        assert!(json.contains(r#""type":"thinking""#));
        assert!(json.contains(r#""thinking":"Thinking...""#));
    }

    #[test]
    fn test_tool_use_block_serialization() {
        let block = ContentBlock::tool_use("tool_123", "read_file", json!({"path": "/test.txt"}));
        let json = serde_json::to_string(&block).unwrap();

        assert!(json.contains(r#""type":"tool_use""#));
        assert!(json.contains(r#""id":"tool_123""#));
        assert!(json.contains(r#""name":"read_file""#));
    }

    #[test]
    fn test_tool_result_block_serialization() {
        let block = ContentBlock::tool_result("tool_123", "File contents here");
        let json = serde_json::to_string(&block).unwrap();

        assert!(json.contains(r#""type":"tool_result""#));
        assert!(json.contains(r#""tool_use_id":"tool_123""#));
        assert!(json.contains(r#""content":"File contents here""#));
        assert!(json.contains(r#""is_error":false"#));
    }

    #[test]
    fn test_tool_error_block_serialization() {
        let block = ContentBlock::tool_error("tool_123", "File not found");
        let json = serde_json::to_string(&block).unwrap();

        assert!(json.contains(r#""is_error":true"#));
    }

    #[test]
    fn test_image_block_serialization() {
        let source = ImageSource::base64("image/png", "base64data");
        let block = ContentBlock::image(source);
        let json = serde_json::to_string(&block).unwrap();

        assert!(json.contains(r#""type":"image""#));
        assert!(json.contains(r#""media_type":"image/png""#));
        assert!(json.contains(r#""data":"base64data""#));
    }

    #[test]
    fn test_block_deserialization() {
        let json = r#"{"type":"text","text":"Hello"}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();

        assert!(block.is_text());
        assert_eq!(block.as_text(), Some("Hello"));
    }
}
