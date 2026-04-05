//! Auto-Compaction
//!
//! When conversation approaches token limits, summarizes older messages
//! to keep the context window manageable.
//!
//! This module provides stateless compaction helpers that operate on
//! [`Conversation`] instances. For the higher-level async compaction
//! manager, see [`crate::compact_agent::ContextManager`].

use crate::agent::conversation::Conversation;

/// Configuration for auto-compaction.
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Token threshold percentage to trigger compaction (0.0-1.0).
    /// Default: 0.8 (trigger at 80% of context window).
    pub threshold_pct: f64,
    /// Maximum context window tokens.
    pub max_context_tokens: u64,
    /// Number of recent messages to always keep.
    pub keep_recent: usize,
    /// Whether compaction is enabled.
    pub enabled: bool,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            threshold_pct: 0.8,
            max_context_tokens: 200_000,
            keep_recent: 6,
            enabled: true,
        }
    }
}

/// Result of a compaction operation.
#[derive(Debug)]
pub struct CompactionResult {
    /// Number of messages before compaction.
    pub messages_before: usize,
    /// Number of messages after compaction.
    pub messages_after: usize,
    /// Tokens saved by compaction.
    pub tokens_saved: u64,
    /// Summary of compacted messages (to be inserted).
    pub summary: String,
}

/// Check if compaction is needed for a conversation.
pub fn needs_compaction(conversation: &Conversation, config: &CompactionConfig) -> bool {
    if !config.enabled {
        return false;
    }
    let threshold = (config.max_context_tokens as f64 * config.threshold_pct) as u64;
    conversation.total_tokens() >= threshold
}

/// Perform compaction on a conversation.
///
/// Returns a summary of the removed messages and the count.
/// The caller is responsible for replacing old messages with the summary.
pub fn compact_conversation(
    conversation: &mut Conversation,
    config: &CompactionConfig,
) -> Option<CompactionResult> {
    if !needs_compaction(conversation, config) {
        return None;
    }

    let messages_before = conversation.len();
    let tokens_before = conversation.total_tokens();

    // Keep first message (system/user context) + last N messages
    let removed = conversation.compact(config.keep_recent);

    if removed == 0 {
        return None;
    }

    let tokens_after = conversation.total_tokens();
    let messages_after = conversation.len();
    let tokens_saved = tokens_before.saturating_sub(tokens_after);

    let summary = format!(
        "[Context auto-compacted: {removed} earlier messages summarized to save ~{tokens_saved} tokens. \
         Recent conversation preserved.]"
    );

    Some(CompactionResult {
        messages_before,
        messages_after,
        tokens_saved,
        summary,
    })
}

/// Build a compaction prompt for LLM-based summarization.
///
/// Returns prompt text that can be sent to a small/fast model
/// to summarize the conversation history.
pub fn build_compaction_prompt(messages_text: &str) -> String {
    format!(
        "Summarize the following conversation history concisely. \
         Preserve key decisions, file paths mentioned, and important context. \
         Omit tool execution details unless they contain critical results.\n\n\
         ---\n{messages_text}\n---\n\n\
         Provide a concise summary:"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::Message;

    /// Helper: build a conversation with N messages of approximate token size.
    fn make_conversation(message_count: usize, chars_per_message: usize) -> Conversation {
        let mut conv = Conversation::new();
        for i in 0..message_count {
            let text = "x".repeat(chars_per_message) + &format!(" msg {i}");
            conv.add_message(Message::user_text(text));
        }
        conv
    }

    #[test]
    fn needs_compaction_returns_false_when_under_threshold() {
        let conv = make_conversation(5, 100); // ~125 tokens
        let config = CompactionConfig {
            max_context_tokens: 10_000,
            threshold_pct: 0.8,
            ..Default::default()
        };
        assert!(!needs_compaction(&conv, &config));
    }

    #[test]
    fn needs_compaction_returns_true_when_over_threshold() {
        // 10 messages x 4000 chars = ~10 000 tokens
        let conv = make_conversation(10, 4000);
        let config = CompactionConfig {
            max_context_tokens: 10_000,
            threshold_pct: 0.8,
            ..Default::default()
        };
        assert!(needs_compaction(&conv, &config));
    }

    #[test]
    fn needs_compaction_respects_enabled_false() {
        // Well over threshold but disabled
        let conv = make_conversation(10, 4000);
        let config = CompactionConfig {
            enabled: false,
            max_context_tokens: 10_000,
            threshold_pct: 0.8,
            ..Default::default()
        };
        assert!(!needs_compaction(&conv, &config));
    }

    #[test]
    fn compact_conversation_preserves_recent_messages() {
        // 10 messages, keep recent 4 (+1 first) = 5 kept, 5 removed
        let mut conv = make_conversation(10, 100);
        let config = CompactionConfig {
            max_context_tokens: 1, // force compaction
            threshold_pct: 0.01,
            keep_recent: 4,
            enabled: true,
        };

        let result = compact_conversation(&mut conv, &config).unwrap();

        // Should keep first + 4 recent = 5
        assert_eq!(result.messages_before, 10);
        assert_eq!(result.messages_after, 5);
        assert!(result.tokens_saved > 0);
    }

    #[test]
    fn compaction_result_has_correct_counts() {
        let mut conv = make_conversation(8, 200);
        let config = CompactionConfig {
            max_context_tokens: 1,
            threshold_pct: 0.01,
            keep_recent: 2,
            enabled: true,
        };

        let result = compact_conversation(&mut conv, &config).unwrap();

        // 8 before, keep first + 2 recent = 3 after
        assert_eq!(result.messages_before, 8);
        assert_eq!(result.messages_after, 3);
        assert_eq!(result.messages_before - result.messages_after, 5);
    }

    #[test]
    fn compact_conversation_returns_none_when_not_needed() {
        let mut conv = make_conversation(3, 100);
        let config = CompactionConfig {
            max_context_tokens: 100_000,
            threshold_pct: 0.8,
            keep_recent: 6,
            enabled: true,
        };

        assert!(compact_conversation(&mut conv, &config).is_none());
    }

    #[test]
    fn build_compaction_prompt_includes_messages() {
        let prompt = build_compaction_prompt("user asked about foo");
        assert!(prompt.contains("user asked about foo"));
        assert!(prompt.contains("Summarize"));
    }
}
