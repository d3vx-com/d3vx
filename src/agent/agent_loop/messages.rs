//! Agent loop message management: add, get, compact, set methods.

use tracing::info;

use crate::agent::context::compaction::{needs_compaction, CompactionConfig};
use crate::providers::{ContentBlock, Message, Role, TokenUsage};

use super::AgentLoop;

impl AgentLoop {
    /// Add a user message to the conversation.
    pub async fn add_user_message(&self, content: impl Into<String>) {
        let mut conv = self.conversation.write().await;
        conv.add_user_text(content);
    }

    /// Add a user message with content blocks.
    pub async fn add_user_blocks(&self, blocks: Vec<ContentBlock>) {
        let mut conv = self.conversation.write().await;
        conv.add_user_blocks(blocks);
    }

    /// Add a message to the conversation.
    pub async fn add_message(&self, message: Message) {
        let mut conv = self.conversation.write().await;
        conv.add_message(message);
    }

    /// Get the current conversation messages.
    pub async fn get_messages(&self) -> Vec<Message> {
        self.conversation.read().await.get_messages()
    }

    /// Get total token usage.
    pub async fn get_usage(&self) -> TokenUsage {
        self.total_usage.read().await.clone()
    }

    /// Clear the conversation history.
    pub async fn clear_history(&self) {
        let mut conv = self.conversation.write().await;
        conv.clear();

        let mut usage = self.total_usage.write().await;
        *usage = TokenUsage::default();
    }

    /// Compact the conversation history.
    pub async fn compact_history(&self, keep_last: usize) -> usize {
        let mut conv = self.conversation.write().await;
        conv.compact(keep_last)
    }

    /// Auto-compact if the conversation is approaching the model's context limit.
    ///
    /// Queries the provider for the model's actual `context_window`, then triggers
    /// compaction at 80% usage. Falls back to 200K if the model info is unavailable.
    /// Keeps the first message (system context) and the 6 most recent messages.
    pub async fn auto_compact_if_needed(&self) {
        let model = {
            let cfg = self.config.read().await;
            if cfg.skip_compaction {
                return;
            }
            cfg.model.clone()
        };

        // Resolve the model's actual context window from the provider
        let context_window = self
            .provider
            .model_info(&model)
            .map(|info| info.context_window)
            .unwrap_or(200_000);

        let max_output = self
            .provider
            .model_info(&model)
            .map(|info| info.max_output_tokens)
            .unwrap_or(8_192);

        // Usable context = full window minus reserved output tokens (like opencode)
        let usable = context_window.saturating_sub(max_output);

        let config = CompactionConfig {
            max_context_tokens: usable,
            threshold_pct: 0.8,
            keep_recent: 6,
            enabled: true,
        };

        let should_compact = {
            let conv = self.conversation.read().await;
            needs_compaction(&conv, &config)
        };

        if should_compact {
            let removed = self.compact_history(config.keep_recent).await;
            if removed > 0 {
                info!(
                    "Auto-compacted conversation: removed {} older messages (context_window={}, usable={})",
                    removed, context_window, usable
                );
            }
        }
    }

    /// Set the system prompt for the next run.
    pub async fn set_system_prompt(&self, prompt: impl Into<String>) {
        let mut config = self.config.write().await;
        config.system_prompt = prompt.into();
    }

    /// Set the working directory for subsequent tool execution.
    pub async fn set_working_dir(&self, working_dir: impl Into<String>) {
        let mut config = self.config.write().await;
        config.working_dir = working_dir.into();
    }

    /// Get the current system prompt.
    pub async fn system_prompt(&self) -> String {
        self.config.read().await.system_prompt.clone()
    }

    /// Get the current working directory.
    pub async fn working_dir(&self) -> String {
        self.config.read().await.working_dir.clone()
    }

    pub(super) async fn latest_user_prompt(&self) -> Option<String> {
        let conv = self.conversation.read().await;
        conv.last_with_role(Role::User)
            .and_then(|message| message.as_text())
            .map(|text| text.to_string())
    }

    /// Reset internal counters for a fresh session (used after resume).
    pub async fn reset_for_resume(&self) {
        *self.total_usage.write().await = TokenUsage::default();
        *self.failure_count.write().await = 0;
    }
}
