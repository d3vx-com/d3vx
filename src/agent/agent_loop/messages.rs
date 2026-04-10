//! Agent loop message management: add, get, compact, set methods.

use tracing::info;

use crate::agent::context::compaction::{needs_compaction, CompactionConfig};
use crate::providers::{ContentBlock, Message, MessageContent, Role, TokenUsage};

use super::AgentLoop;

/// Extract a flat text snapshot from a slice of messages.
/// Truncates each message to ~200 chars to keep the archive compact.
fn summarize_messages(messages: &[Message]) -> String {
    let mut lines = Vec::new();
    for msg in messages {
        let role = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
        };
        let text = match &msg.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
        };
        if text.is_empty() {
            continue;
        }
        let truncated = if text.len() > 200 {
            format!("{}...", &text[..197])
        } else {
            text
        };
        lines.push(format!("[{role}] {truncated}"));
    }
    lines.join("\n")
}

/// Extract key information (file paths, tool names, decisions) from messages.
fn extract_key_context(messages: &[Message]) -> Vec<String> {
    let mut keys = Vec::new();
    for msg in messages {
        let text = match &msg.content {
            MessageContent::Text(t) => t,
            MessageContent::Blocks(blocks) => {
                // Extract tool names and file paths from structured blocks
                for block in blocks {
                    match block {
                        ContentBlock::ToolUse { name, .. } => {
                            keys.push(format!("tool:{name}"));
                        }
                        _ => {}
                    }
                }
                continue;
            }
        };
        // Extract file paths (heuristic: strings containing /src/, /lib/, .rs, .ts, etc.)
        for word in text.split_whitespace() {
            if word.contains("/src/")
                || word.contains("/lib/")
                || word.ends_with(".rs")
                || word.ends_with(".ts")
                || word.ends_with(".tsx")
                || word.ends_with(".py")
                || word.ends_with(".js")
                || word.ends_with(".toml")
                || word.ends_with(".yml")
            {
                let cleaned = word.trim_matches(|c: char| {
                    !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-'
                });
                if !cleaned.is_empty() && !keys.contains(&cleaned.to_string()) {
                    keys.push(cleaned.to_string());
                }
            }
        }
    }
    keys.truncate(15); // Keep top 15 unique keys
    keys
}

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
    ///
    /// **Before dropping**, persists the about-to-be-removed messages to the
    /// `memory_entries` SQLite table so the context is not lost — it can be
    /// recalled later via the existing FTS5 search.
    pub async fn auto_compact_if_needed(&self) {
        let (model, session_id, db) = {
            let cfg = self.config.read().await;
            if cfg.skip_compaction {
                return;
            }
            (cfg.model.clone(), cfg.session_id.clone(), cfg.db.clone())
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

        if !should_compact {
            return;
        }

        // Drain the messages that will be removed (so we can persist them)
        let drained = {
            let mut conv = self.conversation.write().await;
            conv.compact_drain(config.keep_recent)
        };

        let removed = drained.len();
        if removed == 0 {
            return;
        }

        // Build a text archive of the drained messages
        let archive = summarize_messages(&drained);
        let key_context = extract_key_context(&drained);
        let tags = serde_json::to_string(&key_context).unwrap_or_else(|_| "[]".to_string());

        // Persist to memory_entries if DB is available
        let mut persisted = false;
        if let Some(ref db) = db {
            let entry_id = format!("compact_{}", session_id);
            let title = format!("Context archive: {} messages", removed);
            let summary = if key_context.is_empty() {
                format!("{} messages archived from context compaction", removed)
            } else {
                format!(
                    "{} messages archived. Key files: {}",
                    removed,
                    key_context.join(", ")
                )
            };

            let db_lock = db.lock();
            let result = db_lock.execute(
                "INSERT OR REPLACE INTO memory_entries (id, type, title, file_path, tags, summary, project_path)
                 VALUES (?1, 'conversation', ?2, '', ?3, ?4, '')",
                rusqlite::params![entry_id, title, tags, summary],
            );
            drop(db_lock);

            if let Err(e) = result {
                info!("Could not persist compaction archive to memory: {e}");
            } else {
                // Also index the full text for FTS5 search
                let db_lock = db.lock();
                let fts_result = db_lock.execute(
                    "INSERT OR REPLACE INTO memory_index(name, content) VALUES (?1, ?2)",
                    rusqlite::params![entry_id, archive],
                );
                drop(db_lock);

                if let Err(e) = fts_result {
                    info!("Could not index compaction archive for search: {e}");
                } else {
                    persisted = true;
                }
            }
        }

        // Insert a brief summary message into the conversation so the LLM knows context was compacted
        {
            let mut conv = self.conversation.write().await;
            let summary_text = if persisted {
                format!(
                    "[Context auto-compacted: {} earlier messages archived to persistent memory. \
                     Key context: {}. You can recall details if needed.]",
                    removed,
                    if key_context.is_empty() {
                        "see memory".to_string()
                    } else {
                        key_context.join(", ")
                    }
                )
            } else {
                format!(
                    "[Context auto-compacted: {} earlier messages removed to free context. \
                     Key context: {}]",
                    removed,
                    if key_context.is_empty() {
                        "none extracted".to_string()
                    } else {
                        key_context.join(", ")
                    }
                )
            };
            // Insert after the first message (system context)
            if conv.len() > 1 {
                conv.insert_after_first(Message::user_text(summary_text));
            }
        }

        // Emit event so the TUI can show a notification
        self.emit(super::types::AgentEvent::Text {
            text: format!(
                "Context auto-compacted: {} messages archived{}. Key: {}",
                removed,
                if persisted { " to memory" } else { "" },
                key_context.join(", ")
            ),
        });

        info!(
            "Auto-compacted conversation: archived {} older messages (context_window={}, usable={}, persisted={})",
            removed, context_window, usable, persisted
        );
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
