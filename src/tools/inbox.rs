//! Inbox Communication Tools
//!
//! Provides the `SendInboxMessageTool` and `ReadInboxTool` allowing
//! asynchronous, file-backed communication between parallel sub-agents.
//!
//! Additionally integrates with the in-memory `MailboxRegistry` for
//! fast cross-agent direct messaging when both agents are registered.

use crate::agent::subagent::mailbox;
use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

fn get_inbox_dir(working_dir: &str, agent_id: &str) -> PathBuf {
    let mut path = PathBuf::from(working_dir);
    path.push(".d3vx");
    path.push("inboxes");
    path.push(agent_id);
    path
}

/// Tool to send a message to another agent's inbox
#[derive(Clone, Default)]
pub struct SendInboxMessageTool {
    sender: Option<Arc<std::sync::Mutex<mpsc::Sender<crate::event::Event>>>>,
}

impl SendInboxMessageTool {
    pub fn new() -> Self {
        Self { sender: None }
    }

    pub fn with_sender(sender: mpsc::Sender<crate::event::Event>) -> Self {
        Self {
            sender: Some(Arc::new(std::sync::Mutex::new(sender))),
        }
    }

    pub fn set_sender(&mut self, sender: mpsc::Sender<crate::event::Event>) {
        self.sender = Some(Arc::new(std::sync::Mutex::new(sender)));
    }
}

#[async_trait]
impl Tool for SendInboxMessageTool {
    fn name(&self) -> String {
        "send_inbox_message".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: "Send an asynchronous message to another agent's inbox. Use this to notify the Tech Lead or other specialized agents about task completion, bugs, or requests.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "to_agent": {
                        "type": "string",
                        "description": "The ID or role of the target agent (e.g., 'tech_lead', 'frontend_dev')"
                    },
                    "message": {
                        "type": "string",
                        "description": "The content of the message"
                    }
                },
                "required": ["to_agent", "message"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let to_agent = match input.get("to_agent").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => return ToolResult::error("Missing 'to_agent'"),
        };
        let message = match input.get("message").and_then(|v| v.as_str()) {
            Some(m) => m,
            None => return ToolResult::error("Missing 'message'"),
        };

        let from_agent = context
            .session_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        // Try in-memory mailbox delivery first (fast path for registered agents)
        if mailbox::is_registered(to_agent) {
            match mailbox::send_message(&from_agent, to_agent, "", message) {
                Ok(_msg_id) => {
                    // Emit event if sender is available
                    if let Some(ref sender_arc) = self.sender {
                        if let Ok(sender) = sender_arc.lock() {
                            let _ = sender.try_send(crate::event::Event::InboxMessage {
                                to_agent: to_agent.to_string(),
                                from_agent: from_agent.clone(),
                                message: message.to_string(),
                            });
                        }
                    }
                    return ToolResult::success(format!(
                        "Message successfully delivered to {} via mailbox",
                        to_agent
                    ));
                }
                Err(e) => {
                    // Fall through to file-based delivery
                    tracing::debug!(
                        to = %to_agent,
                        error = %e,
                        "mailbox delivery failed, falling back to file-based inbox"
                    );
                }
            }
        }

        // File-based delivery fallback
        let target_dir = get_inbox_dir(&context.cwd, to_agent);
        if let Err(e) = fs::create_dir_all(&target_dir) {
            return ToolResult::error(format!("Failed to create inbox directory: {}", e));
        }

        let msg_id = Uuid::new_v4().to_string();
        let file_path = target_dir.join(format!("msg_{}.json", msg_id));

        let payload = json!({
            "from_agent": from_agent,
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        match serde_json::to_string_pretty(&payload) {
            Ok(json_str) => match fs::write(&file_path, json_str) {
                Ok(_) => {
                    // Emit event if sender is available
                    if let Some(ref sender_arc) = self.sender {
                        if let Ok(sender) = sender_arc.lock() {
                            let _ = sender.try_send(crate::event::Event::InboxMessage {
                                to_agent: to_agent.to_string(),
                                from_agent: context
                                    .session_id
                                    .clone()
                                    .unwrap_or_else(|| "unknown".to_string()),
                                message: message.to_string(),
                            });
                        }
                    }
                    ToolResult::success(format!("Message successfully delivered to {}", to_agent))
                }
                Err(e) => ToolResult::error(format!("Failed to write message: {}", e)),
            },
            Err(e) => ToolResult::error(format!("Failed to serialize message: {}", e)),
        }
    }
}

/// Tool to read and consume messages from the agent's own inbox
#[derive(Clone, Default)]
pub struct ReadInboxTool;

impl ReadInboxTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ReadInboxTool {
    fn name(&self) -> String {
        "read_inbox".to_string()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Read and consume all pending asynchronous messages from your inbox."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn execute(&self, _input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let mut all_messages = Vec::new();

        // Read from in-memory mailbox if agent is registered
        if let Some(ref session_id) = context.session_id {
            let mailbox_msgs = mailbox::read_messages(session_id, false);
            for msg in mailbox_msgs {
                all_messages.push(json!({
                    "from_agent": msg.from,
                    "message": msg.body,
                    "subject": msg.subject,
                    "timestamp": msg.timestamp,
                    "source": "mailbox",
                }));
            }
        }

        // Also check file-based inboxes (for messages from non-registered senders)
        let mut dirs_to_check = vec![get_inbox_dir(&context.cwd, "tech_lead")];

        if let Some(ref session_id) = context.session_id {
            dirs_to_check.push(get_inbox_dir(&context.cwd, session_id));
        }

        for inbox_dir in dirs_to_check {
            if let Ok(entries) = fs::read_dir(&inbox_dir) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                                all_messages.push(val);
                            }
                        }
                        // Consume the message by deleting it
                        let _ = fs::remove_file(path);
                    }
                }
            }
        }

        if all_messages.is_empty() {
            ToolResult::success("Inbox is empty.")
        } else {
            match serde_json::to_string_pretty(&all_messages) {
                Ok(s) => ToolResult::success(s),
                Err(e) => ToolResult::error(format!("Failed to serialize inbox: {}", e)),
            }
        }
    }
}
