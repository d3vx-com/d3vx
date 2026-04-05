//! Idempotency Guards
//!
//! Prevents duplicate side effects during recovery and restart scenarios.
//! Tracks which operations have been performed to avoid repeating them.
//!
//! ## Usage
//!
//! ```rust
//! use recovery::idempotency::{IdempotencyGuard, OperationId};
//!
//! let guard = IdempotencyGuard::new();
//!
//! // Check if we've already posted a comment
//! if guard.can_perform("post_comment") {
//!     post_comment().await;
//!     guard.mark_performed("post_comment");
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Unique identifier for an operation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationId {
    /// Session or task ID
    pub context_id: String,
    /// Operation name (e.g., "post_comment", "create_pr", "send_notification")
    pub operation: String,
    /// Optional operation-specific key (e.g., comment ID)
    pub key: Option<String>,
}

impl OperationId {
    pub fn new(context_id: &str, operation: &str) -> Self {
        Self {
            context_id: context_id.to_string(),
            operation: operation.to_string(),
            key: None,
        }
    }

    pub fn with_key(mut self, key: &str) -> Self {
        self.key = Some(key.to_string());
        self
    }

    pub fn as_str(&self) -> String {
        match &self.key {
            Some(k) => format!("{}:{}:{}", self.context_id, self.operation, k),
            None => format!("{}:{}", self.context_id, self.operation),
        }
    }
}

/// Tracks which operations have been performed for a context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformedOperations {
    /// Operations that completed successfully
    completed: HashSet<String>,
    /// Operations that failed (can be retried)
    failed: HashSet<String>,
}

impl PerformedOperations {
    pub fn new() -> Self {
        Self {
            completed: HashSet::new(),
            failed: HashSet::new(),
        }
    }

    pub fn is_completed(&self, op: &str) -> bool {
        self.completed.contains(op)
    }

    pub fn is_failed(&self, op: &str) -> bool {
        self.failed.contains(op)
    }

    pub fn mark_completed(&mut self, op: &str) {
        self.completed.insert(op.to_string());
        self.failed.remove(op);
    }

    pub fn mark_failed(&mut self, op: &str) {
        self.failed.insert(op.to_string());
    }

    pub fn clear_failed(&mut self, op: &str) {
        self.failed.remove(op);
    }
}

impl Default for PerformedOperations {
    fn default() -> Self {
        Self::new()
    }
}

/// Guards against duplicate side effects during recovery
pub struct IdempotencyGuard {
    /// In-memory tracking (for single-process scenarios)
    memory: Arc<RwLock<PerformedOperations>>,
}

impl IdempotencyGuard {
    pub fn new() -> Self {
        Self {
            memory: Arc::new(RwLock::new(PerformedOperations::new())),
        }
    }

    pub fn with_arc() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Check if an operation can be performed (hasn't completed yet)
    pub async fn can_perform(&self, operation: &str) -> bool {
        let ops = self.memory.read().await;
        !ops.is_completed(operation)
    }

    /// Check if an operation has completed
    pub async fn is_completed(&self, operation: &str) -> bool {
        let ops = self.memory.read().await;
        ops.is_completed(operation)
    }

    /// Mark an operation as completed
    pub async fn mark_completed(&self, operation: &str) {
        let mut ops = self.memory.write().await;
        ops.mark_completed(operation);
    }

    /// Mark an operation as failed (can be retried)
    pub async fn mark_failed(&self, operation: &str) {
        let mut ops = self.memory.write().await;
        ops.mark_failed(operation);
    }

    /// Clear a failed operation so it can be retried
    pub async fn clear_failed(&self, operation: &str) {
        let mut ops = self.memory.write().await;
        ops.clear_failed(operation);
    }

    /// Perform an operation if not already completed, tracking success/failure
    pub async fn perform<F, Fut>(&self, operation: &str, f: F) -> Result<(), IdempotencyError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), IdempotencyError>>,
    {
        if !self.can_perform(operation).await {
            return Err(IdempotencyError::AlreadyPerformed(operation.to_string()));
        }

        match f().await {
            Ok(()) => {
                self.mark_completed(operation).await;
                Ok(())
            }
            Err(e) => {
                self.mark_failed(operation).await;
                Err(e)
            }
        }
    }

    /// Load state from persistence
    pub async fn load_from(&self, ops: PerformedOperations) {
        let mut memory = self.memory.write().await;
        *memory = ops;
    }

    /// Get current state for persistence
    pub async fn get_state(&self) -> PerformedOperations {
        self.memory.read().await.clone()
    }
}

/// Error types for idempotency operations
#[derive(Debug, thiserror::Error)]
pub enum IdempotencyError {
    #[error("Operation '{0}' has already been performed")]
    AlreadyPerformed(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_first_perform_succeeds() {
        let guard = IdempotencyGuard::new();
        assert!(guard.can_perform("test_op").await);
    }

    #[tokio::test]
    async fn test_mark_completed_blocks_repeat() {
        let guard = IdempotencyGuard::new();
        guard.mark_completed("test_op").await;
        assert!(!guard.can_perform("test_op").await);
    }

    #[tokio::test]
    async fn test_failed_can_be_retried() {
        let guard = IdempotencyGuard::new();
        guard.mark_failed("test_op").await;
        assert!(guard.can_perform("test_op").await); // Can retry
    }

    #[tokio::test]
    async fn test_clear_failed() {
        let guard = IdempotencyGuard::new();
        guard.mark_completed("test_op").await;
        guard.clear_failed("test_op").await;
        assert!(!guard.can_perform("test_op").await);
    }

    #[tokio::test]
    async fn test_operation_id() {
        let id = OperationId::new("sess-1", "post_comment").with_key("comment-123");
        assert_eq!(id.as_str(), "sess-1:post_comment:comment-123");
    }

    #[tokio::test]
    async fn test_perform_tracking() {
        let guard = IdempotencyGuard::new();
        let call_count = std::sync::atomic::AtomicUsize::new(0);

        let guard = guard.clone();
        let result = guard
            .perform("test_op", || async {
                call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Second call should be blocked
        let result = guard
            .perform("test_op", || async {
                call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
            .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1); // Not incremented
    }
}

impl Clone for IdempotencyGuard {
    fn clone(&self) -> Self {
        Self {
            memory: Arc::clone(&self.memory),
        }
    }
}
