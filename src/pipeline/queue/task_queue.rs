//! Task queue implementation - core mutation operations

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::types::{merge_json, QueueError, TaskDependency};
use crate::pipeline::phases::{Priority, Task, TaskStatus};

/// The task queue for managing multiple tasks
///
/// # Usage
///
/// This queue is an internal component of the pipeline. Task creation must go
/// through [`PipelineOrchestrator`](super::super::orchestrator::PipelineOrchestrator)
/// (or its [`TaskAuthority`](super::super::orchestrator::TaskAuthority) trait),
/// which guarantees that tasks are classified, routed, and tracked before enqueueing.
///
/// [`add_task`](Self::add_task) is `pub(in crate::pipeline)`: crate-external callers
/// cannot bypass the orchestrator. The runtime metadata check on
/// [`with_orchestrator_enforcement`] queues remains as defense-in-depth against
/// accidental intra-pipeline misuse.
///
/// ```ignore
/// // DO this:
/// orchestrator.create_task_from_chat(title, instruction, priority).await?;
/// ```
pub struct TaskQueue {
    /// All tasks indexed by ID
    pub(crate) tasks: RwLock<HashMap<String, Task>>,
    /// Priority-ordered task IDs (priority -> task IDs)
    pub(crate) priority_queue: RwLock<BTreeMap<Priority, VecDeque<String>>>,
    /// Maximum queue capacity
    max_capacity: usize,
    /// Callback when a task is added
    on_task_added: RwLock<Option<Arc<dyn Fn(&Task) + Send + Sync>>>,
    /// Callback when a task status changes
    on_status_changed: RwLock<Option<Arc<dyn Fn(&Task, TaskStatus) + Send + Sync>>>,
    /// Task dependencies
    pub(crate) dependencies: RwLock<HashMap<String, TaskDependency>>,
    /// Whether to enforce orchestrator-only creation
    enforce_orchestrator_only: bool,
}

impl TaskQueue {
    /// Create a new task queue
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
            priority_queue: RwLock::new(BTreeMap::new()),
            max_capacity: usize::MAX,
            on_task_added: RwLock::new(None),
            on_status_changed: RwLock::new(None),
            dependencies: RwLock::new(HashMap::new()),
            enforce_orchestrator_only: false,
        }
    }

    /// Create a new task queue with a maximum capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
            priority_queue: RwLock::new(BTreeMap::new()),
            max_capacity: capacity,
            on_task_added: RwLock::new(None),
            on_status_changed: RwLock::new(None),
            dependencies: RwLock::new(HashMap::new()),
            enforce_orchestrator_only: false,
        }
    }

    /// Create a task queue that enforces orchestrator-only task creation
    pub fn with_orchestrator_enforcement() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
            priority_queue: RwLock::new(BTreeMap::new()),
            max_capacity: usize::MAX,
            on_task_added: RwLock::new(None),
            on_status_changed: RwLock::new(None),
            dependencies: RwLock::new(HashMap::new()),
            enforce_orchestrator_only: true,
        }
    }

    /// Set the callback for when a task is added
    pub async fn on_task_added(&self, callback: Arc<dyn Fn(&Task) + Send + Sync>) {
        let mut cb = self.on_task_added.write().await;
        *cb = Some(callback);
    }

    /// Set the callback for when a task status changes
    pub async fn on_status_changed(&self, callback: Arc<dyn Fn(&Task, TaskStatus) + Send + Sync>) {
        let mut cb = self.on_status_changed.write().await;
        *cb = Some(callback);
    }

    /// Add a task to the queue.
    ///
    /// Restricted to the `pipeline` module tree so only the orchestrator
    /// (and pipeline-internal helpers like `TaskFactory` and the decomposition
    /// executor) can enqueue tasks. External callers must go through
    /// [`PipelineOrchestrator`](super::super::orchestrator::PipelineOrchestrator).
    pub(in crate::pipeline) async fn add_task(&self, task: Task) -> Result<(), QueueError> {
        let task_id = task.id.clone();
        let priority = task.priority;
        let status = task.status;

        if self.enforce_orchestrator_only {
            let is_from_orchestrator = if let serde_json::Value::Object(map) = &task.metadata {
                map.contains_key("classification") || map.contains_key("source")
            } else {
                false
            };

            if !is_from_orchestrator {
                return Err(QueueError::NotFromOrchestrator(task_id));
            }
        }

        let tasks = self.tasks.read().await;
        if tasks.len() >= self.max_capacity {
            return Err(QueueError::AtCapacity(self.max_capacity));
        }
        drop(tasks);

        let mut tasks = self.tasks.write().await;
        if tasks.contains_key(&task_id) {
            return Err(QueueError::AlreadyExists(task_id));
        }

        let deps = self.extract_dependencies(&task);
        if !deps.depends_on.is_empty() {
            let completed: Vec<String> = tasks
                .values()
                .filter(|t| t.status == TaskStatus::Completed)
                .map(|t| t.id.clone())
                .collect();

            if let Err(e) = deps.check_satisfied(&completed) {
                return Err(QueueError::DependencyNotSatisfied(e));
            }

            self.dependencies
                .write()
                .await
                .insert(task_id.clone(), deps);
        }

        tasks.insert(task_id.clone(), task.clone());
        drop(tasks);

        if status == TaskStatus::Queued {
            let mut pq = self.priority_queue.write().await;
            pq.entry(priority)
                .or_insert_with(VecDeque::new)
                .push_back(task_id.clone());
        }

        info!("Added task {} with priority {}", task_id, priority);

        let cb = self.on_task_added.read().await;
        if let Some(callback) = cb.as_ref() {
            callback(&task);
        }

        Ok(())
    }

    fn extract_dependencies(&self, task: &Task) -> TaskDependency {
        let depends_on = if let serde_json::Value::Object(map) = &task.metadata {
            map.get("depends_on")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        TaskDependency::new(&task.id, depends_on)
    }

    /// Get a task by ID
    pub async fn get_task(&self, id: &str) -> Option<Task> {
        let tasks = self.tasks.read().await;
        tasks.get(id).cloned()
    }

    /// Merge additional metadata into an existing task.
    pub async fn update_metadata(
        &self,
        id: &str,
        patch: serde_json::Value,
    ) -> Result<Task, QueueError> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(id)
            .ok_or_else(|| QueueError::NotFound(id.to_string()))?;

        task.metadata = merge_json(task.metadata.clone(), patch);
        task.updated_at = chrono::Utc::now();

        Ok(task.clone())
    }

    /// Get the next task to process (highest priority, FIFO within priority)
    pub async fn get_next(&self) -> Option<Task> {
        let mut pq = self.priority_queue.write().await;

        for (priority, queue) in pq.iter_mut().rev() {
            if let Some(task_id) = queue.pop_front() {
                debug!("Popped task {} from priority {}", task_id, priority);
                let tasks = self.tasks.read().await;
                return tasks.get(&task_id).cloned();
            }
        }

        None
    }

    /// Update a task's status
    pub async fn update_status(
        &self,
        id: &str,
        new_status: TaskStatus,
    ) -> Result<Task, QueueError> {
        let mut tasks = self.tasks.write().await;

        let task = tasks
            .get_mut(id)
            .ok_or_else(|| QueueError::NotFound(id.to_string()))?;
        let old_status = task.status;
        task.set_status(new_status);

        info!(
            "Updated task {} status: {} -> {}",
            id, old_status, new_status
        );

        let task_clone = task.clone();
        drop(tasks);

        self.update_priority_queue(id, old_status, new_status).await;

        let cb = self.on_status_changed.read().await;
        if let Some(callback) = cb.as_ref() {
            callback(&task_clone, new_status);
        }

        Ok(task_clone)
    }

    /// Remove a task from the queue
    pub async fn remove_task(&self, id: &str) -> Result<Task, QueueError> {
        let mut tasks = self.tasks.write().await;

        let task = tasks
            .remove(id)
            .ok_or_else(|| QueueError::NotFound(id.to_string()))?;
        let status = task.status;

        drop(tasks);

        if status == TaskStatus::Queued {
            let mut pq = self.priority_queue.write().await;
            if let Some(queue) = pq.get_mut(&task.priority) {
                queue.retain(|tid| tid != id);
            }
        }

        info!("Removed task {}", id);
        Ok(task)
    }

    async fn update_priority_queue(
        &self,
        id: &str,
        old_status: TaskStatus,
        new_status: TaskStatus,
    ) {
        if old_status == new_status {
            return;
        }

        let tasks = self.tasks.read().await;
        let task = match tasks.get(id) {
            Some(t) => t,
            None => return,
        };
        let priority = task.priority;
        drop(tasks);

        let mut pq = self.priority_queue.write().await;

        if old_status == TaskStatus::Queued {
            if let Some(queue) = pq.get_mut(&priority) {
                queue.retain(|tid| tid != id);
            }
        }

        if new_status == TaskStatus::Queued {
            pq.entry(priority)
                .or_insert_with(VecDeque::new)
                .push_back(id.to_string());
        }
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}
