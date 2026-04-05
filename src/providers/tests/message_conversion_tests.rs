//! Tests for OpenAI Message Conversion
//!
//! Covers the conversion from d3vx message format to OpenAI API format.

#[cfg(test)]
mod tests {
    use crate::providers::openai_compatible::openai_config;
    use crate::providers::MessagesRequest;
    use crate::providers::{ContentBlock, Message, MessageContent, Role};
    use serde_json::json;

    // =========================================================================
    // Simple Text Message Conversion Tests
    // =========================================================================

    #[test]
    fn test_simple_user_message() {
        let msg = Message::user_text("Hello");
        assert_eq!(msg.role, Role::User);
        assert!(matches!(msg.content, MessageContent::Text(_)));
    }

    #[test]
    fn test_simple_assistant_message() {
        let msg = Message::assistant_text("Hi there!");
        assert_eq!(msg.role, Role::Assistant);
        assert!(matches!(msg.content, MessageContent::Text(_)));
    }

    // =========================================================================
    // Tool Use Message Conversion Tests
    // =========================================================================

    #[test]
    fn test_assistant_tool_use_blocks() {
        let blocks = vec![
            ContentBlock::text("I'll run that command."),
            ContentBlock::tool_use("call-123", "bash", json!({"command": "ls"})),
        ];

        let msg = Message::assistant_blocks(blocks);
        assert_eq!(msg.role, Role::Assistant);

        let blocks = msg.as_blocks().unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].is_text());
        assert!(blocks[1].is_tool_use());
    }

    #[test]
    fn test_user_tool_result_blocks() {
        let blocks = vec![ContentBlock::tool_result(
            "call-123",
            "file1.txt\nfile2.txt",
        )];

        let msg = Message::user_blocks(blocks);
        assert_eq!(msg.role, Role::User);

        let blocks = msg.as_blocks().unwrap();
        assert!(blocks[0].is_tool_result());
    }

    #[test]
    fn test_tool_error_result() {
        let blocks = vec![ContentBlock::tool_error(
            "call-456",
            "Command failed with exit code 1",
        )];

        let msg = Message::user_blocks(blocks);
        let blocks = msg.as_blocks().unwrap();

        if let ContentBlock::ToolResult { is_error, .. } = &blocks[0] {
            assert_eq!(*is_error, Some(true));
        } else {
            panic!("Expected ToolResult");
        }
    }

    // =========================================================================
    // Mixed Content Tests
    // =========================================================================

    #[test]
    fn test_mixed_text_and_tool_blocks() {
        let blocks = vec![
            ContentBlock::text("First text"),
            ContentBlock::tool_use("call-1", "read", json!({"path": "/test"})),
            ContentBlock::text("Second text"),
            ContentBlock::tool_result("call-1", "file contents"),
        ];

        // Assistant with tool use
        let assistant_msg = Message::assistant_blocks(blocks[..3].to_vec());
        assert_eq!(assistant_msg.as_blocks().unwrap().len(), 3);

        // User with tool result
        let user_msg = Message::user_blocks(vec![blocks[3].clone()]);
        assert_eq!(user_msg.as_blocks().unwrap().len(), 1);
    }

    // =========================================================================
    // Request Building Tests
    // =========================================================================

    #[test]
    fn test_request_default() {
        let _config = openai_config("test-key".to_string(), None);
        let request = MessagesRequest::default();
        assert!(request.model.is_empty());
        assert!(request.messages.is_empty());
        assert!(request.system_prompt.is_none());
        assert!(request.tools.is_empty());
        assert!(request.max_tokens.is_none());
        assert!(request.temperature.is_none());
        assert!(request.thinking.is_none());
    }

    #[test]
    fn test_request_with_system_prompt() {
        let request = MessagesRequest {
            model: "gpt-4o".to_string(),
            messages: vec![Message::user_text("Hello")],
            system_prompt: Some("You are a helpful assistant.".to_string()),
            tools: vec![],
            max_tokens: Some(1000),
            temperature: Some(0.7),
            thinking: None,
            prompt_caching: true,
        };

        assert_eq!(request.model, "gpt-4o");
        assert!(request.system_prompt.is_some());
        assert_eq!(request.max_tokens, Some(1000));
        assert_eq!(request.temperature, Some(0.7));
    }

    // =========================================================================
    // Serialization Round-Trip Tests
    // =========================================================================

    #[test]
    fn test_message_roundtrip() {
        let original = Message::user_blocks(vec![
            ContentBlock::text("Check this file"),
            ContentBlock::tool_use("id-1", "read", json!({"path": "/tmp/test.txt"})),
        ]);

        let json = serde_json::to_string(&original).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.role, original.role);

        let original_blocks = original.as_blocks().unwrap();
        let parsed_blocks = parsed.as_blocks().unwrap();
        assert_eq!(original_blocks.len(), parsed_blocks.len());
    }

    #[test]
    fn test_tool_use_block_roundtrip() {
        let block = ContentBlock::tool_use(
            "call-xyz",
            "bash",
            json!({
                "command": "echo hello",
                "timeout": 30000
            }),
        );

        let json = serde_json::to_string(&block).unwrap();
        let parsed: ContentBlock = serde_json::from_str(&json).unwrap();

        if let ContentBlock::ToolUse { id, name, input } = parsed {
            assert_eq!(id, "call-xyz");
            assert_eq!(name, "bash");
            assert_eq!(input["command"], "echo hello");
        } else {
            panic!("Expected ToolUse");
        }
    }
}
