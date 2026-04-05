//! Queue types and error definitions

/// Errors that can occur in the task queue
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    /// Task not found
    #[error("Task not found: {0}")]
    NotFound(String),

    /// Task already exists
    #[error("Task already exists: {0}")]
    AlreadyExists(String),

    /// Invalid operation for task state
    #[error("Invalid operation for task state: {0}")]
    InvalidState(String),

    /// Queue is at capacity
    #[error("Queue is at capacity: {0}")]
    AtCapacity(usize),

    /// Task not created through orchestrator (missing required metadata)
    #[error("Task {0} was not created through the orchestrator")]
    NotFromOrchestrator(String),

    /// Dependency not satisfied
    #[error("Task dependency not satisfied: {0}")]
    DependencyNotSatisfied(String),
}

/// Statistics about the task queue
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    /// Total number of tasks
    pub total: usize,
    /// Tasks in backlog
    pub backlog: usize,
    /// Tasks queued
    pub queued: usize,
    /// Tasks in progress
    pub in_progress: usize,
    /// Tasks completed
    pub completed: usize,
    /// Tasks failed
    pub failed: usize,
    /// Tasks cancelled
    pub cancelled: usize,
    /// Tasks with unknown status
    pub unknown: usize,
}

/// Dependency information for a task
#[derive(Debug, Clone)]
pub struct TaskDependency {
    /// Task ID that has the dependency
    pub task_id: String,
    /// IDs of tasks this task depends on
    pub depends_on: Vec<String>,
}

impl TaskDependency {
    /// Create new dependency info
    pub fn new(task_id: impl Into<String>, depends_on: Vec<String>) -> Self {
        Self {
            task_id: task_id.into(),
            depends_on,
        }
    }

    /// Check if all dependencies are satisfied
    pub fn check_satisfied(&self, completed_tasks: &[String]) -> Result<(), String> {
        for dep_id in &self.depends_on {
            if !completed_tasks.contains(dep_id) {
                return Err(format!("Dependency {} not satisfied", dep_id));
            }
        }
        Ok(())
    }
}

/// Recursively merge a JSON patch into an existing JSON value
pub fn merge_json(existing: serde_json::Value, patch: serde_json::Value) -> serde_json::Value {
    match (existing, patch) {
        (serde_json::Value::Object(mut base), serde_json::Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                let merged = match base.remove(&key) {
                    Some(existing_value) => merge_json(existing_value, value),
                    None => value,
                };
                base.insert(key, merged);
            }
            serde_json::Value::Object(base)
        }
        (_, replacement) => replacement,
    }
}
