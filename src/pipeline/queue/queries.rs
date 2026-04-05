//! Task queue implementation - query and utility operations

use std::collections::VecDeque;
use tracing::info;

use super::task_queue::TaskQueue;
use super::types::QueueStats;
use crate::pipeline::phases::{Priority, TaskStatus};

impl TaskQueue {
    /// List all tasks
    pub async fn list_tasks(&self) -> Vec<crate::pipeline::phases::Task> {
        let tasks = self.tasks.read().await;
        tasks.values().cloned().collect()
    }

    /// List tasks by status
    pub async fn list_by_status(&self, status: TaskStatus) -> Vec<crate::pipeline::phases::Task> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|t| t.status == status)
            .cloned()
            .collect()
    }

    /// List tasks by priority
    pub async fn list_by_priority(&self, priority: Priority) -> Vec<crate::pipeline::phases::Task> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|t| t.priority == priority)
            .cloned()
            .collect()
    }

    /// Get queue statistics
    pub async fn stats(&self) -> QueueStats {
        let tasks = self.tasks.read().await;

        let mut stats = QueueStats {
            total: tasks.len(),
            ..Default::default()
        };

        for task in tasks.values() {
            match task.status {
                TaskStatus::Backlog => stats.backlog += 1,
                TaskStatus::Queued => stats.queued += 1,
                TaskStatus::InProgress => stats.in_progress += 1,
                TaskStatus::Completed => stats.completed += 1,
                TaskStatus::Failed => stats.failed += 1,
                TaskStatus::Cancelled => stats.cancelled += 1,
                TaskStatus::Unknown => stats.unknown += 1,
            }
        }

        stats
    }

    /// Check if the queue is empty
    pub async fn is_empty(&self) -> bool {
        let tasks = self.tasks.read().await;
        tasks.is_empty()
    }

    /// Get the number of tasks in the queue
    pub async fn len(&self) -> usize {
        let tasks = self.tasks.read().await;
        tasks.len()
    }

    /// Check if a task exists
    pub async fn contains(&self, id: &str) -> bool {
        let tasks = self.tasks.read().await;
        tasks.contains_key(id)
    }

    /// Clear all tasks from the queue
    pub async fn clear(&self) {
        let mut tasks = self.tasks.write().await;
        let mut pq = self.priority_queue.write().await;

        tasks.clear();
        pq.clear();

        info!("Cleared all tasks from queue");
    }

    /// Get the number of queued tasks
    pub async fn queued_count(&self) -> usize {
        let pq = self.priority_queue.read().await;
        pq.values().map(|q| q.len()).sum()
    }

    /// Re-prioritize a task
    pub async fn reprioritize(
        &self,
        id: &str,
        new_priority: Priority,
    ) -> Result<crate::pipeline::phases::Task, super::types::QueueError> {
        let mut tasks = self.tasks.write().await;

        let task = tasks
            .get_mut(id)
            .ok_or_else(|| super::types::QueueError::NotFound(id.to_string()))?;
        let old_priority = task.priority;
        let status = task.status;

        if old_priority == new_priority {
            return Ok(task.clone());
        }

        task.priority = new_priority;
        let task_clone = task.clone();
        drop(tasks);

        if status == TaskStatus::Queued {
            let mut pq = self.priority_queue.write().await;

            if let Some(queue) = pq.get_mut(&old_priority) {
                queue.retain(|tid| tid != id);
            }

            pq.entry(new_priority)
                .or_insert_with(VecDeque::new)
                .push_back(id.to_string());
        }

        info!(
            "Reprioritized task {}: {:?} -> {:?}",
            id, old_priority, new_priority
        );
        Ok(task_clone)
    }
}
