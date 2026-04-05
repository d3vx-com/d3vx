//! Sub-agent management
//!
//! Handles spawning and tracking parallel agent loops.

pub mod mailbox;
mod management;
mod spawn;
mod spawn_inline;
mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

// Re-export all public types
pub use mailbox::{AgentMessage, MailboxRegistry};
pub use types::{InlineCallback, SubAgentHandle, SubAgentStatus};

/// Manages sub-agent lifecycles: spawning, tracking, and cleanup.
pub struct SubAgentManager {
    pub(crate) agents: Arc<RwLock<HashMap<String, SubAgentHandle>>>,
    pub(crate) db: Option<crate::store::database::DatabaseHandle>,
    pub(crate) broadcast_tx: broadcast::Sender<crate::agent::AgentEvent>,
}

impl SubAgentManager {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            db: None,
            broadcast_tx: tx,
        }
    }

    pub fn with_db(mut self, db: crate::store::database::DatabaseHandle) -> Self {
        self.db = Some(db);
        self
    }
}

impl Default for SubAgentManager {
    fn default() -> Self {
        Self::new()
    }
}
