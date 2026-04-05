//! Context Compaction System

use std::sync::Arc;
use tokio::sync::RwLock;

use super::conversation::Conversation;
use crate::providers::{Message, Role};

#[derive(Debug, Clone)]
pub struct CompactionConfig {
    pub threshold_ratio: f64,
    pub keep_recent: usize,
    pub min_compact_count: usize,
    pub summarization_model: Option<String>,
    pub enabled: bool,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            threshold_ratio: 0.80,
            keep_recent: 10,
            min_compact_count: 3,
            summarization_model: None,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub messages_removed: usize,
    pub tokens_saved: u64,
    pub summary: String,
    pub duration_ms: u64,
}

pub struct ContextManager {
    config: CompactionConfig,
    model_max_tokens: u64,
    total_compactions: usize,
    total_tokens_saved: u64,
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            config: CompactionConfig::default(),
            model_max_tokens: 200_000,
            total_compactions: 0,
            total_tokens_saved: 0,
        }
    }

    pub fn with_config(config: CompactionConfig) -> Self {
        Self {
            config,
            model_max_tokens: 200_000,
            total_compactions: 0,
            total_tokens_saved: 0,
        }
    }

    pub fn with_model_limit(mut self, max_tokens: u64) -> Self {
        self.model_max_tokens = max_tokens;
        self
    }

    pub fn needs_compaction(&self, current_tokens: u64) -> bool {
        if !self.config.enabled {
            return false;
        }
        let ratio = current_tokens as f64 / self.model_max_tokens as f64;
        ratio >= self.config.threshold_ratio
    }

    pub fn token_ratio(&self, current_tokens: u64) -> f64 {
        current_tokens as f64 / self.model_max_tokens as f64
    }

    pub fn stats(&self) -> ContextStats {
        ContextStats {
            total_compactions: self.total_compactions,
            total_tokens_saved: self.total_tokens_saved,
            model_max_tokens: self.model_max_tokens,
        }
    }

    pub fn update_config(&mut self, config: CompactionConfig) {
        self.config = config;
    }

    pub fn record_compaction(&mut self, tokens_saved: u64) {
        self.total_compactions += 1;
        self.total_tokens_saved += tokens_saved;
    }
}

#[derive(Debug, Clone)]
pub struct ContextStats {
    pub total_compactions: usize,
    pub total_tokens_saved: u64,
    pub model_max_tokens: u64,
}

pub struct CompactConversation {
    conversation: Conversation,
    context_manager: Arc<RwLock<ContextManager>>,
}

impl CompactConversation {
    pub fn new(conversation: Conversation) -> Self {
        Self {
            conversation,
            context_manager: Arc::new(RwLock::new(ContextManager::new())),
        }
    }

    pub fn with_manager(
        conversation: Conversation,
        context_manager: Arc<RwLock<ContextManager>>,
    ) -> Self {
        Self {
            conversation,
            context_manager,
        }
    }

    pub async fn check_compaction(&self) -> Option<CompactionNeeded> {
        let manager = self.context_manager.read().await;
        let tokens = self.conversation.total_tokens();

        if manager.needs_compaction(tokens) {
            let ratio = manager.token_ratio(tokens);
            let message_count = self.conversation.len();

            Some(CompactionNeeded {
                current_tokens: tokens,
                token_ratio: ratio,
                message_count,
                messages_to_compact: message_count.saturating_sub(manager.config.keep_recent + 1),
            })
        } else {
            None
        }
    }

    pub async fn compact(&mut self, summary: &str) -> CompactionResult {
        let start = std::time::Instant::now();

        let tokens_before = self.conversation.total_tokens();
        let keep_recent = {
            let manager = self.context_manager.read().await;
            manager.config.keep_recent
        };

        let removed = self.conversation.compact(keep_recent);

        self.conversation.add_user_text(format!(
            "[Previous context summarized in {} messages: {}]",
            removed, summary
        ));

        let tokens_after = self.conversation.total_tokens();
        let tokens_saved = tokens_before.saturating_sub(tokens_after);

        {
            let mut manager = self.context_manager.write().await;
            manager.record_compaction(tokens_saved);
        }

        CompactionResult {
            messages_removed: removed,
            tokens_saved,
            summary: summary.to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
        }
    }

    pub fn conversation(&self) -> &Conversation {
        &self.conversation
    }

    pub fn conversation_mut(&mut self) -> &mut Conversation {
        &mut self.conversation
    }
}

#[derive(Debug, Clone)]
pub struct CompactionNeeded {
    pub current_tokens: u64,
    pub token_ratio: f64,
    pub message_count: usize,
    pub messages_to_compact: usize,
}

pub trait CompactionExt {
    fn add_system_message(&mut self, content: impl Into<String>);
    fn get_messages_for_summary(&self, keep_recent: usize) -> Vec<Message>;
    fn summarize_messages(&self, keep_recent: usize) -> String;
}

impl CompactionExt for Conversation {
    fn add_system_message(&mut self, content: impl Into<String>) {
        let msg = Message::user_text(content);
        self.add_message(msg);
    }

    fn get_messages_for_summary(&self, keep_recent: usize) -> Vec<Message> {
        let messages = self.get_messages();
        let total = messages.len();
        if total <= keep_recent + 1 {
            return vec![];
        }
        messages
            .into_iter()
            .skip(1)
            .take(total - keep_recent - 1)
            .collect()
    }

    fn summarize_messages(&self, keep_recent: usize) -> String {
        let messages = self.get_messages_for_summary(keep_recent);

        if messages.is_empty() {
            return "No significant context to summarize.".to_string();
        }

        let mut user_count = 0;
        let mut assistant_count = 0;

        for msg in &messages {
            match msg.role {
                Role::User => user_count += 1,
                Role::Assistant => assistant_count += 1,
            }
        }

        format!(
            "Conversation covered {} user messages and {} assistant responses.",
            user_count, assistant_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compaction_threshold() {
        let manager = ContextManager::with_config(CompactionConfig {
            threshold_ratio: 0.5,
            enabled: true,
            ..Default::default()
        })
        .with_model_limit(100_000);

        assert!(!manager.needs_compaction(40_000));
        assert!(manager.needs_compaction(60_000));
    }

    #[test]
    fn test_compaction_disabled() {
        let manager = ContextManager::with_config(CompactionConfig {
            enabled: false,
            ..Default::default()
        });

        assert!(!manager.needs_compaction(199_000));
    }

    #[test]
    fn test_token_ratio() {
        let manager = ContextManager::new().with_model_limit(200_000);

        assert!((manager.token_ratio(100_000) - 0.5).abs() < 0.001);
        assert!((manager.token_ratio(150_000) - 0.75).abs() < 0.001);
    }
}
