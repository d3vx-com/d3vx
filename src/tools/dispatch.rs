//! Relay Message Tool
//!
//! Provides message routing between agents in a coordinated swarm.

use crate::team::coordinator::get_swarm;
use crate::team::message_bus::{SwarmMessage, SwarmMessageType};
use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde_json::json;

// ---------------------------------------------------------------------------
// SwarmMessageType parsing
// ---------------------------------------------------------------------------

/// Parse the message_type string from tool input into a `SwarmMessageType`.
fn parse_message_type(raw: &str) -> Option<SwarmMessageType> {
    match raw {
        "text" => Some(SwarmMessageType::Text),
        "task_claim" => Some(SwarmMessageType::TaskClaim),
        "task_complete" => Some(SwarmMessageType::TaskComplete),
        "shutdown_request" => Some(SwarmMessageType::ShutdownRequest),
        "shutdown_response" => Some(SwarmMessageType::ShutdownResponse { approve: true }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// RelayMessageTool
// ---------------------------------------------------------------------------

/// Tool that routes messages between agents inside a coordinated swarm.
#[derive(Clone, Default)]
pub struct RelayMessageTool;

impl RelayMessageTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for RelayMessageTool {
    fn name(&self) -> String {
        "relay_message".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Route a message to another team member in the same swarm. \
                Use call sign for direct messages or '*' for broadcast."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "description": "Recipient call sign, or '*' for broadcast"
                    },
                    "body": {
                        "type": "string",
                        "description": "Message content"
                    },
                    "message_type": {
                        "type": "string",
                        "enum": [
                            "text",
                            "task_claim",
                            "task_complete",
                            "shutdown_request",
                            "shutdown_response"
                        ],
                        "description": "Type of message (default: text)"
                    }
                },
                "required": ["to", "body"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        // -- Extract required fields ------------------------------------------------

        let to = match input.get("to").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return ToolResult::error("Missing required field: 'to'"),
        };

        let body = match input.get("body").and_then(|v| v.as_str()) {
            Some(b) => b.to_string(),
            None => return ToolResult::error("Missing required field: 'body'"),
        };

        let message_type_str = input
            .get("message_type")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        let message_type = match parse_message_type(message_type_str) {
            Some(mt) => mt,
            None => {
                return ToolResult::error(format!(
                    "Invalid message_type '{}'. \
                     Valid values: text, task_claim, task_complete, shutdown_request, shutdown_response",
                    message_type_str
                ));
            }
        };

        // -- Validate swarm membership ----------------------------------------------

        let swarm = match &context.swarm_membership {
            Some(s) => s,
            None => {
                return ToolResult::error(
                    "Not running inside a swarm. This tool requires swarm membership.",
                );
            }
        };

        // -- Resolve the swarm coordinator ------------------------------------------

        let bus = match get_swarm(&swarm.swarm_name) {
            Some(b) => b,
            None => {
                return ToolResult::error(format!(
                    "Swarm '{}' not found or no longer active.",
                    swarm.swarm_name
                ));
            }
        };

        // -- Build and send the message ---------------------------------------------

        let message = SwarmMessage::new(&swarm.call_sign, &to, body.clone(), message_type);

        match bus.send(message).await {
            Ok(()) => {
                let broadcast_label = if to == "*" {
                    "broadcast to all members"
                } else {
                    "direct message"
                };
                ToolResult::success(
                    json!({
                        "status": "delivered",
                        "from": swarm.call_sign,
                        "to": to,
                        "message_type": message_type_str,
                        "delivery": broadcast_label,
                    })
                    .to_string(),
                )
            }
            Err(e) => ToolResult::error(format!("Failed to deliver message: {}", e)),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::types::SwarmContext;
    use std::collections::HashMap;

    fn make_context(swarm: Option<SwarmContext>) -> ToolContext {
        ToolContext {
            cwd: "/tmp".to_string(),
            env: HashMap::new(),
            trust_mode: false,
            session_id: Some("test-session".to_string()),
            parent_session_id: None,
            agent_depth: 0,
            allow_parallel_spawn: true,
            bash_blocklist: vec![],
            sandbox_mode: crate::config::types::SandboxMode::Disabled,
            sandbox_config: None,
            swarm_membership: swarm,
        }
    }

    fn sample_swarm() -> SwarmContext {
        SwarmContext {
            swarm_name: "swarm-1".to_string(),
            call_sign: "backend-1".to_string(),
            is_lead: false,
        }
    }

    #[test]
    fn name_returns_relay_message() {
        let tool = RelayMessageTool::new();
        assert_eq!(tool.name(), "relay_message");
    }

    #[test]
    fn definition_has_required_fields() {
        let tool = RelayMessageTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "relay_message");
        let schema = &def.input_schema;
        let required = schema["required"].as_array().expect("required array");
        assert!(required.iter().any(|r| r == "to"));
        assert!(required.iter().any(|r| r == "body"));
    }

    #[tokio::test]
    async fn execute_returns_error_without_swarm_membership() {
        let tool = RelayMessageTool::new();
        let ctx = make_context(None);
        let input = json!({"to": "lead", "body": "hello"});

        let result = tool.execute(input, &ctx).await;
        assert!(result.is_error);
        assert!(result.content.contains("Not running inside a swarm"));
    }

    #[tokio::test]
    async fn execute_returns_error_when_swarm_not_found() {
        let tool = RelayMessageTool::new();
        let ctx = make_context(Some(sample_swarm()));
        let input = json!({"to": "lead", "body": "hello"});

        let result = tool.execute(input, &ctx).await;
        assert!(result.is_error);
        assert!(result.content.contains("not found or no longer active"));
    }

    #[tokio::test]
    async fn execute_returns_error_for_missing_to_field() {
        let tool = RelayMessageTool::new();
        let ctx = make_context(Some(sample_swarm()));
        let input = json!({"body": "hello"});

        let result = tool.execute(input, &ctx).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required field: 'to'"));
    }

    #[tokio::test]
    async fn execute_returns_error_for_missing_body_field() {
        let tool = RelayMessageTool::new();
        let ctx = make_context(Some(sample_swarm()));
        let input = json!({"to": "lead"});

        let result = tool.execute(input, &ctx).await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required field: 'body'"));
    }

    #[tokio::test]
    async fn execute_returns_error_for_invalid_message_type() {
        let tool = RelayMessageTool::new();
        let ctx = make_context(Some(sample_swarm()));
        let input = json!({"to": "lead", "body": "hi", "message_type": "bogus"});

        let result = tool.execute(input, &ctx).await;
        assert!(result.is_error);
        assert!(result.content.contains("Invalid message_type"));
    }

    #[test]
    fn parse_message_type_handles_all_variants() {
        assert!(matches!(
            parse_message_type("text"),
            Some(SwarmMessageType::Text)
        ));
        assert!(matches!(
            parse_message_type("task_claim"),
            Some(SwarmMessageType::TaskClaim)
        ));
        assert!(matches!(
            parse_message_type("task_complete"),
            Some(SwarmMessageType::TaskComplete)
        ));
        assert!(matches!(
            parse_message_type("shutdown_request"),
            Some(SwarmMessageType::ShutdownRequest)
        ));
        assert!(matches!(
            parse_message_type("shutdown_response"),
            Some(SwarmMessageType::ShutdownResponse { .. })
        ));
        assert!(parse_message_type("unknown").is_none());
    }
}
