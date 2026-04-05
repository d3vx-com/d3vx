//! Session State
//!
//! Encapsulates all session-related state for the application.
//! This struct was extracted from the main App struct to reduce the God Object pattern.

use std::time::Instant;

use crate::agent::file_change_log::FileChangeLog;
use crate::ipc::{Message, PermissionRequest, ThinkingState, TokenUsage};

/// Session-related state extracted from App
///
/// Contains all fields related to conversation, messages, and session management.
/// This allows session state to be tested in isolation and reduces coupling in the main App struct.
#[derive(Debug)]
pub struct SessionState {
    // ════════════════════════════════════════════════════════════════════════════
    // Session Identification
    // ════════════════════════════════════════════════════════════════════════════
    /// Current session ID to resume
    pub session_id: Option<String>,
    /// Last active session in the 'home' workspace
    pub home_session_id: Option<String>,
    /// Target file path to mirror raw LLM token deltas out of the terminal buffer limit
    pub stream_out: Option<std::path::PathBuf>,

    // ════════════════════════════════════════════════════════════════════════════
    // Conversation State
    // ════════════════════════════════════════════════════════════════════════════
    /// Chat messages
    pub messages: Vec<Message>,
    /// Queued messages to be sent after current one finishes
    pub message_queue: Vec<String>,
    /// Pending image attachments for the next message
    pub pending_images: Vec<std::path::PathBuf>,

    // ════════════════════════════════════════════════════════════════════════════
    // Permission/Request State
    // ════════════════════════════════════════════════════════════════════════════
    /// Permission request (if any)
    pub permission_request: Option<PermissionRequest>,
    /// Initialization hint (shown when agent fails to start)
    pub init_hint: Option<String>,

    // ════════════════════════════════════════════════════════════════════════════
    // Thinking/Processing State
    // ════════════════════════════════════════════════════════════════════════════
    /// Current thinking state
    pub thinking: ThinkingState,
    /// Thinking start time (for elapsed display)
    pub thinking_start: Option<Instant>,

    // ════════════════════════════════════════════════════════════════════════════
    // Token/Cost Tracking
    // ════════════════════════════════════════════════════════════════════════════
    /// Token usage
    pub token_usage: TokenUsage,
    /// Accumulated runtime cost in USD
    pub session_cost: f64,
    /// Formatted cost string
    pub formatted_cost: String,

    // ════════════════════════════════════════════════════════════════════════════
    // File Change Tracking (for undo/revert)
    // ════════════════════════════════════════════════════════════════════════════
    /// Tracks file modifications made by agent tools for undo support.
    pub file_change_log: FileChangeLog,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            session_id: None,
            home_session_id: None,
            stream_out: None,

            messages: Vec::new(),
            message_queue: Vec::new(),
            pending_images: Vec::new(),

            permission_request: None,
            init_hint: None,

            thinking: ThinkingState::default(),
            thinking_start: None,

            token_usage: TokenUsage::default(),
            session_cost: 0.0,
            formatted_cost: String::new(),

            file_change_log: FileChangeLog::new(),
        }
    }
}

impl SessionState {
    /// Create a new SessionState with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a message to the conversation
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Clear all messages and reset session state
    pub fn clear(&mut self) {
        self.messages.clear();
        self.message_queue.clear();
        self.pending_images.clear();
        self.permission_request = None;
        self.init_hint = None;
        self.thinking = ThinkingState::default();
        self.thinking_start = None;
        self.session_cost = 0.0;
        self.formatted_cost.clear();
        self.file_change_log = FileChangeLog::new();
    }

    /// Queue a message to be sent
    pub fn queue_message(&mut self, message: String) {
        self.message_queue.push(message);
    }

    /// Start thinking state
    pub fn start_thinking(&mut self) {
        self.thinking = ThinkingState {
            is_thinking: true,
            text: String::new(),
            phase: crate::ipc::ThinkingPhase::Thinking,
        };
        self.thinking_start = Some(Instant::now());
    }

    /// Stop thinking state
    pub fn stop_thinking(&mut self) {
        self.thinking = ThinkingState::default();
        self.thinking_start = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_default() {
        let state = SessionState::default();
        assert!(state.session_id.is_none());
        assert!(state.messages.is_empty());
        assert!(state.message_queue.is_empty());
        assert!(state.permission_request.is_none());
        assert_eq!(state.session_cost, 0.0);
    }

    #[test]
    fn test_add_message() {
        let mut state = SessionState::default();
        state.add_message(Message::user("test".to_string()));
        assert_eq!(state.messages.len(), 1);
    }

    #[test]
    fn test_clear_session() {
        let mut state = SessionState::default();
        state.messages.push(Message::user("test".to_string()));
        state.session_cost = 10.0;
        state.clear();
        assert!(state.messages.is_empty());
        assert_eq!(state.session_cost, 0.0);
    }

    #[test]
    fn test_queue_message() {
        let mut state = SessionState::default();
        state.queue_message("hello".to_string());
        assert_eq!(state.message_queue.len(), 1);
        assert_eq!(state.message_queue[0], "hello");
    }
}
