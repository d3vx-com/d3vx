//! Tests for task queue

use std::sync::Arc;

use super::task_queue::TaskQueue;
use super::types::QueueError;
use crate::pipeline::phases::{Priority, Task, TaskStatus};

#[tokio::test]
async fn test_add_task() {
    let queue = TaskQueue::new();
    let task =
        Task::new("TEST-001", "Test task", "Test instruction").with_status(TaskStatus::Queued);

    let result = queue.add_task(task).await;
    assert!(result.is_ok());
    assert!(queue.contains("TEST-001").await);
}

#[tokio::test]
async fn test_add_duplicate_task() {
    let queue = TaskQueue::new();
    let task1 = Task::new("TEST-001", "Test task", "Test instruction");
    let task2 = Task::new("TEST-001", "Another task", "Another instruction");

    queue.add_task(task1).await.unwrap();
    let result = queue.add_task(task2).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_next_priority_ordering() {
    let queue = TaskQueue::new();

    let low_task = Task::new("LOW-001", "Low priority", "Test")
        .with_status(TaskStatus::Queued)
        .with_priority(Priority::Low);
    let high_task = Task::new("HIGH-001", "High priority", "Test")
        .with_status(TaskStatus::Queued)
        .with_priority(Priority::High);
    let critical_task = Task::new("CRITICAL-001", "Critical priority", "Test")
        .with_status(TaskStatus::Queued)
        .with_priority(Priority::Critical);

    queue.add_task(low_task).await.unwrap();
    queue.add_task(high_task).await.unwrap();
    queue.add_task(critical_task).await.unwrap();

    let next = queue.get_next().await.unwrap();
    assert_eq!(next.id, "CRITICAL-001");

    let next = queue.get_next().await.unwrap();
    assert_eq!(next.id, "HIGH-001");

    let next = queue.get_next().await.unwrap();
    assert_eq!(next.id, "LOW-001");

    assert!(queue.get_next().await.is_none());
}

#[tokio::test]
async fn test_get_next_fifo_within_priority() {
    let queue = TaskQueue::new();

    let task1 = Task::new("TASK-001", "First", "Test")
        .with_status(TaskStatus::Queued)
        .with_priority(Priority::High);
    let task2 = Task::new("TASK-002", "Second", "Test")
        .with_status(TaskStatus::Queued)
        .with_priority(Priority::High);

    queue.add_task(task1).await.unwrap();
    queue.add_task(task2).await.unwrap();

    let next = queue.get_next().await.unwrap();
    assert_eq!(next.id, "TASK-001");

    let next = queue.get_next().await.unwrap();
    assert_eq!(next.id, "TASK-002");
}

#[tokio::test]
async fn test_update_status() {
    let queue = TaskQueue::new();
    let task = Task::new("TEST-001", "Test", "Test").with_status(TaskStatus::Backlog);

    queue.add_task(task).await.unwrap();

    let updated = queue
        .update_status("TEST-001", TaskStatus::Queued)
        .await
        .unwrap();
    assert_eq!(updated.status, TaskStatus::Queued);

    let task = queue.get_task("TEST-001").await.unwrap();
    assert_eq!(task.status, TaskStatus::Queued);
}

#[tokio::test]
async fn test_update_status_not_found() {
    let queue = TaskQueue::new();
    let result = queue.update_status("NONEXISTENT", TaskStatus::Queued).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_remove_task() {
    let queue = TaskQueue::new();
    let task = Task::new("TEST-001", "Test", "Test");

    queue.add_task(task).await.unwrap();
    assert!(queue.contains("TEST-001").await);

    let removed = queue.remove_task("TEST-001").await.unwrap();
    assert_eq!(removed.id, "TEST-001");
    assert!(!queue.contains("TEST-001").await);
}

#[tokio::test]
async fn test_list_tasks() {
    let queue = TaskQueue::new();

    queue
        .add_task(Task::new("TASK-001", "1", "1"))
        .await
        .unwrap();
    queue
        .add_task(Task::new("TASK-002", "2", "2"))
        .await
        .unwrap();
    queue
        .add_task(Task::new("TASK-003", "3", "3"))
        .await
        .unwrap();

    let tasks = queue.list_tasks().await;
    assert_eq!(tasks.len(), 3);
}

#[tokio::test]
async fn test_list_by_status() {
    let queue = TaskQueue::new();

    queue
        .add_task(Task::new("TASK-001", "1", "1").with_status(TaskStatus::Queued))
        .await
        .unwrap();
    queue
        .add_task(Task::new("TASK-002", "2", "2").with_status(TaskStatus::Queued))
        .await
        .unwrap();
    queue
        .add_task(Task::new("TASK-003", "3", "3").with_status(TaskStatus::InProgress))
        .await
        .unwrap();

    let queued = queue.list_by_status(TaskStatus::Queued).await;
    assert_eq!(queued.len(), 2);

    let in_progress = queue.list_by_status(TaskStatus::InProgress).await;
    assert_eq!(in_progress.len(), 1);
}

#[tokio::test]
async fn test_stats() {
    let queue = TaskQueue::new();

    queue
        .add_task(Task::new("TASK-001", "1", "1").with_status(TaskStatus::Backlog))
        .await
        .unwrap();
    queue
        .add_task(Task::new("TASK-002", "2", "2").with_status(TaskStatus::Queued))
        .await
        .unwrap();
    queue
        .add_task(Task::new("TASK-003", "3", "3").with_status(TaskStatus::InProgress))
        .await
        .unwrap();
    queue
        .add_task(Task::new("TASK-004", "4", "4").with_status(TaskStatus::Completed))
        .await
        .unwrap();
    queue
        .add_task(Task::new("TASK-005", "5", "5").with_status(TaskStatus::Failed))
        .await
        .unwrap();

    let stats = queue.stats().await;
    assert_eq!(stats.total, 5);
    assert_eq!(stats.backlog, 1);
    assert_eq!(stats.queued, 1);
    assert_eq!(stats.in_progress, 1);
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.failed, 1);
}

#[tokio::test]
async fn test_reprioritize() {
    let queue = TaskQueue::new();
    let task = Task::new("TEST-001", "Test", "Test")
        .with_status(TaskStatus::Queued)
        .with_priority(Priority::Low);

    queue.add_task(task).await.unwrap();

    let updated = queue
        .reprioritize("TEST-001", Priority::Critical)
        .await
        .unwrap();
    assert_eq!(updated.priority, Priority::Critical);

    let next = queue.get_next().await.unwrap();
    assert_eq!(next.id, "TEST-001");
}

#[tokio::test]
async fn test_capacity_limit() {
    let queue = TaskQueue::with_capacity(2);

    queue
        .add_task(Task::new("TASK-001", "1", "1"))
        .await
        .unwrap();
    queue
        .add_task(Task::new("TASK-002", "2", "2"))
        .await
        .unwrap();

    let result = queue.add_task(Task::new("TASK-003", "3", "3")).await;
    assert!(matches!(result, Err(QueueError::AtCapacity(2))));
}

#[tokio::test]
async fn test_clear() {
    let queue = TaskQueue::new();

    queue
        .add_task(Task::new("TASK-001", "1", "1"))
        .await
        .unwrap();
    queue
        .add_task(Task::new("TASK-002", "2", "2"))
        .await
        .unwrap();

    queue.clear().await;

    assert!(queue.is_empty().await);
    assert_eq!(queue.len().await, 0);
}

#[tokio::test]
async fn test_callbacks() {
    let queue = TaskQueue::new();
    let added_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let status_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let added_clone = added_count.clone();
    queue
        .on_task_added(Arc::new(move |_task| {
            added_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }))
        .await;

    let status_clone = status_count.clone();
    queue
        .on_status_changed(Arc::new(move |_task, _status| {
            status_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }))
        .await;

    queue
        .add_task(Task::new("TASK-001", "1", "1"))
        .await
        .unwrap();
    queue
        .update_status("TASK-001", TaskStatus::Queued)
        .await
        .unwrap();

    assert_eq!(added_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(status_count.load(std::sync::atomic::Ordering::SeqCst), 1);
}
