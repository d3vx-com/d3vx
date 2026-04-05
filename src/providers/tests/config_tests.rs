//! Tests for Provider Configuration and Core Types
//!
//! Covers Message, ContentBlock, Role, and other core provider types.

#[cfg(test)]
mod tests {
    use crate::providers::types::ToolParameter;
    use crate::providers::{
        ContentBlock, Message, MessageContent, Role, ToolDefinition, ToolSchema,
    };
    use serde_json::json;
    use std::collections::HashMap;

    // =========================================================================
    // Message Tests
    // =========================================================================

    #[test]
    fn test_message_user_text() {
        let msg = Message::user_text("Hello, world!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.as_text(), Some("Hello, world!"));
    }

    #[test]
    fn test_message_assistant_text() {
        let msg = Message::assistant_text("Hello back!");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.as_text(), Some("Hello back!"));
    }

    #[test]
    fn test_message_with_blocks() {
        let blocks = vec![
            ContentBlock::text("First part"),
            ContentBlock::text("Second part"),
        ];
        let msg = Message::user_blocks(blocks);

        assert_eq!(msg.role, Role::User);
        assert!(msg.as_blocks().is_some());
        assert_eq!(msg.as_blocks().unwrap().len(), 2);
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user_text("Test message");
        let json = serde_json::to_string(&msg).unwrap();

        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"Test message\""));
    }

    #[test]
    fn test_message_deserialization() {
        let json = r#"{"role":"assistant","content":"Response"}"#;
        let msg: Message = serde_json::from_str(json).unwrap();

        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.as_text(), Some("Response"));
    }

    // =========================================================================
    // ContentBlock Tests
    // =========================================================================

    #[test]
    fn test_content_block_text() {
        let block = ContentBlock::text("Hello");
        assert!(block.is_text());
        assert!(!block.is_tool_use());
        assert!(!block.is_tool_result());
        assert_eq!(block.as_text(), Some("Hello"));
    }

    #[test]
    fn test_content_block_thinking() {
        let block = ContentBlock::thinking("Let me think...");
        assert!(matches!(block, ContentBlock::Thinking { .. }));
    }

    #[test]
    fn test_content_block_tool_use() {
        let input = json!({"command": "ls"});
        let block = ContentBlock::tool_use("tool-123", "bash", input.clone());

        assert!(block.is_tool_use());
        assert!(!block.is_text());

        if let ContentBlock::ToolUse {
            id,
            name,
            input: tool_input,
        } = block
        {
            assert_eq!(id, "tool-123");
            assert_eq!(name, "bash");
            assert_eq!(tool_input, input);
        } else {
            panic!("Expected ToolUse variant");
        }
    }

    #[test]
    fn test_content_block_tool_result() {
        let block = ContentBlock::tool_result("tool-123", "Output here");
        assert!(block.is_tool_result());
    }

    #[test]
    fn test_content_block_tool_error() {
        let block = ContentBlock::tool_error("tool-123", "Error message");

        if let ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } = block
        {
            assert_eq!(tool_use_id, "tool-123");
            assert_eq!(content, "Error message");
            assert_eq!(is_error, Some(true));
        } else {
            panic!("Expected ToolResult variant");
        }
    }

    #[test]
    fn test_content_block_serialization() {
        let block = ContentBlock::text("Test");
        let json = serde_json::to_string(&block).unwrap();

        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Test\""));
    }

    // =========================================================================
    // Role Tests
    // =========================================================================

    #[test]
    fn test_role_serialization() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
    }

    #[test]
    fn test_role_deserialization() {
        let user: Role = serde_json::from_str("\"user\"").unwrap();
        assert_eq!(user, Role::User);

        let assistant: Role = serde_json::from_str("\"assistant\"").unwrap();
        assert_eq!(assistant, Role::Assistant);
    }

    // =========================================================================
    // MessageContent Tests
    // =========================================================================

    #[test]
    fn test_message_content_as_text() {
        let text_content = MessageContent::Text("Simple text".to_string());
        assert_eq!(text_content.as_text(), Some("Simple text"));
    }

    #[test]
    fn test_message_content_blocks_as_text() {
        let blocks = vec![
            ContentBlock::text("First"),
            ContentBlock::thinking("Thinking..."),
            ContentBlock::text("Second"),
        ];
        let content = MessageContent::Blocks(blocks);

        // Should return the first text block
        assert_eq!(content.as_text(), Some("First"));
    }

    // =========================================================================
    // ToolDefinition Tests
    // =========================================================================

    #[test]
    fn test_tool_definition_creation() {
        let mut properties = HashMap::new();
        properties.insert(
            "path".to_string(),
            ToolParameter {
                param_type: "string".to_string(),
                description: Some("The file path".to_string()),
                enum_values: None,
                items: None,
                default: None,
            },
        );

        let tool = ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file from disk".to_string(),
            input_schema: ToolSchema {
                schema_type: "object".to_string(),
                properties,
                required: Some(vec!["path".to_string()]),
            },
        };

        assert_eq!(tool.name, "read_file");
        assert!(tool.input_schema.properties.contains_key("path"));
    }

    #[test]
    fn test_tool_schema_serialization() {
        let schema = ToolSchema {
            schema_type: "object".to_string(),
            properties: HashMap::new(),
            required: None,
        };

        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("\"type\":\"object\""));
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_empty_message_text() {
        let msg = Message::user_text("");
        assert_eq!(msg.as_text(), Some(""));
    }

    #[test]
    fn test_empty_blocks() {
        let msg = Message::user_blocks(vec![]);
        assert!(msg.as_blocks().is_some());
        assert!(msg.as_blocks().unwrap().is_empty());
    }

    #[test]
    fn test_complex_nested_content() {
        let blocks = vec![
            ContentBlock::text("Here's the result:"),
            ContentBlock::tool_result("call-1", "Success"),
            ContentBlock::text("And another:"),
            ContentBlock::tool_error("call-2", "Failed"),
        ];

        let msg = Message::assistant_blocks(blocks);
        assert!(msg.as_blocks().is_some());
        assert_eq!(msg.as_blocks().unwrap().len(), 4);
    }
}
