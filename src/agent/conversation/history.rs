//! Conversation Management
//!
//! Manages conversation message history. Supports both user
//! and assistant messages with structured content blocks.

use crate::providers::{ContentBlock, Message, MessageContent, Role};
use crate::utils::text::truncate_80_20;
use crate::utils::token::estimate_message_tokens;
use std::collections::VecDeque;

/// Maximum number of messages to keep in history before auto-pruning
const DEFAULT_MAX_MESSAGES: usize = 100;

/// Manages conversation message history.
///
/// The `Conversation` struct maintains a list of messages and provides
/// methods for adding, retrieving, and managing the conversation history.
#[derive(Debug, Clone)]
pub struct Conversation {
    /// The message history
    messages: VecDeque<Message>,
    /// Maximum number of messages to keep
    max_messages: usize,
    /// Total token count (approximate)
    total_tokens: u64,
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

impl Conversation {
    /// Create a new empty conversation.
    pub fn new() -> Self {
        Self {
            messages: VecDeque::with_capacity(50),
            max_messages: DEFAULT_MAX_MESSAGES,
            total_tokens: 0,
        }
    }

    /// Create a conversation with a custom max message limit.
    pub fn with_max_messages(max_messages: usize) -> Self {
        Self {
            messages: VecDeque::with_capacity(max_messages.min(100)),
            max_messages,
            total_tokens: 0,
        }
    }

    /// Add a user message with text content.
    pub fn add_user_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.add_message(Message::user_text(&text));
    }

    /// Add an assistant message with text content.
    pub fn add_assistant_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.add_message(Message::assistant_text(&text));
    }

    /// Add a user message with content blocks.
    pub fn add_user_blocks(&mut self, blocks: Vec<ContentBlock>) {
        self.add_message(Message::user_blocks(blocks));
    }

    /// Add an assistant message with content blocks.
    pub fn add_assistant_blocks(&mut self, blocks: Vec<ContentBlock>) {
        self.add_message(Message::assistant_blocks(blocks));
    }

    /// Add a message to the conversation history.
    ///
    /// If the conversation exceeds `max_messages`, the oldest messages
    /// are removed (except system messages if any).
    pub fn add_message(&mut self, message: Message) {
        // Approximate token count (4 chars per token)
        let token_count = estimate_message_tokens(&message);
        self.total_tokens += token_count;

        self.messages.push_back(message);

        // Prune if needed
        while self.messages.len() > self.max_messages {
            if let Some(removed) = self.messages.pop_front() {
                let removed_tokens = estimate_message_tokens(&removed);
                self.total_tokens = self.total_tokens.saturating_sub(removed_tokens);
            }
        }
    }

    /// Get all messages in the conversation.
    pub fn get_messages(&self) -> Vec<Message> {
        self.messages.iter().cloned().collect()
    }

    /// Get the number of messages in the conversation.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the conversation is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Clear all messages from the conversation.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.total_tokens = 0;
    }

    /// Get the approximate total token count.
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens
    }

    /// Set messages directly (for restoring from storage).
    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages.clear();
        self.total_tokens = 0;

        for message in messages {
            self.add_message(message);
        }
    }

    /// Add a message (alias for add_message to match some usage patterns)
    pub fn push_message(&mut self, message: Message) {
        self.add_message(message);
    }

    /// Insert a message after the first message (i.e., after system context).
    /// Used by compaction to place a summary where the LLM will see it early.
    pub fn insert_after_first(&mut self, message: Message) {
        if self.messages.len() <= 1 {
            self.add_message(message);
            return;
        }
        let tokens = estimate_message_tokens(&message);
        self.total_tokens += tokens;
        // VecDeque doesn't have insert-after, so drain into a Vec and rebuild
        let mut vec: Vec<Message> = self.messages.drain(..).collect();
        vec.insert(1, message);
        self.messages = vec.into_iter().collect();
    }

    /// Get the last message in the conversation.
    pub fn last(&self) -> Option<&Message> {
        self.messages.back()
    }

    /// Get the last message with a specific role.
    pub fn last_with_role(&self, role: Role) -> Option<&Message> {
        self.messages.iter().rev().find(|m| m.role == role)
    }

    /// Remove the last message from the conversation.
    pub fn pop(&mut self) -> Option<Message> {
        let message = self.messages.pop_back();
        if let Some(ref msg) = message {
            let tokens = estimate_message_tokens(msg);
            self.total_tokens = self.total_tokens.saturating_sub(tokens);
        }
        message
    }

    /// Truncate the conversation to keep only the last N messages.
    pub fn truncate(&mut self, keep_count: usize) {
        while self.messages.len() > keep_count {
            if let Some(removed) = self.messages.pop_front() {
                let tokens = estimate_message_tokens(&removed);
                self.total_tokens = self.total_tokens.saturating_sub(tokens);
            }
        }
    }

    /// Prune messages to fit within a token budget.
    ///
    /// This implementation uses a "Smart Pruning" strategy:
    /// 1. First, it attempts to truncate excessively large individual messages (80/20 rule).
    /// 2. If still over budget, it keeps the first message and the most recent messages.
    pub fn prune_to_budget(&mut self, max_tokens: u64) {
        if self.total_tokens <= max_tokens || self.messages.len() <= 1 {
            return;
        }

        // Step 1: Truncate large individual messages
        let threshold = (max_tokens as f64 * 0.3) as u64;
        let mut budget_after_truncation = 0;

        for msg in self.messages.iter_mut() {
            let original_tokens = estimate_message_tokens(msg);
            if original_tokens > threshold {
                match &mut msg.content {
                    MessageContent::Text(text) => {
                        *text = truncate_80_20(text, (threshold * 4) as usize);
                    }
                    MessageContent::Blocks(blocks) => {
                        for block in blocks {
                            match block {
                                ContentBlock::Text { text } => {
                                    *text = truncate_80_20(text, (threshold * 3) as usize);
                                }
                                ContentBlock::ToolResult { content, .. } => {
                                    *content = truncate_80_20(content, (threshold * 3) as usize);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            budget_after_truncation += estimate_message_tokens(msg);
        }
        self.total_tokens = budget_after_truncation;

        if self.total_tokens <= max_tokens {
            return;
        }

        // Step 2: Drop middle messages if still over budget
        let first = self.messages.pop_front();
        let mut kept_messages: VecDeque<Message> = VecDeque::new();
        let mut current_tokens = 0;

        if let Some(msg) = first {
            let tokens = estimate_message_tokens(&msg);
            current_tokens += tokens;
            kept_messages.push_back(msg);
        }

        let mut temp: Vec<Message> = Vec::new();
        let mut dropped_count = 0;

        while let Some(msg) = self.messages.pop_back() {
            let tokens = estimate_message_tokens(&msg);
            if current_tokens + tokens <= max_tokens {
                current_tokens += tokens;
                temp.push(msg);
            } else {
                dropped_count += self.messages.len() + 1;
                break;
            }
        }

        if dropped_count > 0 {
            kept_messages.push_back(Message::user_text(format!(
                "[...system: {} previous interaction(s) pruned from context to save tokens...]",
                dropped_count
            )));
            current_tokens += 10;
        }

        for msg in temp.into_iter().rev() {
            kept_messages.push_back(msg);
        }

        self.messages = kept_messages;
        self.total_tokens = current_tokens;
    }

    /// Compact the conversation by keeping the first message and the last N messages.
    /// Returns the removed messages so the caller can persist them before discarding.
    pub fn compact_drain(&mut self, keep_last: usize) -> Vec<Message> {
        if self.messages.len() <= keep_last + 1 {
            return Vec::new();
        }

        let first = self.messages.pop_front();

        let mut drained = Vec::new();
        while self.messages.len() > keep_last {
            if let Some(msg) = self.messages.pop_front() {
                drained.push(msg);
            }
        }

        if let Some(msg) = first {
            self.messages.push_front(msg);
        }

        self.total_tokens = self
            .messages
            .iter()
            .map(|m| estimate_message_tokens(m))
            .sum();

        drained
    }

    /// Compact the conversation by keeping the first message and the last N messages.
    /// Returns the number of messages removed (messages are discarded).
    pub fn compact(&mut self, keep_last: usize) -> usize {
        self.compact_drain(keep_last).len()
    }
}
