//! Pipeline Checkpoint Manager
//!
//! Provides crash recovery and resumability for pipeline tasks.
//! Follows Single Responsibility Principle - only handles checkpoint persistence.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::handlers::PhaseResult;
use super::phases::{Phase, Task};

/// Checkpoint data structure for task recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Task being executed
    pub task: Task,
    /// Completed phases and their results
    pub completed_phases: Vec<(Phase, PhaseResult)>,
    /// Timestamp when checkpoint was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Checkpoint version for migration support
    pub version: u32,
}

impl Checkpoint {
    /// Current checkpoint version
    const VERSION: u32 = 1;

    /// Create a new checkpoint for a task
    pub fn new(task: Task) -> Self {
        Self {
            task,
            completed_phases: Vec::new(),
            created_at: chrono::Utc::now(),
            version: Self::VERSION,
        }
    }

    /// Add a completed phase to the checkpoint
    pub fn add_phase_result(&mut self, phase: Phase, result: PhaseResult) {
        self.completed_phases.push((phase, result));
        self.created_at = chrono::Utc::now();
    }

    /// Get the last completed phase
    pub fn last_completed_phase(&self) -> Option<&(Phase, PhaseResult)> {
        self.completed_phases.last()
    }

    /// Check if a phase has been completed
    pub fn has_completed_phase(&self, phase: Phase) -> bool {
        self.completed_phases.iter().any(|(p, _)| *p == phase)
    }

    /// Get the result of a completed phase
    pub fn get_phase_result(&self, phase: Phase) -> Option<&PhaseResult> {
        self.completed_phases
            .iter()
            .find(|(p, _)| *p == phase)
            .map(|(_, result)| result)
    }
}

/// Manages checkpoint persistence and recovery
pub struct CheckpointManager {
    /// Directory to store checkpoints
    checkpoint_dir: PathBuf,
    /// In-memory cache of checkpoints
    cache: Arc<RwLock<Vec<Checkpoint>>>,
    /// Enable auto-save on phase completion
    auto_save: bool,
}

impl CheckpointManager {
    /// Create a new checkpoint manager
    pub fn new<P: AsRef<Path>>(checkpoint_dir: P) -> Self {
        Self {
            checkpoint_dir: checkpoint_dir.as_ref().to_path_buf(),
            cache: Arc::new(RwLock::new(Vec::new())),
            auto_save: true,
        }
    }

    /// Create a checkpoint manager without auto-save
    pub fn without_auto_save<P: AsRef<Path>>(checkpoint_dir: P) -> Self {
        Self {
            checkpoint_dir: checkpoint_dir.as_ref().to_path_buf(),
            cache: Arc::new(RwLock::new(Vec::new())),
            auto_save: false,
        }
    }

    /// Initialize the checkpoint directory
    pub async fn initialize(&self) -> Result<()> {
        fs::create_dir_all(&self.checkpoint_dir).await?;
        info!(
            "Checkpoint manager initialized at {:?}",
            self.checkpoint_dir
        );
        Ok(())
    }

    /// Create a checkpoint for a task
    pub async fn create_checkpoint(&self, task: Task) -> Result<Checkpoint> {
        let checkpoint = Checkpoint::new(task);

        // Add to cache
        let mut cache = self.cache.write().await;
        cache.push(checkpoint.clone());

        // Save to disk if auto-save enabled
        if self.auto_save {
            self.save_checkpoint(&checkpoint).await?;
        }

        debug!("Created checkpoint for task: {}", checkpoint.task.id);
        Ok(checkpoint)
    }

    /// Update an existing checkpoint
    pub async fn update_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        // Update in cache
        let mut cache = self.cache.write().await;
        if let Some(existing) = cache.iter_mut().find(|c| c.task.id == checkpoint.task.id) {
            *existing = checkpoint.clone();
        }

        // Save to disk if auto-save enabled
        if self.auto_save {
            self.save_checkpoint(checkpoint).await?;
        }

        debug!("Updated checkpoint for task: {}", checkpoint.task.id);
        Ok(())
    }

    /// Load a checkpoint for a task
    pub async fn load_checkpoint(&self, task_id: &str) -> Result<Option<Checkpoint>> {
        // Try cache first
        let cache = self.cache.read().await;
        if let Some(checkpoint) = cache.iter().find(|c| c.task.id == task_id) {
            debug!("Loaded checkpoint from cache for task: {}", task_id);
            return Ok(Some(checkpoint.clone()));
        }
        drop(cache);

        // Try to load from disk
        let checkpoint_path = self.get_checkpoint_path(task_id);
        if !checkpoint_path.exists() {
            debug!("No checkpoint found for task: {}", task_id);
            return Ok(None);
        }

        let contents = fs::read_to_string(&checkpoint_path).await?;
        let checkpoint: Checkpoint = serde_json::from_str(&contents)?;

        // Add to cache
        let mut cache = self.cache.write().await;
        cache.push(checkpoint.clone());

        info!("Loaded checkpoint from disk for task: {}", task_id);
        Ok(Some(checkpoint))
    }

    /// Delete a checkpoint
    pub async fn delete_checkpoint(&self, task_id: &str) -> Result<()> {
        // Remove from cache
        let mut cache = self.cache.write().await;
        cache.retain(|c| c.task.id != task_id);

        // Remove from disk
        let checkpoint_path = self.get_checkpoint_path(task_id);
        if checkpoint_path.exists() {
            fs::remove_file(&checkpoint_path).await?;
            debug!("Deleted checkpoint for task: {}", task_id);
        }

        Ok(())
    }

    /// List all available checkpoints
    pub async fn list_checkpoints(&self) -> Result<Vec<Checkpoint>> {
        let mut checkpoints = Vec::new();

        // Read from disk
        let mut entries = fs::read_dir(&self.checkpoint_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                match fs::read_to_string(&path).await {
                    Ok(contents) => match serde_json::from_str::<Checkpoint>(&contents) {
                        Ok(checkpoint) => checkpoints.push(checkpoint),
                        Err(e) => warn!("Failed to parse checkpoint {:?}: {}", path, e),
                    },
                    Err(e) => warn!("Failed to read checkpoint {:?}: {}", path, e),
                }
            }
        }

        // Sort by creation time (newest first)
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(checkpoints)
    }

    /// Get checkpoints for tasks that can be resumed
    pub async fn get_resumable_checkpoints(&self) -> Result<Vec<Checkpoint>> {
        let checkpoints = self.list_checkpoints().await?;

        // Filter to only include tasks that can be resumed
        let resumable: Vec<Checkpoint> = checkpoints
            .into_iter()
            .filter(|c| {
                // Task is resumable if:
                // 1. Not in a terminal state
                // 2. Not all phases are completed
                !c.task.status.is_terminal() && !c.task.phase.is_final()
            })
            .collect();

        info!("Found {} resumable checkpoints", resumable.len());
        Ok(resumable)
    }

    /// Clear all checkpoints
    pub async fn clear_all(&self) -> Result<()> {
        // Clear cache
        let mut cache = self.cache.write().await;
        cache.clear();

        // Clear disk
        if self.checkpoint_dir.exists() {
            let mut entries = fs::read_dir(&self.checkpoint_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "json") {
                    fs::remove_file(&path).await?;
                }
            }
        }

        info!("Cleared all checkpoints");
        Ok(())
    }

    /// Get the file path for a checkpoint
    fn get_checkpoint_path(&self, task_id: &str) -> PathBuf {
        self.checkpoint_dir
            .join(format!("checkpoint-{}.json", task_id))
    }

    /// Save a checkpoint to disk atomically using temp file + rename.
    async fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        let checkpoint_path = self.get_checkpoint_path(&checkpoint.task.id);

        if let Some(parent) = checkpoint_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let contents = serde_json::to_string_pretty(checkpoint)?;

        let temp_path = checkpoint_path.with_extension("tmp");
        fs::write(&temp_path, contents.as_bytes()).await?;

        tokio::fs::rename(&temp_path, &checkpoint_path).await?;

        debug!("Saved checkpoint to {:?}", checkpoint_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::phases::TaskStatus;
    use tempfile::TempDir;

    fn create_test_task() -> Task {
        Task::new("TEST-001", "Test task", "Test instruction")
    }

    #[tokio::test]
    async fn test_create_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::without_auto_save(temp_dir.path());
        manager.initialize().await.unwrap();

        let task = create_test_task();
        let checkpoint = manager.create_checkpoint(task.clone()).await.unwrap();

        assert_eq!(checkpoint.task.id, "TEST-001");
        assert_eq!(checkpoint.version, Checkpoint::VERSION);
        assert!(checkpoint.completed_phases.is_empty());
    }

    #[tokio::test]
    async fn test_add_phase_result() {
        let task = create_test_task();
        let mut checkpoint = Checkpoint::new(task);

        let result = PhaseResult::success("Research completed");
        checkpoint.add_phase_result(Phase::Research, result);

        assert_eq!(checkpoint.completed_phases.len(), 1);
        assert!(checkpoint.has_completed_phase(Phase::Research));
        assert!(!checkpoint.has_completed_phase(Phase::Plan));
    }

    #[tokio::test]
    async fn test_save_and_load_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path());
        manager.initialize().await.unwrap();

        let task = create_test_task().with_phase(Phase::Plan);
        let _checkpoint = manager.create_checkpoint(task).await.unwrap();

        // Load the checkpoint
        let loaded = manager.load_checkpoint("TEST-001").await.unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.task.id, "TEST-001");
        assert_eq!(loaded.task.phase, Phase::Plan);
    }

    #[tokio::test]
    async fn test_delete_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path());
        manager.initialize().await.unwrap();

        let task = create_test_task();
        manager.create_checkpoint(task).await.unwrap();

        manager.delete_checkpoint("TEST-001").await.unwrap();

        let loaded = manager.load_checkpoint("TEST-001").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_list_checkpoints() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path());
        manager.initialize().await.unwrap();

        // Create multiple checkpoints
        manager
            .create_checkpoint(Task::new("TASK-001", "Task 1", "Test"))
            .await
            .unwrap();
        manager
            .create_checkpoint(Task::new("TASK-002", "Task 2", "Test"))
            .await
            .unwrap();
        manager
            .create_checkpoint(Task::new("TASK-003", "Task 3", "Test"))
            .await
            .unwrap();

        let checkpoints = manager.list_checkpoints().await.unwrap();
        assert_eq!(checkpoints.len(), 3);
    }

    #[tokio::test]
    async fn test_resumable_checkpoints() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path());
        manager.initialize().await.unwrap();

        // Create a completed task (not resumable)
        let completed =
            Task::new("COMPLETED-001", "Completed", "Test").with_status(TaskStatus::Completed);
        manager.create_checkpoint(completed).await.unwrap();

        // Create an in-progress task (resumable)
        let in_progress = Task::new("IN_PROGRESS-001", "In Progress", "Test")
            .with_status(TaskStatus::InProgress)
            .with_phase(Phase::Plan);
        manager.create_checkpoint(in_progress).await.unwrap();

        let resumable = manager.get_resumable_checkpoints().await.unwrap();
        assert_eq!(resumable.len(), 1);
        assert_eq!(resumable[0].task.id, "IN_PROGRESS-001");
    }

    #[tokio::test]
    async fn test_clear_all() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path());
        manager.initialize().await.unwrap();

        manager
            .create_checkpoint(Task::new("TASK-001", "Task 1", "Test"))
            .await
            .unwrap();
        manager
            .create_checkpoint(Task::new("TASK-002", "Task 2", "Test"))
            .await
            .unwrap();

        manager.clear_all().await.unwrap();

        let checkpoints = manager.list_checkpoints().await.unwrap();
        assert_eq!(checkpoints.len(), 0);
    }

    #[tokio::test]
    async fn test_atomic_write_no_temp_file_left_behind() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path());
        manager.initialize().await.unwrap();

        manager
            .create_checkpoint(Task::new("ATOMIC-001", "Atomic Test", "Test"))
            .await
            .unwrap();

        let checkpoint_path = temp_dir.path().join("checkpoint-ATOMIC-001.json");
        let temp_path = temp_dir.path().join("checkpoint-ATOMIC-001.tmp");

        assert!(checkpoint_path.exists(), "checkpoint file should exist");
        assert!(
            !temp_path.exists(),
            "temp file should be cleaned up after rename"
        );
    }

    #[tokio::test]
    async fn test_atomic_write_overwrites_consistently() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path());
        manager.initialize().await.unwrap();

        let task1 = Task::new("OVERWRITE-001", "Version 1", "Test");
        manager.create_checkpoint(task1).await.unwrap();

        let task2 = Task::new("OVERWRITE-001", "Version 2", "Test").with_phase(Phase::Plan);
        let checkpoint = manager.create_checkpoint(task2).await.unwrap();

        manager.update_checkpoint(&checkpoint).await.unwrap();

        let loaded = manager
            .load_checkpoint("OVERWRITE-001")
            .await
            .unwrap()
            .expect("should load updated checkpoint");
        assert_eq!(loaded.task.title, "Version 2");
        assert_eq!(loaded.task.phase, Phase::Plan);
    }
}
