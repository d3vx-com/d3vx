//! Worker/Session Ownership and Generation Management
//!
//! Provides epoch-based ownership tracking to prevent stale workers or resumed
//! sessions from acting as if they still own a task after a newer owner has
//! taken over.
//!
//! # Concepts
//!
//! - **Generation**: Monotonically increasing epoch for a task/session
//! - **Owner**: Entity (worker or session) that holds ownership
//! - **OwnerToken**: Proof of ownership with generation embedded
//! - **Stale**: An owner whose generation is lower than the current one
//!
//! # State Transitions
//!
//! ```text
//! Task Created: generation=1, owner=None
//! Worker A acquires: generation=1, owner=A
//! Worker A crashes
//! Worker B recovers task: generation=2, owner=B
//! Worker A's lease expires but worker process might still run
//! Worker A tries to update: REJECTED (stale, generation mismatch)
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Errors in ownership management.
#[derive(Debug, thiserror::Error)]
pub enum OwnershipError {
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Ownership conflict: current owner is {current:?}, not {requestor:?}")]
    Conflict {
        current: OwnerId,
        requestor: OwnerId,
    },

    #[error("Stale owner detected (generation {stale} < current {current})")]
    StaleOwner { stale: u64, current: u64 },

    #[error("Invalid owner token")]
    InvalidToken,

    #[error("Task already owned by another entity")]
    AlreadyOwned,
}

/// Identifier for an owner (worker or session).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OwnerId {
    /// Type of owner (worker or session).
    pub owner_type: OwnerType,
    /// Unique identifier within the type.
    pub id: String,
}

impl OwnerId {
    /// Create a new worker owner ID.
    pub fn worker(id: u64) -> Self {
        Self {
            owner_type: OwnerType::Worker,
            id: format!("worker-{}", id),
        }
    }

    /// Create a new session owner ID.
    pub fn session(id: &str) -> Self {
        Self {
            owner_type: OwnerType::Session,
            id: id.to_string(),
        }
    }

    /// Get a display string for this owner.
    pub fn display(&self) -> String {
        match self.owner_type {
            OwnerType::Worker => format!("Worker({})", self.id),
            OwnerType::Session => format!("Session({})", self.id),
        }
    }
}

impl std::fmt::Display for OwnerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

/// Type of owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerType {
    Worker,
    Session,
}

/// A token proving ownership of a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerToken {
    /// The task this token owns.
    pub task_id: String,
    /// The owner holding this token.
    pub owner: OwnerId,
    /// Generation of the ownership (monotonically increasing).
    pub generation: u64,
    /// When the ownership was acquired.
    pub acquired_at: DateTime<Utc>,
    /// Optional lease ID for this ownership.
    pub lease_id: Option<u64>,
}

impl OwnerToken {
    /// Get the task ID.
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Get the generation.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Get the owner.
    pub fn owner(&self) -> &OwnerId {
        &self.owner
    }

    /// Check if this token is valid for the given generation.
    pub fn is_valid_for(&self, generation: u64) -> bool {
        self.generation >= generation
    }

    /// Check if this token is stale (lower generation than current).
    pub fn is_stale(&self, current_generation: u64) -> bool {
        self.generation < current_generation
    }
}

/// Current ownership state for a task.
#[derive(Debug, Clone)]
pub struct OwnershipState {
    /// Task ID.
    pub task_id: String,
    /// Current generation (monotonically increasing).
    pub generation: u64,
    /// Current owner, if any.
    pub current_owner: Option<OwnerId>,
    /// Token held by current owner.
    pub token: Option<OwnerToken>,
    /// When ownership was last acquired.
    pub last_acquired: Option<DateTime<Utc>>,
    /// Previous owners (for auditing).
    pub history: Vec<OwnershipEvent>,
}

impl OwnershipState {
    /// Create a new ownership state for a task.
    pub fn new(task_id: String) -> Self {
        Self {
            task_id,
            generation: 0,
            current_owner: None,
            token: None,
            last_acquired: None,
            history: Vec::new(),
        }
    }

    /// Get the current generation.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Check if there is a current owner.
    pub fn is_owned(&self) -> bool {
        self.current_owner.is_some()
    }

    /// Check if the given owner is the current owner.
    pub fn is_owner(&self, owner: &OwnerId) -> bool {
        self.current_owner.as_ref() == Some(owner)
    }

    /// Check if the given token is still valid (current generation).
    pub fn is_token_valid(&self, token: &OwnerToken) -> bool {
        token.generation >= self.generation
    }

    /// Get the current owner.
    pub fn owner(&self) -> Option<&OwnerId> {
        self.current_owner.as_ref()
    }
}

/// Event in the ownership history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipEvent {
    /// When this event occurred.
    pub timestamp: DateTime<Utc>,
    /// Type of event.
    pub event_type: OwnershipEventType,
    /// Owner involved.
    pub owner: Option<OwnerId>,
    /// Generation at time of event.
    pub generation: u64,
    /// Optional reason.
    pub reason: Option<String>,
}

/// Type of ownership event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipEventType {
    /// Task initialized (no owner).
    Initialized,
    /// Ownership acquired.
    Acquired,
    /// Ownership transferred (new owner took over).
    Transferred,
    /// Ownership released voluntarily.
    Released,
    /// Ownership revoked (forced, e.g., due to staleness).
    Revoked,
    /// Ownership expired (lease timeout).
    Expired,
}

/// Result of an ownership operation.
#[derive(Debug, Clone)]
pub struct OwnershipResult {
    /// Whether the operation succeeded.
    pub success: bool,
    /// The ownership state after the operation.
    pub state: OwnershipState,
    /// Token issued (if applicable).
    pub token: Option<OwnerToken>,
    /// Error message (if failed).
    pub error: Option<String>,
}

/// Ownership manager - tracks task ownership with generation support.
pub struct OwnershipManager {
    /// Task ownership states.
    tasks: Arc<RwLock<HashMap<String, OwnershipState>>>,
    /// Tokens by task ID (for quick lookup).
    tokens: Arc<RwLock<HashMap<String, OwnerToken>>>,
}

impl Default for OwnershipManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OwnershipManager {
    /// Create a new ownership manager.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize or get ownership state for a task.
    pub async fn get_or_create(&self, task_id: &str) -> OwnershipState {
        let mut tasks = self.tasks.write().await;
        if let Some(state) = tasks.get(task_id) {
            return state.clone();
        }
        let state = OwnershipState::new(task_id.to_string());
        let entry = state.clone();
        tasks.insert(task_id.to_string(), state);
        entry
    }

    /// Acquire ownership of a task.
    ///
    /// If the task is already owned by another entity, this will:
    /// - If `force` is true: revoke the current owner's ownership and transfer
    /// - If `force` is false: return an error
    pub async fn acquire(
        &self,
        task_id: &str,
        owner: OwnerId,
        lease_id: Option<u64>,
        force: bool,
    ) -> OwnershipResult {
        let state = self.get_or_create(task_id).await;

        // Check if already owned by another entity
        if let Some(ref current) = state.current_owner {
            if current != &owner {
                if !force {
                    let current_clone = current.clone();
                    return OwnershipResult {
                        success: false,
                        state,
                        token: None,
                        error: Some(format!(
                            "Task {} already owned by {}",
                            task_id, current_clone
                        )),
                    };
                }

                // Force transfer - increment generation
                info!(
                    "Force-transferring task {} from {} to {}",
                    task_id, current, owner
                );
            }
        }

        // Increment generation
        let new_generation = state.generation + 1;
        let now = Utc::now();

        let token = OwnerToken {
            task_id: task_id.to_string(),
            owner: owner.clone(),
            generation: new_generation,
            acquired_at: now,
            lease_id,
        };

        let event = OwnershipEvent {
            timestamp: now,
            event_type: if state.current_owner.is_some() {
                OwnershipEventType::Transferred
            } else {
                OwnershipEventType::Acquired
            },
            owner: Some(owner.clone()),
            generation: new_generation,
            reason: if force {
                Some("Force transfer".to_string())
            } else {
                None
            },
        };

        let mut tasks = self.tasks.write().await;
        let state = tasks.get_mut(task_id).unwrap();

        state.generation = new_generation;
        state.current_owner = Some(owner);
        state.token = Some(token.clone());
        state.last_acquired = Some(now);
        state.history.push(event);

        let final_state = state.clone();

        {
            let mut tokens = self.tokens.write().await;
            tokens.insert(task_id.to_string(), token.clone());
        }

        OwnershipResult {
            success: true,
            state: final_state,
            token: Some(token),
            error: None,
        }
    }

    /// Release ownership of a task.
    pub async fn release(&self, task_id: &str, owner: &OwnerId) -> OwnershipResult {
        let mut tasks = self.tasks.write().await;
        let state = match tasks.get_mut(task_id) {
            Some(s) => s,
            None => {
                return OwnershipResult {
                    success: false,
                    state: OwnershipState::new(task_id.to_string()),
                    token: None,
                    error: Some(format!("Task {} not found", task_id)),
                };
            }
        };

        // Verify owner matches
        if state.current_owner.as_ref() != Some(owner) {
            return OwnershipResult {
                success: false,
                state: state.clone(),
                token: None,
                error: Some(format!("Not the current owner of {}", task_id)),
            };
        }

        let now = Utc::now();
        let event = OwnershipEvent {
            timestamp: now,
            event_type: OwnershipEventType::Released,
            owner: Some(owner.clone()),
            generation: state.generation,
            reason: None,
        };

        state.current_owner = None;
        state.token = None;
        state.history.push(event);

        {
            let mut tokens = self.tokens.write().await;
            tokens.remove(task_id);
        }

        OwnershipResult {
            success: true,
            state: state.clone(),
            token: None,
            error: None,
        }
    }

    /// Validate an ownership token.
    ///
    /// Returns Ok if the token is valid (current generation matches).
    /// Returns Err with details if stale or invalid.
    pub async fn validate_token(
        &self,
        task_id: &str,
        token: &OwnerToken,
    ) -> Result<OwnerToken, OwnershipError> {
        let tasks = self.tasks.read().await;
        let state = tasks
            .get(task_id)
            .ok_or_else(|| OwnershipError::TaskNotFound(task_id.to_string()))?;

        if token.generation < state.generation {
            return Err(OwnershipError::StaleOwner {
                stale: token.generation,
                current: state.generation,
            });
        }

        if token.owner
            != state
                .current_owner
                .as_ref()
                .cloned()
                .unwrap_or_else(|| OwnerId {
                    owner_type: OwnerType::Session,
                    id: String::new(),
                })
        {
            return Err(OwnershipError::InvalidToken);
        }

        Ok(token.clone())
    }

    /// Check if an update from an owner should be accepted.
    ///
    /// An update is accepted if:
    /// - The owner matches the current owner, OR
    /// - The owner has a token with current or higher generation
    pub async fn can_update(&self, task_id: &str, owner: &OwnerId) -> bool {
        let tasks = self.tasks.read().await;
        let state = match tasks.get(task_id) {
            Some(s) => s,
            None => return true, // New task, accept
        };

        // If no current owner, accept
        if state.current_owner.is_none() {
            return true;
        }

        // If owner matches, accept
        if state.current_owner.as_ref() == Some(owner) {
            return true;
        }

        false
    }

    /// Check if an owner is stale (lower generation than current).
    pub async fn is_stale(&self, task_id: &str, owner: &OwnerId, generation: u64) -> bool {
        let tasks = self.tasks.read().await;
        if let Some(state) = tasks.get(task_id) {
            return state.generation > generation && state.current_owner.as_ref() != Some(owner);
        }
        false
    }

    /// Get the current ownership state for a task.
    pub async fn get(&self, task_id: &str) -> Option<OwnershipState> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).cloned()
    }

    /// Get the current token for a task.
    pub async fn get_token(&self, task_id: &str) -> Option<OwnerToken> {
        let tokens = self.tokens.read().await;
        tokens.get(task_id).cloned()
    }

    /// Get all tasks owned by an owner.
    pub async fn tasks_owned_by(&self, owner: &OwnerId) -> Vec<String> {
        let tasks = self.tasks.read().await;
        tasks
            .iter()
            .filter(|(_, state)| state.current_owner.as_ref() == Some(owner))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Revoke ownership due to staleness (force transfer to new owner).
    pub async fn revoke(
        &self,
        task_id: &str,
        reason: &str,
    ) -> Result<OwnershipState, OwnershipError> {
        let mut tasks = self.tasks.write().await;
        let state = tasks
            .get_mut(task_id)
            .ok_or_else(|| OwnershipError::TaskNotFound(task_id.to_string()))?;

        let now = Utc::now();
        let event = OwnershipEvent {
            timestamp: now,
            event_type: OwnershipEventType::Revoked,
            owner: state.current_owner.clone(),
            generation: state.generation,
            reason: Some(reason.to_string()),
        };

        state.generation += 1;
        state.current_owner = None;
        state.token = None;
        state.history.push(event);

        {
            let mut tokens = self.tokens.write().await;
            tokens.remove(task_id);
        }

        Ok(state.clone())
    }

    /// Get ownership statistics.
    pub async fn stats(&self) -> OwnershipStats {
        let tasks = self.tasks.read().await;

        let mut owned = 0;
        let mut unowned = 0;
        let mut by_owner: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for state in tasks.values() {
            if state.is_owned() {
                owned += 1;
                if let Some(owner) = &state.current_owner {
                    let key = owner.display();
                    *by_owner.entry(key).or_insert(0) += 1;
                }
            } else {
                unowned += 1;
            }
        }

        OwnershipStats {
            total_tasks: tasks.len(),
            owned_tasks: owned,
            unowned_tasks: unowned,
            tasks_by_owner: by_owner,
        }
    }
}

/// Statistics about ownership.
#[derive(Debug, Clone)]
pub struct OwnershipStats {
    pub total_tasks: usize,
    pub owned_tasks: usize,
    pub unowned_tasks: usize,
    pub tasks_by_owner: std::collections::HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_acquire_unowned_task() {
        let mgr = OwnershipManager::new();

        let result = mgr.acquire("task-1", OwnerId::worker(1), None, false).await;

        assert!(result.success);
        assert!(result.token.is_some());
        assert_eq!(result.state.generation, 1);
        assert_eq!(result.state.current_owner, Some(OwnerId::worker(1)));
    }

    #[tokio::test]
    async fn test_acquire_already_owned_task() {
        let mgr = OwnershipManager::new();

        // First worker acquires
        mgr.acquire("task-1", OwnerId::worker(1), None, false).await;

        // Second worker tries to acquire (not force)
        let result = mgr.acquire("task-1", OwnerId::worker(2), None, false).await;

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_force_transfer_increments_generation() {
        let mgr = OwnershipManager::new();

        // First worker acquires
        let r1 = mgr.acquire("task-1", OwnerId::worker(1), None, false).await;
        assert_eq!(r1.state.generation, 1);

        // Second worker force-acquires
        let r2 = mgr.acquire("task-1", OwnerId::worker(2), None, true).await;
        assert!(r2.success);
        assert_eq!(r2.state.generation, 2);
        assert_eq!(r2.state.current_owner, Some(OwnerId::worker(2)));
    }

    #[tokio::test]
    async fn test_stale_owner_detection() {
        let mgr = OwnershipManager::new();

        // Worker A acquires
        let r1 = mgr.acquire("task-1", OwnerId::worker(1), None, false).await;
        let token_a = r1.token.unwrap();

        // Worker B force-acquires
        mgr.acquire("task-1", OwnerId::worker(2), None, true).await;

        // Worker A's token is now stale
        let is_stale = mgr
            .is_stale("task-1", &OwnerId::worker(1), token_a.generation)
            .await;
        assert!(is_stale);

        // Worker B's token is not stale
        let is_stale_b = mgr.is_stale("task-1", &OwnerId::worker(2), 2).await;
        assert!(!is_stale_b);
    }

    #[tokio::test]
    async fn test_token_validation() {
        let mgr = OwnershipManager::new();

        // Acquire as worker 1
        let result = mgr.acquire("task-1", OwnerId::worker(1), None, false).await;
        let token = result.token.unwrap();

        // Token should be valid
        let validated = mgr.validate_token("task-1", &token).await;
        assert!(validated.is_ok());

        // Force acquire as worker 2
        mgr.acquire("task-1", OwnerId::worker(2), None, true).await;

        // Original token should be stale now
        let validated_stale = mgr.validate_token("task-1", &token).await;
        assert!(validated_stale.is_err());
    }

    #[tokio::test]
    async fn test_release_ownership() {
        let mgr = OwnershipManager::new();

        // Acquire
        mgr.acquire("task-1", OwnerId::worker(1), None, false).await;

        // Release
        let result = mgr.release("task-1", &OwnerId::worker(1)).await;
        assert!(result.success);
        assert!(!result.state.is_owned());

        // Can now be acquired by anyone
        let r2 = mgr.acquire("task-1", OwnerId::worker(2), None, false).await;
        assert!(r2.success);
    }

    #[tokio::test]
    async fn test_release_by_non_owner_fails() {
        let mgr = OwnershipManager::new();

        // Worker 1 acquires
        mgr.acquire("task-1", OwnerId::worker(1), None, false).await;

        // Worker 2 tries to release (not the owner)
        let result = mgr.release("task-1", &OwnerId::worker(2)).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_tasks_owned_by() {
        let mgr = OwnershipManager::new();

        // Worker 1 owns task-1
        mgr.acquire("task-1", OwnerId::worker(1), None, false).await;

        // Worker 1 owns task-2
        mgr.acquire("task-2", OwnerId::worker(1), None, false).await;

        // Worker 2 owns task-3
        mgr.acquire("task-3", OwnerId::worker(2), None, false).await;

        let owned_by_1 = mgr.tasks_owned_by(&OwnerId::worker(1)).await;
        assert_eq!(owned_by_1.len(), 2);
        assert!(owned_by_1.contains(&"task-1".to_string()));
        assert!(owned_by_1.contains(&"task-2".to_string()));
    }

    #[tokio::test]
    async fn test_generation_continues_after_multiple_transfers() {
        let mgr = OwnershipManager::new();

        // Acquire sequence
        let r1 = mgr.acquire("task-1", OwnerId::worker(1), None, false).await;
        assert_eq!(r1.state.generation, 1);

        let r2 = mgr.acquire("task-1", OwnerId::worker(2), None, true).await;
        assert_eq!(r2.state.generation, 2);

        let r3 = mgr
            .acquire("task-1", OwnerId::session("sess-1"), None, true)
            .await;
        assert_eq!(r3.state.generation, 3);

        // Release and re-acquire
        mgr.release("task-1", &OwnerId::session("sess-1")).await;

        let r4 = mgr.acquire("task-1", OwnerId::worker(1), None, false).await;
        assert_eq!(r4.state.generation, 4);
    }

    #[tokio::test]
    async fn test_can_update() {
        let mgr = OwnershipManager::new();

        // Worker 1 acquires
        mgr.acquire("task-1", OwnerId::worker(1), None, false).await;

        // Worker 1 can update
        assert!(mgr.can_update("task-1", &OwnerId::worker(1)).await);

        // Worker 2 cannot update
        assert!(!mgr.can_update("task-1", &OwnerId::worker(2)).await);

        // After force transfer, worker 1 cannot update
        mgr.acquire("task-1", OwnerId::worker(2), None, true).await;
        assert!(!mgr.can_update("task-1", &OwnerId::worker(1)).await);
        assert!(mgr.can_update("task-1", &OwnerId::worker(2)).await);
    }

    #[tokio::test]
    async fn test_stats() {
        let mgr = OwnershipManager::new();

        mgr.acquire("task-1", OwnerId::worker(1), None, false).await;
        mgr.acquire("task-2", OwnerId::worker(1), None, false).await;
        mgr.acquire("task-3", OwnerId::worker(2), None, false).await;

        let stats = mgr.stats().await;
        assert_eq!(stats.total_tasks, 3);
        assert_eq!(stats.owned_tasks, 3);
        assert_eq!(stats.unowned_tasks, 0);
    }

    #[tokio::test]
    async fn test_history_preserved() {
        let mgr = OwnershipManager::new();

        // Acquire
        mgr.acquire("task-1", OwnerId::worker(1), None, false).await;

        // Force transfer
        mgr.acquire("task-1", OwnerId::worker(2), None, true).await;

        let state = mgr.get("task-1").await.unwrap();
        assert_eq!(state.history.len(), 2);
        assert!(matches!(
            state.history[0].event_type,
            OwnershipEventType::Acquired
        ));
        assert!(matches!(
            state.history[1].event_type,
            OwnershipEventType::Transferred
        ));
    }
}
