//! Per-agent inboxes and a shared broadcast log.
//!
//! An **inbox** is a JSONL file owned by one recipient agent. Any agent
//! can append; only the owner drains. A **broadcast log** is the same
//! shape but has no owner — every agent reads the full log.
//!
//! JSONL was chosen over a single JSON array because appends are
//! single-syscall and atomic for small payloads on POSIX, which means
//! two writing agents cannot corrupt each other. Readers see any
//! already-written lines consistently and newly-arrived lines show up
//! on the next call.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::errors::CoordinationError;
use super::io;

/// A single coordination message.
///
/// Messages are immutable once appended. The envelope metadata (from,
/// to, timestamp) exists on every message so the log is useful even
/// when the recipient has already processed and dropped the body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// Agent id of the sender.
    pub from: String,
    /// Agent id of the recipient. For broadcasts, this is the string
    /// `"*"` — callers are free to use a different convention, but the
    /// Inbox API itself is oblivious to the value.
    pub to: String,
    /// Free-form payload. Typically JSON-as-string so the agent
    /// receiving it can parse further; no structural validation here.
    pub body: String,
    pub sent_at: DateTime<Utc>,
}

impl Message {
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            body: body.into(),
            sent_at: Utc::now(),
        }
    }
}

/// Per-agent inbox. `agent_id` identifies the owner.
#[derive(Debug, Clone)]
pub struct Inbox {
    agent_id: String,
    path: PathBuf,
}

impl Inbox {
    /// Open (or create) the inbox for `agent_id` rooted at
    /// `inboxes_dir`. Missing directories are created.
    pub fn open(
        inboxes_dir: impl AsRef<Path>,
        agent_id: impl Into<String>,
    ) -> Result<Self, CoordinationError> {
        let dir = inboxes_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|source| CoordinationError::Io {
            path: dir.clone(),
            source,
        })?;
        let agent_id = agent_id.into();
        let path = dir.join(format!("{agent_id}.jsonl"));
        Ok(Self { agent_id, path })
    }

    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append `msg` to this inbox. Any agent can call this — it does
    /// not matter whether they "own" the inbox.
    pub fn send(&self, msg: &Message) -> Result<(), CoordinationError> {
        io::append_jsonl(&self.path, msg)
    }

    /// Read every message currently in this inbox, oldest first.
    pub fn read_all(&self) -> Result<Vec<Message>, CoordinationError> {
        io::read_jsonl(&self.path)
    }

    /// Read every message, then truncate the inbox. Intended for the
    /// owner agent after it has processed the batch. Not atomic against
    /// a concurrent sender — a message arriving between read and
    /// truncate would be lost. For the cooperative coordination model
    /// this is acceptable; agents that need stronger semantics can use
    /// per-message receipts instead.
    pub fn drain(&self) -> Result<Vec<Message>, CoordinationError> {
        let messages = self.read_all()?;
        io::truncate_file(&self.path)?;
        Ok(messages)
    }

    /// Count pending messages without draining them.
    pub fn len(&self) -> Result<usize, CoordinationError> {
        Ok(self.read_all()?.len())
    }

    pub fn is_empty(&self) -> Result<bool, CoordinationError> {
        Ok(self.len()? == 0)
    }
}

/// Shared append-only log of broadcast messages.
///
/// Readers consume from an offset they track themselves — the log is
/// never truncated, so an agent that disconnects can catch up when it
/// reconnects by reading from its last-seen index.
#[derive(Debug, Clone)]
pub struct BroadcastLog {
    path: PathBuf,
}

impl BroadcastLog {
    /// Open (or create) the broadcast log at `path`. Parent directories
    /// are created if missing.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CoordinationError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| CoordinationError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append `msg` to the broadcast.
    pub fn publish(&self, msg: &Message) -> Result<(), CoordinationError> {
        io::append_jsonl(&self.path, msg)
    }

    /// Read every broadcast message, oldest first.
    pub fn read_all(&self) -> Result<Vec<Message>, CoordinationError> {
        io::read_jsonl(&self.path)
    }

    /// Read messages at index `since..` (inclusive lower bound). Useful
    /// when a reader tracks its own offset so it doesn't re-process
    /// messages. `since` past the current end returns an empty vec.
    pub fn read_since(&self, since: usize) -> Result<Vec<Message>, CoordinationError> {
        let all = self.read_all()?;
        if since >= all.len() {
            return Ok(Vec::new());
        }
        Ok(all[since..].to_vec())
    }

    /// Current number of broadcasts. Useful for initial offset capture
    /// on an agent that wants to ignore history.
    pub fn len(&self) -> Result<usize, CoordinationError> {
        Ok(self.read_all()?.len())
    }

    pub fn is_empty(&self) -> Result<bool, CoordinationError> {
        Ok(self.len()? == 0)
    }
}
