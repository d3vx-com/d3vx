#[cfg(test)]
mod tests {

    use crate::pipeline::checkpoint::CheckpointManager;
    use crate::pipeline::docs_completeness::{DocType, DocsCompleteness, DocsStatus};
    use crate::pipeline::engine::PipelineEngine;
    use crate::pipeline::merge_gate::MergeGate;
    use crate::pipeline::phases::{Phase, PhaseContext, Priority, Task, TaskStatus};
    use crate::pipeline::qa_loop::{QAConfig, QALoop};
    use crate::pipeline::queue::TaskQueue;
    use crate::pipeline::review_gate::GateResult;
    use crate::pipeline::review_summary::{
        FindingCategory, ReviewFinding, ReviewSeverity, ReviewStatus, ReviewSummary,
    };
    use crate::pipeline::trust_parser::UnifiedTrustData;
    use crate::pipeline::validation_summary::{Confidence, ValidationSummary};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_full_pipeline_flow() {
        // Create engine and queue
        let engine = PipelineEngine::new();
        let queue = TaskQueue::new();

        // Add a task to the queue
        let task = Task::new(
            "INTEGRATION-001",
            "Integration test task",
            "Test the full pipeline",
        )
        .with_status(TaskStatus::Queued)
        .with_priority(Priority::High);

        queue.add_task(task.clone()).await.unwrap();

        // Get the next task
        let next_task = queue.get_next().await.unwrap();
        assert_eq!(next_task.id, "INTEGRATION-001");

        // Create context
        let context = PhaseContext::new(next_task.clone(), "/project", "/worktree");

        // Run a single phase
        let result = engine
            .run_phases(next_task.clone(), context.clone(), &[Phase::Research])
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.phase_results.contains_key(&Phase::Research));

        // Update task status in queue
        queue
            .update_status("INTEGRATION-001", TaskStatus::Completed)
            .await
            .unwrap();

        // Verify stats
        let stats = queue.stats().await;
        assert_eq!(stats.completed, 1);
    }

    #[tokio::test]
    async fn test_priority_queue_ordering() {
        let queue = TaskQueue::new();

        // Add tasks with different priorities
        queue
            .add_task(
                Task::new("LOW-001", "Low priority", "Test")
                    .with_status(TaskStatus::Queued)
                    .with_priority(Priority::Low),
            )
            .await
            .unwrap();

        queue
            .add_task(
                Task::new("CRITICAL-001", "Critical priority", "Test")
                    .with_status(TaskStatus::Queued)
                    .with_priority(Priority::Critical),
            )
            .await
            .unwrap();

        queue
            .add_task(
                Task::new("HIGH-001", "High priority", "Test")
                    .with_status(TaskStatus::Queued)
                    .with_priority(Priority::High),
            )
            .await
            .unwrap();

        // Should get critical first, then high, then low
        let first = queue.get_next().await.unwrap();
        assert_eq!(first.id, "CRITICAL-001");

        let second = queue.get_next().await.unwrap();
        assert_eq!(second.id, "HIGH-001");

        let third = queue.get_next().await.unwrap();
        assert_eq!(third.id, "LOW-001");

        // Queue should be empty
        assert!(queue.get_next().await.is_none());
    }

    #[tokio::test]
    async fn test_phase_progression() {
        let engine = PipelineEngine::new();
        let task = Task::new("PHASE-001", "Phase test", "Test phase progression")
            .with_phase(Phase::Research);

        let context = PhaseContext::new(task.clone(), "/project", "/worktree");

        // Run through all phases
        let result = engine.run(task, context).await.unwrap();

        assert!(result.success);
        assert!(result.task.phase.is_final());
        assert_eq!(result.task.status, TaskStatus::Completed);

        // All phases should have results
        assert!(result.phase_results.contains_key(&Phase::Research));
        assert!(result.phase_results.contains_key(&Phase::Plan));
        assert!(result.phase_results.contains_key(&Phase::Implement));
        assert!(result.phase_results.contains_key(&Phase::Review));
        assert!(result.phase_results.contains_key(&Phase::Docs));
    }

    #[tokio::test]
    async fn test_task_serialization() {
        let task = Task::new("SER-001", "Serialization test", "Test JSON serialization")
            .with_phase(Phase::Implement)
            .with_status(TaskStatus::InProgress)
            .with_priority(Priority::High)
            .with_worktree("/path/to/worktree");

        // Serialize to JSON
        let json = serde_json::to_string(&task).unwrap();

        // Deserialize back
        let deserialized: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, task.id);
        assert_eq!(deserialized.title, task.title);
        assert_eq!(deserialized.phase, task.phase);
        assert_eq!(deserialized.status, task.status);
        assert_eq!(deserialized.priority, task.priority);
        assert_eq!(deserialized.worktree_path, task.worktree_path);
    }

    #[tokio::test]
    async fn test_callback_notifications() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let engine = PipelineEngine::new();
        let phase_count = Arc::new(AtomicUsize::new(0));
        let status_count = Arc::new(AtomicUsize::new(0));

        let pc = phase_count.clone();
        engine
            .on_phase_complete(Arc::new(move |_task, _phase, _result| {
                pc.fetch_add(1, Ordering::SeqCst);
            }))
            .await;

        let sc = status_count.clone();
        engine
            .on_status_change(Arc::new(move |_task, _status| {
                sc.fetch_add(1, Ordering::SeqCst);
            }))
            .await;

        let task = Task::new("CALLBACK-001", "Callback test", "Test callbacks");
        let context = PhaseContext::new(task.clone(), "/project", "/worktree");

        let _ = engine.run(task, context).await.unwrap();

        // Should have 7 phase completions (Research, Ideation, Plan, Draft, Review, Implement, Docs)
        assert_eq!(phase_count.load(Ordering::SeqCst), 7);

        // Should have 2 status changes (InProgress, Completed)
        assert_eq!(status_count.load(Ordering::SeqCst), 2);
    }

    // =========================================================================
    // Core Production Path Integration Tests
    // =========================================================================

    #[test]
    fn test_qa_loop_to_merge_gate_integration() {
        let mut qa = QALoop::new("task-qa-001".to_string(), QAConfig::default());

        qa.start_review();
        let mut review = ReviewSummary::new("task-qa-001".to_string());
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug found".to_string(),
            description: "Bug description".to_string(),
            location: None,
            suggestion: Some("Fix it".to_string()),
            resolved: false,
        });
        review.finalize();

        let blocking_reason = crate::pipeline::review_gate::BlockingReason {
            code: "BUG_FOUND".to_string(),
            message: "Bug found".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["f1".to_string()],
        };
        let gate_result = GateResult {
            blocked: true,
            reasons: vec![blocking_reason],
            warnings: vec![],
            ready: false,
        };

        qa.record_review_result(&review, &gate_result);
        let transition = qa.check_and_transition(&gate_result);

        assert!(matches!(
            transition,
            crate::pipeline::qa_loop::QATransition::NeedsFix { .. }
        ));
        assert_eq!(qa.state, crate::pipeline::qa_loop::QAState::AwaitingFix);

        qa.start_fix();
        qa.record_fix_result(None, None);

        qa.start_rereview();
        let mut fixed_review = ReviewSummary::new("task-qa-001".to_string());
        fixed_review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug fixed".to_string(),
            description: "Bug now fixed".to_string(),
            location: None,
            suggestion: None,
            resolved: true,
        });
        fixed_review.finalize();

        let clean_gate = GateResult {
            blocked: false,
            reasons: vec![],
            warnings: vec![],
            ready: true,
        };

        qa.record_review_result(&fixed_review, &clean_gate);
        let final_transition = qa.check_and_transition(&clean_gate);

        assert!(matches!(
            final_transition,
            crate::pipeline::qa_loop::QATransition::Approved
        ));
        assert!(qa.current_status().is_merge_ready);
    }

    #[test]
    fn test_merge_gate_consumes_qa_state() {
        let gate = MergeGate::with_defaults();

        let review = ReviewSummary {
            task_id: "task-merge-001".to_string(),
            status: ReviewStatus::Approved,
            findings: vec![],
            changed_files: vec![],
            reviewer: crate::pipeline::review_summary::ReviewerType::Automated,
            requested_at: None,
            completed_at: None,
            summary_text: None,
            merge_blocked: false,
            blocking_findings: vec![],
        };

        let mut validation = ValidationSummary::new(Some("task-merge-001".to_string()));
        validation.confidence = crate::pipeline::validation_summary::Confidence::High;
        validation.passed = 3;
        validation.total = 3;
        let docs = DocsCompleteness {
            task_id: Some("task-merge-001".to_string()),
            status: DocsStatus::Complete,
            signals: vec![],
            docs_required: true,
            satisfied: true,
            missing_types: vec![],
            changed_files: vec![],
            evaluated_at: None,
        };

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(
            readiness.ready,
            "All phases passing should result in merge ready"
        );
        assert!(!readiness.blocked);
        assert!(readiness.reasons.is_empty());
        assert!(readiness.signals.review.is_some());
        assert!(readiness.signals.validation.is_some());
        assert!(readiness.signals.docs.is_some());
    }

    #[test]
    fn test_trust_parser_consumes_canonical_merge_readiness() {
        let full_metadata = serde_json::json!({
            "merge_readiness": {
                "ready": true,
                "blocked": false,
                "reasons": [],
                "warnings": [],
                "summary": "Ready to merge",
                "signals": {
                    "review": {"ready": true, "status": "approved"},
                    "validation": {"ready": true, "status": "High", "details": "3/3 passed"},
                    "docs": {"ready": true, "status": "Complete"}
                }
            },
            "review_summary": {
                "merge_blocked": false,
                "status": "Approved",
                "blocking_findings": []
            },
            "validation_summary": {
                "confidence": "High",
                "total": 3,
                "passed": 3,
                "failed": 0,
                "warnings": 0
            },
            "docs_completeness": {
                "status": "Complete",
                "docs_required": true,
                "satisfied": true,
                "signals": []
            },
            "qa_status": {
                "state": "approved",
                "iteration": 1,
                "max_retries": 3,
                "pending_fixes": 0,
                "is_merge_ready": true,
                "needs_escalation": false
            }
        });

        let trust = UnifiedTrustData::from_metadata(&full_metadata);

        assert!(
            trust.is_merge_ready(),
            "Trust parser should report merge ready"
        );
        assert_eq!(trust.blocking_count(), 0, "No blockers when merge ready");
        assert_eq!(trust.qa_iteration(), 1);
        assert!(!trust.needs_escalation());
    }

    #[tokio::test]
    async fn test_task_metadata_survives_checkpoint_restore() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path());
        manager.initialize().await.unwrap();

        let mut task = Task::new("CHECKPOINT-001", "Checkpoint test", "Test persistence")
            .with_status(TaskStatus::InProgress)
            .with_phase(Phase::Implement);

        let mut checkpoint = manager.create_checkpoint(task.clone()).await.unwrap();

        task.set_status(TaskStatus::Completed);
        task.phase = Phase::Docs;

        // Mutate the checkpoint to reflect the updated phase before persisting
        checkpoint.task.set_status(TaskStatus::InProgress);
        checkpoint.task.phase = Phase::Docs;
        manager.update_checkpoint(&checkpoint).await.unwrap();

        let restored = manager.load_checkpoint("CHECKPOINT-001").await.unwrap();

        assert!(
            restored.is_some(),
            "Checkpoint should be restorable after update"
        );
        let loaded = restored.unwrap();
        assert_eq!(loaded.task.status, TaskStatus::InProgress, "Check if task status is correctly restored as InProgress");
        assert_eq!(loaded.task.phase, Phase::Docs);
    }

    #[test]
    fn test_trust_parser_falls_back_to_review_summary() {
        let legacy_metadata = serde_json::json!({
            "review_summary": {
                "merge_blocked": false,
                "status": "approved",
                "blocking_findings": []
            }
        });

        let trust = UnifiedTrustData::from_metadata(&legacy_metadata);

        assert!(
            trust.is_merge_ready(),
            "Should fall back to review_summary for readiness"
        );
        assert!(trust.merge_readiness.is_none(), "No modern merge_readiness");
        assert!(
            trust.review_summary.is_some(),
            "Should have legacy review_summary"
        );
    }

    #[test]
    fn test_multiple_findings_across_phases_affect_merge_readiness() {
        let gate = MergeGate::with_defaults();

        let mut review = ReviewSummary::new("multi-001".to_string());
        review.add_finding(ReviewFinding {
            id: "f1".to_string(),
            category: FindingCategory::Security,
            severity: ReviewSeverity::Critical,
            title: "Auth bypass".to_string(),
            description: "...".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });
        review.finalize();
        review.merge_blocked = true;
        review.blocking_findings = vec!["f1".to_string()];

        let validation = ValidationSummary::new(Some("multi-001".to_string()));

        let docs = DocsCompleteness {
            task_id: Some("multi-001".to_string()),
            status: DocsStatus::Missing,
            signals: vec![],
            docs_required: true,
            satisfied: false,
            missing_types: vec![DocType::ApiDocs],
            changed_files: vec![],
            evaluated_at: None,
        };

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(!readiness.ready, "Security + docs should block merge");
        assert!(
            readiness.has_blocking_review(),
            "Should have blocking review"
        );
        assert!(readiness.has_blocking_docs(), "Should have blocking docs");
        assert_eq!(readiness.reasons.len(), 3, "Ensure all reasons for blocking are captured");
    }

    #[test]
    fn test_validation_low_confidence_blocks_even_with_good_review() {
        let gate = MergeGate::with_defaults();

        let review = ReviewSummary {
            task_id: "val-fail-001".to_string(),
            status: ReviewStatus::Approved,
            findings: vec![],
            changed_files: vec![],
            reviewer: crate::pipeline::review_summary::ReviewerType::Automated,
            requested_at: None,
            completed_at: None,
            summary_text: None,
            merge_blocked: false,
            blocking_findings: vec![],
        };

        let mut validation = ValidationSummary::new(Some("val-fail-001".to_string()));
        validation.confidence = Confidence::Low;
        validation.passed = 1;
        validation.failed = 4;

        let docs = DocsCompleteness {
            task_id: Some("val-fail-001".to_string()),
            status: DocsStatus::Complete,
            signals: vec![],
            docs_required: true,
            satisfied: true,
            missing_types: vec![],
            changed_files: vec![],
            evaluated_at: None,
        };

        let readiness = gate.evaluate(Some(&review), Some(&validation), Some(&docs));

        assert!(!readiness.ready, "Low confidence validation should block");
        assert!(readiness.has_blocking_validation());
    }

    #[tokio::test]
    async fn test_checkpoint_preserves_qa_state_across_phases() {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path());
        manager.initialize().await.unwrap();

        let task =
            Task::new("QA-CHECK-001", "QA Checkpoint test", "Test").with_phase(Phase::Review);

        let mut checkpoint = manager.create_checkpoint(task).await.unwrap();

        checkpoint.task.set_status(TaskStatus::InProgress);
        manager.update_checkpoint(&checkpoint).await.unwrap();

        let restored = manager
            .load_checkpoint("QA-CHECK-001")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(restored.task.phase, Phase::Review);
        assert_eq!(restored.task.status, TaskStatus::InProgress);
    }

    #[test]
    fn test_qa_iteration_tracking_affects_trust() {
        let iter_1_metadata = serde_json::json!({
            "qa_status": {
                "state": "awaiting_fix",
                "iteration": 1,
                "max_retries": 3,
                "pending_fixes": 2,
                "is_merge_ready": false,
                "needs_escalation": false
            },
            "merge_readiness": {
                "ready": false,
                "blocked": true,
                "reasons": [{"code": "QA_PENDING", "message": "Fix in progress"}]
            }
        });

        let trust_1 = UnifiedTrustData::from_metadata(&iter_1_metadata);
        assert_eq!(trust_1.qa_iteration(), 1, "QA iteration should be parsed from metadata");
        assert!(!trust_1.needs_escalation());
        assert!(!trust_1.is_merge_ready());

        let escalated_metadata = serde_json::json!({
            "qa_status": {
                "state": "escalated",
                "iteration": 3,
                "max_retries": 3,
                "pending_fixes": 5,
                "is_merge_ready": false,
                "needs_escalation": true
            },
            "merge_readiness": {
                "ready": false,
                "blocked": true,
                "reasons": [{"code": "MAX_RETRIES", "message": "Max retries exceeded"}]
            }
        });

        let trust_escalated = UnifiedTrustData::from_metadata(&escalated_metadata);
        assert!(!trust_escalated.is_merge_ready());
        assert_eq!(trust_escalated.qa_iteration(), 3);
        assert!(
            trust_escalated.needs_escalation(),
            "Should flag escalation needed"
        );
    }
}
