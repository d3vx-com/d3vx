#[cfg(test)]
mod tests {
    use crate::pipeline::engine::*;
    use crate::pipeline::phases::{Phase, PhaseContext, Task, TaskStatus};
    use std::sync::Arc;
    // use std::collections::HashMap;

    fn create_test_task() -> Task {
        Task::new("TEST-001", "Test task", "Test instruction")
    }

    fn create_test_context(task: Task) -> PhaseContext {
        PhaseContext::new(task, "/project/root", "/project/worktree")
    }

    #[tokio::test]
    async fn test_engine_creation() {
        let engine = PipelineEngine::new();

        // Should have all default handlers
        for phase in Phase::all() {
            let handler = engine.get_handler(*phase).await;
            assert!(handler.is_some(), "Missing handler for phase: {}", phase);
        }
    }

    #[tokio::test]
    async fn test_engine_with_config() {
        let config = PipelineConfig {
            auto_commit: false,
            max_cost_usd: Some(10.0),
            timeout_minutes: 30,
            enable_checkpoints: false,
        };

        let engine = PipelineEngine::with_config(config.clone());
        assert_eq!(engine.config().auto_commit, false);
        assert_eq!(engine.config().max_cost_usd, Some(10.0));
        assert_eq!(engine.config().timeout_minutes, 30);
    }

    #[tokio::test]
    async fn test_run_single_phase() {
        let engine = PipelineEngine::new();
        let task = create_test_task();
        let context = create_test_context(task.clone());

        let result = engine.run_phases(task, context, &[Phase::Research]).await;

        assert!(result.is_ok());
        let run_result = result.unwrap();
        assert!(run_result.success);
        assert!(run_result.phase_results.contains_key(&Phase::Research));
    }

    #[tokio::test]
    async fn test_run_fails_terminal_task() {
        let engine = PipelineEngine::new();
        let task = create_test_task().with_status(TaskStatus::Completed);
        let context = create_test_context(task.clone());

        let result = engine.run(task, context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_phase_callbacks() {
        let engine = PipelineEngine::new();
        let callback_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count_clone = callback_count.clone();

        engine
            .on_phase_complete(Arc::new(move |_task, _phase, _result| {
                count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }))
            .await;

        let task = create_test_task();
        let context = create_test_context(task.clone());

        let _ = engine.run_phases(task, context, &[Phase::Research]).await;

        assert_eq!(callback_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_status_callbacks() {
        let engine = PipelineEngine::new();
        let status_changes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let changes_clone = status_changes.clone();

        engine
            .on_status_change(Arc::new(move |_task, status| {
                changes_clone.lock().unwrap().push(status);
            }))
            .await;

        let task = create_test_task();
        let context = create_test_context(task.clone());

        let _ = engine.run_phases(task, context, &[Phase::Research]).await;

        let changes = status_changes.lock().unwrap();
        assert!(changes.contains(&TaskStatus::InProgress));
        assert!(changes.contains(&TaskStatus::Completed));
    }

    #[tokio::test]
    async fn test_advance_phase() {
        let engine = PipelineEngine::new();
        let mut task = create_test_task().with_phase(Phase::Research);

        assert!(engine.advance_phase(&mut task));
        assert_eq!(task.phase, Phase::Ideation);

        assert!(engine.advance_phase(&mut task));
        assert_eq!(task.phase, Phase::Plan);
    }
}
