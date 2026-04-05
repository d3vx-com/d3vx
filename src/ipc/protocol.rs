//! IPC Protocol - JSON-RPC communication between TUI and Agent
//!
//! Uses JSON-RPC 2.0 over stdio for communication.

use serde::{Deserialize, Serialize};
use std::fmt;

pub mod jsonrpc {
    use super::*;

    /// JSON-RPC version
    pub const VERSION: &str = "2.0";

    /// JSON-RPC Request
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Request {
        pub jsonrpc: &'static str,
        pub id: u64,
        pub method: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub params: Option<serde_json::Value>,
    }

    impl Request {
        pub fn new(id: u64, method: impl Into<String>) -> Self {
            Self {
                jsonrpc: VERSION,
                id,
                method: method.into(),
                params: None,
            }
        }

        pub fn with_params(mut self, params: impl Serialize) -> Self {
            self.params =
                Some(serde_json::to_value(params).expect("Failed to serialize JSON-RPC params"));
            self
        }
    }

    /// JSON-RPC Response
    #[derive(Debug, Clone, Deserialize)]
    pub struct Response {
        pub jsonrpc: String,
        pub id: u64,
        #[serde(default)]
        pub result: Option<serde_json::Value>,
        #[serde(default)]
        pub error: Option<Error>,
    }

    /// JSON-RPC Error
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Error {
        pub code: i32,
        pub message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub data: Option<serde_json::Value>,
    }

    /// JSON-RPC Notification (no id, one-way)
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Notification {
        pub jsonrpc: &'static str,
        pub method: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub params: Option<serde_json::Value>,
    }

    impl Notification {
        pub fn new(method: impl Into<String>) -> Self {
            Self {
                jsonrpc: VERSION,
                method: method.into(),
                params: None,
            }
        }

        pub fn with_params(mut self, params: impl Serialize) -> Self {
            self.params =
                Some(serde_json::to_value(params).expect("Failed to serialize JSON-RPC params"));
            self
        }
    }
}

// ────────────────────────────────────────────────────────────
// RPC Methods
// ────────────────────────────────────────────────────────────

/// RPC Methods (TUI → Agent)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    // Chat
    SendMessage,
    CancelCurrent,

    // Session
    ClearHistory,
    ExportSession,

    // Configuration
    SetVerbose,
    SetModel,

    // Permission
    RespondPermission,
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SendMessage => write!(f, "sendMessage"),
            Self::CancelCurrent => write!(f, "cancelCurrent"),
            Self::ClearHistory => write!(f, "clearHistory"),
            Self::ExportSession => write!(f, "exportSession"),
            Self::SetVerbose => write!(f, "setVerbose"),
            Self::SetModel => write!(f, "setModel"),
            Self::RespondPermission => write!(f, "respondPermission"),
        }
    }
}

impl From<Method> for String {
    fn from(method: Method) -> Self {
        method.to_string()
    }
}

/// Notification Events (Agent → TUI)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    // Messages
    OnMessage,
    OnMessageUpdate,

    // Tools
    OnToolCall,
    OnToolCallUpdate,

    // Thinking
    OnThinking,

    // Permission
    OnPermissionRequest,

    // Status
    OnError,
    OnSessionEnd,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OnMessage => write!(f, "onMessage"),
            Self::OnMessageUpdate => write!(f, "onMessageUpdate"),
            Self::OnToolCall => write!(f, "onToolCall"),
            Self::OnToolCallUpdate => write!(f, "onToolCallUpdate"),
            Self::OnThinking => write!(f, "onThinking"),
            Self::OnPermissionRequest => write!(f, "onPermissionRequest"),
            Self::OnError => write!(f, "onError"),
            Self::OnSessionEnd => write!(f, "onSessionEnd"),
        }
    }
}

impl From<Event> for String {
    fn from(event: Event) -> Self {
        event.to_string()
    }
}

impl std::str::FromStr for Event {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "onMessage" => Ok(Self::OnMessage),
            "onMessageUpdate" => Ok(Self::OnMessageUpdate),
            "onToolCall" => Ok(Self::OnToolCall),
            "onToolCallUpdate" => Ok(Self::OnToolCallUpdate),
            "onThinking" => Ok(Self::OnThinking),
            "onPermissionRequest" => Ok(Self::OnPermissionRequest),
            "onError" => Ok(Self::OnError),
            "onSessionEnd" => Ok(Self::OnSessionEnd),
            _ => anyhow::bail!("Unknown event: {}", s),
        }
    }
}

// ────────────────────────────────────────────────────────────
// Protocol Messages
// ────────────────────────────────────────────────────────────

/// Send message request params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageParams {
    pub content: String,
}

/// Set verbose request params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetVerboseParams {
    pub verbose: bool,
}

/// Respond permission request params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RespondPermissionParams {
    pub request_id: String,
    pub response: String,
}

/// Error notification params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorParams {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

// ────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_serialization() {
        let notif = jsonrpc::Notification::new(Event::OnMessage).with_params(SendMessageParams {
            content: "Update".to_string(),
        });
        let json = serde_json::to_string(&notif).unwrap();
        assert!(json.contains("onMessage"));
        assert!(json.contains("Update"));
    }

    #[test]
    fn test_error_params_serialization() {
        let params = ErrorParams {
            message: "Failed".to_string(),
            code: Some("ECONNRESET".to_string()),
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("Failed"));
        assert!(json.contains("ECONNRESET"));
    }

    #[test]
    fn test_method_display() {
        assert_eq!(Method::SendMessage.to_string(), "sendMessage");
        assert_eq!(Method::RespondPermission.to_string(), "respondPermission");
    }

    #[test]
    fn test_event_from_str() {
        use std::str::FromStr;
        assert_eq!(Event::from_str("onMessage").unwrap(), Event::OnMessage);
        assert_eq!(Event::from_str("onError").unwrap(), Event::OnError);
        assert!(Event::from_str("unknown").is_err());
    }
}
