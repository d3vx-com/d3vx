//! Message types for LLM communication
//!
//! Messages are the primary unit of communication with LLM providers.
//! Each message has a role (user/assistant) and content blocks.

use serde::{Deserialize, Serialize};

use super::content::ContentBlock;

/// Message role indicating who sent the message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
        }
    }
}

/// A message in the conversation
///
/// Messages can have either a simple string content or structured content blocks.
/// For API compatibility, content is serialized as either a string or array
/// depending on what was set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message author
    pub role: Role,
    /// The content of the message - either text or structured blocks
    pub content: MessageContent,
}

/// Message content can be either simple text or structured blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content
    Text(String),
    /// Structured content blocks
    Blocks(Vec<ContentBlock>),
}

impl Message {
    /// Create a new user message with text content
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(text.into()),
        }
    }

    /// Create a new assistant message with text content
    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Text(text.into()),
        }
    }

    /// Create a new user message with content blocks
    pub fn user_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Blocks(blocks),
        }
    }

    /// Create a new assistant message with content blocks
    pub fn assistant_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Blocks(blocks),
        }
    }

    /// Get the message as text if it's simple text content
    pub fn as_text(&self) -> Option<&str> {
        match &self.content {
            MessageContent::Text(text) => Some(text),
            MessageContent::Blocks(_) => None,
        }
    }

    /// Get the content blocks if this is structured content
    pub fn as_blocks(&self) -> Option<&[ContentBlock]> {
        match &self.content {
            MessageContent::Text(_) => None,
            MessageContent::Blocks(blocks) => Some(blocks),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_serialization() {
        let user = Role::User;
        let assistant = Role::Assistant;

        assert_eq!(serde_json::to_string(&user).unwrap(), r#""user""#);
        assert_eq!(serde_json::to_string(&assistant).unwrap(), r#""assistant""#);
    }

    #[test]
    fn test_message_user_text() {
        let msg = Message::user_text("Hello, world!");
        let json = serde_json::to_string(&msg).unwrap();

        assert!(json.contains(r#""role":"user""#));
        assert!(json.contains(r#""Hello, world!""#));
    }

    #[test]
    fn test_message_assistant_text() {
        let msg = Message::assistant_text("Hello!");
        let json = serde_json::to_string(&msg).unwrap();

        assert!(json.contains(r#""role":"assistant""#));
        assert!(json.contains(r#""Hello!""#));
    }

    #[test]
    fn test_message_deserialization() {
        let json = r#"{"role":"user","content":"Hello"}"#;
        let msg: Message = serde_json::from_str(json).unwrap();

        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.as_text(), Some("Hello"));
    }
}
