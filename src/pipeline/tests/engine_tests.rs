#[cfg(test)]
mod tests {
    use crate::pipeline::engine::*;
    use crate::pipeline::handlers::{PhaseError, PhaseHandler, PhaseResult};
    use crate::pipeline::phases::{Phase, PhaseContext, Task, TaskStatus};
    use crate::pipeline::timeout::TimeoutConfig;
    use async_trait::async_trait;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::sleep;
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

    // ── per-phase timeout enforcement ─────────────────────────────

    /// A handler that sleeps longer than any reasonable test timeout. Used
    /// to prove the engine's timeout wrap actually fires — previously a
    /// hung handler would hold its worker lease indefinitely.
    struct HangingHandler {
        phase: Phase,
    }

    #[async_trait]
    impl PhaseHandler for HangingHandler {
        fn phase(&self) -> Phase {
            self.phase
        }
        async fn execute(
            &self,
            _task: &Task,
            _context: &PhaseContext,
            _agent: Option<Arc<crate::agent::AgentLoop>>,
        ) -> Result<PhaseResult, PhaseError> {
            // Sleep far longer than any test timeout. The timeout wrap
            // should abort this future, not let it complete.
            sleep(Duration::from_secs(60)).await;
            Ok(PhaseResult::success("should-not-reach"))
        }
    }

    fn short_timeout_config() -> TimeoutConfig {
        let mut cfg = TimeoutConfig::default();
        // Override every phase's timeout to 50ms.
        for phase in Phase::all() {
            cfg = cfg.with_phase_timeout(*phase, Duration::from_millis(50));
        }
        cfg.default_phase_timeout = Duration::from_millis(50);
        cfg.cleanup_grace_period = Duration::from_millis(10);
        cfg
    }

    #[tokio::test]
    async fn test_run_phases_times_out_on_hanging_handler() {
        let mut engine =
            PipelineEngine::new().with_timeout_config(short_timeout_config());
        engine.register_handler_sync(Box::new(HangingHandler {
            phase: Phase::Research,
        }));

        let task = create_test_task().with_phase(Phase::Research);
        let context = create_test_context(task.clone());

        let result = engine
            .run_phases(task, context, &[Phase::Research])
            .await;

        assert!(result.is_err(), "expected timeout error");
        assert!(
            matches!(result.unwrap_err(), PhaseError::Timeout { .. }),
            "expected PhaseError::Timeout, got different error variant"
        );
    }

    #[tokio::test]
    async fn test_run_times_out_and_marks_task_failed_without_retry() {
        let mut engine =
            PipelineEngine::new().with_timeout_config(short_timeout_config());
        engine.register_handler_sync(Box::new(HangingHandler {
            phase: Phase::Research,
        }));

        let task = create_test_task().with_phase(Phase::Research);
        let context = create_test_context(task.clone());

        // Record every status transition so we can verify no retry happened.
        let transitions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let transitions_clone = transitions.clone();
        engine
            .on_status_change(Arc::new(move |_task, status| {
                transitions_clone.lock().unwrap().push(status);
            }))
            .await;

        let err = engine.run(task, context).await.unwrap_err();
        assert!(
            matches!(err, PhaseError::Timeout { .. }),
            "expected PhaseError::Timeout"
        );

        let transitions = transitions.lock().unwrap();
        // Exactly: InProgress (initial) then Failed (after timeout).
        // No retry → no second InProgress.
        let in_progress_count = transitions
            .iter()
            .filter(|s| **s == TaskStatus::InProgress)
            .count();
        assert_eq!(
            in_progress_count, 1,
            "timeout must not trigger retry; saw {} InProgress transitions",
            in_progress_count
        );
        assert!(
            transitions.contains(&TaskStatus::Failed),
            "task must be marked Failed on timeout"
        );
    }

    #[tokio::test]
    async fn test_default_timeout_config_applied_when_builder_not_called() {
        // Default TimeoutConfig has 5-minute budgets per phase; a fast
        // handler should complete well within that. This guards against
        // the default being misconfigured to something restrictive.
        let engine = PipelineEngine::new();
        let task = create_test_task();
        let context = create_test_context(task.clone());

        let result = engine.run_phases(task, context, &[Phase::Research]).await;
        assert!(result.is_ok(), "default timeout must not fire on fast handlers");
    }
}
