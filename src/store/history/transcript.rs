//! Transcript Reader
//!
//! Provides read-only access to the visible conversation transcript.
//! The transcript represents what a user would see in a chat interface.

use chrono::{DateTime, Utc};

/// A single entry in the visible transcript.
#[derive(Debug, Clone)]
pub struct TranscriptEntry {
    /// Entry index within the session (0-based).
    pub index: usize,
    /// Role of the message sender.
    pub role: TranscriptRole,
    /// Text content of the message.
    pub content: String,
    /// When the message was created.
    pub timestamp: DateTime<Utc>,
    /// Optional token count for this entry.
    pub token_count: Option<i64>,
}

impl TranscriptEntry {
    /// Check if this is a user message.
    pub fn is_user(&self) -> bool {
        matches!(self.role, TranscriptRole::User)
    }

    /// Check if this is an assistant message.
    pub fn is_assistant(&self) -> bool {
        matches!(self.role, TranscriptRole::Assistant)
    }

    /// Check if this is a system message.
    pub fn is_system(&self) -> bool {
        matches!(self.role, TranscriptRole::System)
    }

    /// Get a preview of the content (first N chars).
    pub fn preview(&self, max_len: usize) -> String {
        if self.content.len() <= max_len {
            self.content.clone()
        } else {
            format!("{}...", &self.content[..max_len])
        }
    }
}

/// Role in the visible transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptRole {
    User,
    Assistant,
    System,
}

impl std::fmt::Display for TranscriptRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranscriptRole::User => write!(f, "user"),
            TranscriptRole::Assistant => write!(f, "assistant"),
            TranscriptRole::System => write!(f, "system"),
        }
    }
}

impl From<&str> for TranscriptRole {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "user" => TranscriptRole::User,
            "assistant" => TranscriptRole::Assistant,
            "system" => TranscriptRole::System,
            _ => TranscriptRole::System,
        }
    }
}

/// Summary of a transcript for quick inspection.
#[derive(Debug, Clone)]
pub struct TranscriptSummary {
    /// Total number of entries.
    pub total_entries: usize,
    /// Number of user messages.
    pub user_messages: usize,
    /// Number of assistant messages.
    pub assistant_messages: usize,
    /// Number of system messages.
    pub system_messages: usize,
    /// Total token count (if available).
    pub total_tokens: Option<i64>,
    /// Timestamp of first entry.
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp of last entry.
    pub ended_at: Option<DateTime<Utc>>,
}

impl Default for TranscriptSummary {
    fn default() -> Self {
        Self {
            total_entries: 0,
            user_messages: 0,
            assistant_messages: 0,
            system_messages: 0,
            total_tokens: None,
            started_at: None,
            ended_at: None,
        }
    }
}

impl TranscriptSummary {
    /// Create from a slice of transcript entries.
    pub fn from_entries(entries: &[TranscriptEntry]) -> Self {
        let mut summary = Self::default();
        summary.total_entries = entries.len();

        for entry in entries {
            match entry.role {
                TranscriptRole::User => summary.user_messages += 1,
                TranscriptRole::Assistant => summary.assistant_messages += 1,
                TranscriptRole::System => summary.system_messages += 1,
            }
            if let Some(tokens) = entry.token_count {
                summary.total_tokens = Some(summary.total_tokens.unwrap_or(0) + tokens);
            }
        }

        if let Some(first) = entries.first() {
            summary.started_at = Some(first.timestamp);
        }
        if let Some(last) = entries.last() {
            summary.ended_at = Some(last.timestamp);
        }

        summary
    }

    /// Duration of the transcript.
    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.ended_at) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }
}

/// Read-only transcript reader.
pub struct TranscriptReader<'a> {
    session_store: &'a crate::store::SessionStore<'a>,
    message_store: &'a crate::store::MessageStore<'a>,
}

impl<'a> TranscriptReader<'a> {
    /// Create a new transcript reader.
    pub fn new(
        session_store: &'a crate::store::SessionStore<'a>,
        message_store: &'a crate::store::MessageStore<'a>,
    ) -> Self {
        Self {
            session_store,
            message_store,
        }
    }

    /// Get transcript entries for a session.
    ///
    /// Uses the secondary message store for granular access when available,
    /// falling back to the primary session's serialized messages.
    pub fn get_entries(
        &self,
        session_id: &str,
        bounds: Option<&crate::store::history::HistoryBounds>,
    ) -> Result<Vec<TranscriptEntry>, crate::store::DatabaseError> {
        let messages = self.message_store.get_for_session(session_id)?;

        if messages.is_empty() {
            return self.get_entries_from_session(session_id, bounds);
        }

        let entries: Vec<TranscriptEntry> = messages
            .into_iter()
            .enumerate()
            .filter_map(|(idx, msg)| {
                let offset = bounds.as_ref().map(|b| b.offset).unwrap_or(0);
                let limit = bounds.as_ref().map(|b| b.limit).unwrap_or(usize::MAX);

                let global_idx = idx;
                if global_idx < offset {
                    return None;
                }
                if global_idx >= offset + limit {
                    return None;
                }

                let timestamp = DateTime::parse_from_rfc3339(&msg.created_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()?;

                Some(TranscriptEntry {
                    index: global_idx,
                    role: TranscriptRole::from(msg.role.to_string().as_str()),
                    content: msg.content,
                    timestamp,
                    token_count: if msg.token_count > 0 {
                        Some(msg.token_count)
                    } else {
                        None
                    },
                })
            })
            .collect();

        Ok(entries)
    }

    /// Fallback: Parse transcript from session's serialized messages.
    fn get_entries_from_session(
        &self,
        session_id: &str,
        bounds: Option<&crate::store::history::HistoryBounds>,
    ) -> Result<Vec<TranscriptEntry>, crate::store::DatabaseError> {
        let session = match self.session_store.get(session_id)? {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };

        let messages: Vec<serde_json::Value> =
            serde_json::from_str(&session.messages).unwrap_or_default();

        let offset = bounds.as_ref().map(|b| b.offset).unwrap_or(0);
        let limit = bounds.as_ref().map(|b| b.limit).unwrap_or(usize::MAX);

        let entries: Vec<TranscriptEntry> = messages
            .into_iter()
            .enumerate()
            .filter_map(|(idx, msg)| {
                let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("system");
                let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");

                let timestamp = msg
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);

                if idx < offset || idx >= offset + limit {
                    return None;
                }

                Some(TranscriptEntry {
                    index: idx,
                    role: TranscriptRole::from(role),
                    content: content.to_string(),
                    timestamp,
                    token_count: None,
                })
            })
            .collect();

        Ok(entries)
    }

    /// Get transcript summary for a session.
    pub fn get_summary(
        &self,
        session_id: &str,
    ) -> Result<TranscriptSummary, crate::store::DatabaseError> {
        let entries = self.get_entries(session_id, None)?;
        Ok(TranscriptSummary::from_entries(&entries))
    }

    /// Get recent transcript entries from the most recent session.
    pub fn get_recent_entries(
        &self,
        project_path: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TranscriptEntry>, crate::store::DatabaseError> {
        let session = self.session_store.get_latest(project_path)?;
        match session {
            Some(s) => self.get_entries(
                &s.id,
                Some(&crate::store::history::HistoryBounds::last(limit)),
            ),
            None => Ok(Vec::new()),
        }
    }
}
