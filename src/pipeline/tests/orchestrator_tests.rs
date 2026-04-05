#[cfg(test)]
mod tests {
    use crate::pipeline::orchestrator::*;
    use crate::pipeline::phases::{Phase, Priority, TaskStatus};
    use tempfile::TempDir;

    async fn create_test_orchestrator() -> (PipelineOrchestrator, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = OrchestratorConfig::default()
            .with_checkpoint_dir(temp_dir.path().join("checkpoints"))
            .without_auto_recovery();

        let orchestrator = PipelineOrchestrator::new(config, None).await.unwrap();
        (orchestrator, temp_dir)
    }

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let (orchestrator, _temp) = create_test_orchestrator().await;

        let stats = orchestrator.queue_stats().await;
        assert_eq!(stats.total, 0);
    }

    #[tokio::test]
    async fn test_create_task_from_chat() {
        let (orchestrator, _temp) = create_test_orchestrator().await;

        let task = orchestrator
            .create_task_from_chat("Test task", "Test instruction", None)
            .await
            .unwrap();

        assert!(task.id.starts_with("TASK"));
        assert_eq!(task.title, "Test task");

        let stats = orchestrator.queue_stats().await;
        assert_eq!(stats.total, 1);
    }

    #[tokio::test]
    async fn test_create_task_from_github_issue() {
        let (orchestrator, _temp) = create_test_orchestrator().await;

        let task = orchestrator
            .create_task_from_github_issue(
                42,
                "owner/repo",
                "author",
                "Bug: Something broke",
                "Details",
            )
            .await
            .unwrap();

        assert!(task.id.starts_with("TASK"));
        assert!(task.title.contains("42"));
    }

    #[tokio::test]
    async fn test_create_task_with_priority() {
        let (orchestrator, _temp) = create_test_orchestrator().await;

        let task = orchestrator
            .create_task_from_chat("Test", "Test", Some(Priority::Critical))
            .await
            .unwrap();

        assert_eq!(task.priority, Priority::Critical);
    }

    #[tokio::test]
    async fn test_transition_task() {
        let (orchestrator, _temp) = create_test_orchestrator().await;

        let task = orchestrator
            .create_task_from_chat("Test", "Test", None)
            .await
            .unwrap();

        let updated = orchestrator
            .transition_task(&task.id, TaskStatus::Queued)
            .await
            .unwrap();

        assert_eq!(updated.status, TaskStatus::Queued);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let (orchestrator, _temp) = create_test_orchestrator().await;

        let task = orchestrator
            .create_task_from_chat("Test", "Test", None)
            .await
            .unwrap();

        orchestrator.cancel_task(&task.id).await.unwrap();

        let stats = orchestrator.queue_stats().await;
        assert_eq!(stats.failed, 1);
    }

    #[tokio::test]
    async fn test_recover_interrupted_tasks_requeues_in_progress_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let config =
            OrchestratorConfig::default().with_checkpoint_dir(temp_dir.path().join("checkpoints"));
        let orchestrator = PipelineOrchestrator::new(config, None).await.unwrap();

        let task = orchestrator
            .create_task_from_chat("Recover me", "Test recovery", None)
            .await
            .unwrap();
        let mut checkpoint = orchestrator
            .checkpoint_manager()
            .load_checkpoint(&task.id)
            .await
            .unwrap()
            .unwrap();
        checkpoint.task.set_status(TaskStatus::InProgress);
        checkpoint.task.phase = Phase::Implement;
        orchestrator
            .checkpoint_manager()
            .update_checkpoint(&checkpoint)
            .await
            .unwrap();

        let recovered = orchestrator.recover_interrupted_tasks().await.unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].id, task.id);
        assert_eq!(recovered[0].status, TaskStatus::Queued);
    }

    #[tokio::test]
    async fn test_worker_pool_integration() {
        let (orchestrator, _temp) = create_test_orchestrator().await;

        let stats = orchestrator.worker_pool_stats().await;
        assert_eq!(stats.total_workers, 1); // Default worker
        assert_eq!(stats.available_workers, 1);
    }

    #[tokio::test]
    async fn test_classification_stored_in_metadata() {
        let (orchestrator, _temp) = create_test_orchestrator().await;

        let task = orchestrator
            .create_task_from_chat(
                "Complex task",
                "This is a complex task with multiple operations",
                None,
            )
            .await
            .unwrap();

        // Check that classification is in metadata
        if let serde_json::Value::Object(map) = &task.metadata {
            assert!(map.contains_key("classification"));
        }
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let (orchestrator, _temp) = create_test_orchestrator().await;

        let low = orchestrator
            .create_task_from_chat("Low", "Test", Some(Priority::Low))
            .await
            .unwrap();
        let _ = orchestrator
            .transition_task(&low.id, TaskStatus::Queued)
            .await;

        let high = orchestrator
            .create_task_from_chat("High", "Test", Some(Priority::High))
            .await
            .unwrap();
        let _ = orchestrator
            .transition_task(&high.id, TaskStatus::Queued)
            .await;

        // Should get high priority first
        let first = orchestrator.get_next_task().await.unwrap();
        assert_eq!(first.priority, Priority::High);

        let second = orchestrator.get_next_task().await.unwrap();
        assert_eq!(second.priority, Priority::Low);
    }

    #[test]
    fn test_config_builder() {
        let config = OrchestratorConfig::default()
            .with_max_concurrent_tasks(5)
            .without_auto_recovery()
            .with_task_id_prefix("ISSUE");

        assert_eq!(config.max_concurrent_tasks, 5);
        assert!(!config.enable_auto_recovery);
        assert_eq!(config.task_id_prefix, "ISSUE");
    }
}
