use crate::pipeline::docs_completeness::{DocsCompleteness, DocsStatus};
use crate::pipeline::qa_loop::*;
use crate::pipeline::review_gate::{BlockingReason, GateResult};
use crate::pipeline::review_summary::{
    FindingCategory, ReviewFinding, ReviewSeverity, ReviewSummary,
};
use crate::pipeline::validation_summary::ValidationSummary;

fn make_gate_result(blocked: bool, reasons: Vec<BlockingReason>) -> GateResult {
    GateResult {
        blocked,
        reasons,
        warnings: Vec::new(),
        ready: !blocked,
    }
}

fn make_review_summary() -> ReviewSummary {
    ReviewSummary::new("task-1".to_string())
}

#[test]
fn test_fix_resolution_status_summary() {
    let mut status = FixResolutionStatus::new();
    status.resolved_ids = vec!["f1".to_string(), "f2".to_string()];
    status.all_resolved = true;

    assert!(status.summary().contains("2 fix"));

    status.still_blocking_ids = vec!["f3".to_string()];
    status.all_resolved = false;

    let partial_summary = status.summary();
    assert!(partial_summary.contains("resolved"));
    assert!(partial_summary.contains("still blocking"));
}

#[test]
fn test_record_review_produces_merge_readiness() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    assert!(qa.last_merge_readiness.is_none());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);

    qa.record_review_result(&review, &gate);

    assert!(qa.last_merge_readiness.is_some());
    let readiness = qa.last_merge_readiness.as_ref().unwrap();
    assert!(readiness.ready);
}

#[test]
fn test_record_fix_updates_merge_readiness() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Bug found".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec![],
        }],
    );
    qa.record_review_result(&review, &gate);

    let first_readiness = qa.last_merge_readiness.clone().unwrap();
    assert!(!first_readiness.ready);

    qa.start_fix();
    qa.record_fix_result(None, None);

    let updated_readiness = qa.last_merge_readiness.as_ref().unwrap();
    assert!(!updated_readiness.ready);
}

#[test]
fn test_get_merge_readiness_returns_stored() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);
    qa.record_review_result(&review, &gate);

    let stored = qa.get_merge_readiness();
    assert!(stored.ready);
    assert_eq!(stored.reasons.len(), 0);
}

#[test]
fn test_to_metadata_includes_merge_readiness() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);
    qa.record_review_result(&review, &gate);

    let metadata = qa.to_metadata();

    assert!(metadata.get("merge_readiness").is_some());
    let readiness = metadata.get("merge_readiness").unwrap();
    assert!(readiness.get("ready").unwrap().as_bool().unwrap());
}

#[test]
fn test_qa_loop_persists_merge_readiness() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);
    qa.record_review_result(&review, &gate);

    let metadata = qa.to_metadata();
    let serialized = serde_json::to_string(&metadata).unwrap();

    let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert!(deserialized.get("merge_readiness").is_some());
}

#[test]
fn test_blocked_review_enters_fix_loop() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let mut review = make_review_summary();
    review.add_finding(ReviewFinding {
        id: "sec-1".to_string(),
        category: FindingCategory::Security,
        severity: ReviewSeverity::Critical,
        title: "SQL Injection".to_string(),
        description: "User input not sanitized".to_string(),
        location: None,
        suggestion: Some("Use parameterized queries".to_string()),
        resolved: false,
    });
    review.finalize();

    let gate = make_gate_result(
        true,
        vec![BlockingReason {
            code: "SECURITY".to_string(),
            message: "Critical security issue".to_string(),
            category: FindingCategory::Security,
            finding_ids: vec!["sec-1".to_string()],
        }],
    );

    qa.record_review_result(&review, &gate);
    let transition = qa.check_and_transition(&gate);

    assert!(matches!(transition, QATransition::NeedsFix { .. }));
    assert_eq!(qa.state, QAState::AwaitingFix);
    assert_eq!(qa.pending_findings.len(), 1);
    assert_eq!(qa.pending_findings[0].id, "sec-1");
    assert_eq!(qa.current_iteration, 1);
}

#[test]
fn test_successful_fix_clears_blockers() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let mut review1 = make_review_summary();
    review1.add_finding(ReviewFinding {
        id: "bug-1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Null pointer".to_string(),
        description: "Potential null pointer".to_string(),
        location: None,
        suggestion: Some("Add null check".to_string()),
        resolved: false,
    });
    review1.finalize();

    let gate1 = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Bug found".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["bug-1".to_string()],
        }],
    );

    qa.record_review_result(&review1, &gate1);
    qa.check_and_transition(&gate1);

    assert_eq!(qa.pending_findings.len(), 1);

    qa.start_fix();
    qa.record_fix_result(None, None);

    qa.start_rereview();
    let mut review2 = make_review_summary();
    review2.add_finding(ReviewFinding {
        id: "bug-1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Null pointer fixed".to_string(),
        description: "Now safe".to_string(),
        location: None,
        suggestion: None,
        resolved: true,
    });
    review2.finalize();

    let gate2 = make_gate_result(false, vec![]);
    let transition = qa.handle_rereview_result(&review2, &gate2);

    assert!(matches!(transition, QATransition::Approved));
    assert_eq!(qa.state, QAState::Approved);
    assert!(qa.pending_findings.is_empty());
}

#[test]
fn test_unresolved_blockers_keep_merge_blocked() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let mut review1 = make_review_summary();
    review1.add_finding(ReviewFinding {
        id: "bug-1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Bug still present".to_string(),
        description: "Not fixed".to_string(),
        location: None,
        suggestion: Some("Fix it properly".to_string()),
        resolved: false,
    });
    review1.finalize();

    let gate1 = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Bug not fixed".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["bug-1".to_string()],
        }],
    );

    qa.record_review_result(&review1, &gate1);
    qa.check_and_transition(&gate1);

    assert_eq!(qa.pending_findings.len(), 1);

    qa.start_fix();
    qa.record_fix_result(None, None);

    qa.start_rereview();
    let mut review2 = make_review_summary();
    review2.add_finding(ReviewFinding {
        id: "bug-1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Bug still present".to_string(),
        description: "Not fixed".to_string(),
        location: None,
        suggestion: Some("Fix it properly".to_string()),
        resolved: false,
    });
    review2.finalize();

    let gate2 = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Bug not fixed".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["bug-1".to_string()],
        }],
    );
    let transition = qa.handle_rereview_result(&review2, &gate2);

    assert!(matches!(transition, QATransition::NeedsFix { .. }));
    assert_eq!(qa.state, QAState::AwaitingFix);
    assert_eq!(qa.pending_findings.len(), 1);
}

#[test]
fn test_repeated_failures_escalate() {
    let mut qa = QALoop::new(
        "task-1".to_string(),
        QAConfig {
            max_retries: 3,
            require_validation: false,
            auto_fix_on_nonblocking: false,
        },
    );

    let gate_blocked = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Bug persists".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["bug-1".to_string()],
        }],
    );

    for iteration in 1..=3 {
        qa.start_review();
        let mut review = make_review_summary();
        review.add_finding(ReviewFinding {
            id: "bug-1".to_string(),
            category: FindingCategory::Correctness,
            severity: ReviewSeverity::High,
            title: "Bug".to_string(),
            description: "Not fixed".to_string(),
            location: None,
            suggestion: None,
            resolved: false,
        });
        review.finalize();

        qa.record_review_result(&review, &gate_blocked);
        let _transition = qa.check_and_transition(&gate_blocked);

        assert_eq!(qa.current_iteration, iteration as u32);

        if iteration < 3 {
            assert_eq!(qa.state, QAState::AwaitingFix);
            qa.start_fix();
            qa.record_fix_result(None, None);
        }
    }

    assert!(qa.should_escalate());
    assert_eq!(qa.state, QAState::Escalated);
    assert!(qa.escalation_reason.is_some());

    let status = qa.current_status();
    assert!(status.needs_escalation);
}

#[test]
fn test_fix_resolution_tracked_in_history() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let mut review1 = make_review_summary();
    review1.add_finding(ReviewFinding {
        id: "fix-1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Issue".to_string(),
        description: "Issue".to_string(),
        location: None,
        suggestion: None,
        resolved: false,
    });
    review1.finalize();
    let gate1 = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Bug".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["fix-1".to_string()],
        }],
    );
    qa.record_review_result(&review1, &gate1);
    qa.check_and_transition(&gate1);

    qa.start_fix();
    qa.record_fix_result(None, None);

    qa.start_rereview();
    let mut review2 = make_review_summary();
    review2.add_finding(ReviewFinding {
        id: "fix-1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Fixed".to_string(),
        description: "Fixed".to_string(),
        location: None,
        suggestion: None,
        resolved: true,
    });
    review2.finalize();
    let gate2 = make_gate_result(false, vec![]);
    qa.handle_rereview_result(&review2, &gate2);

    let records_with_resolution: Vec<_> = qa
        .history
        .iter()
        .filter(|r| !r.resolved_finding_ids.is_empty() || !r.still_blocking_ids.is_empty())
        .collect();
    assert!(!records_with_resolution.is_empty());
}

#[test]
fn test_validation_blocking_participates_in_qa() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);
    qa.record_review_result(&review, &gate);

    qa.start_fix();
    let mut validation = ValidationSummary::new(Some("task-1".to_string()));
    validation.confidence = crate::pipeline::validation_summary::Confidence::Low;
    validation.total = 5;
    validation.passed = 2;
    validation.failed = 3;
    qa.record_fix_result(Some(&validation), None);

    let readiness = qa.evaluate_merge_readiness();
    assert!(!readiness.ready);
    assert!(readiness.reasons.iter().any(|r| matches!(
        r.source,
        crate::pipeline::merge_gate::MergeSource::Validation
    )));
}

#[test]
fn test_qa_status_display_summary() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    let status = qa.current_status();
    assert!(status.display_summary().contains("pending"));

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(true, vec![]);
    qa.record_review_result(&review, &gate);
    qa.check_and_transition(&gate);

    let blocked_status = qa.current_status();
    assert!(
        blocked_status.display_summary().contains("AwaitingFix")
            || blocked_status.display_summary().contains("fix")
    );
}

#[test]
fn test_handle_rereview_result_records_resolution() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let mut review1 = make_review_summary();
    review1.add_finding(ReviewFinding {
        id: "partial-fix".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Issue".to_string(),
        description: "Issue".to_string(),
        location: None,
        suggestion: None,
        resolved: false,
    });
    review1.finalize();
    let gate1 = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Bug".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["partial-fix".to_string()],
        }],
    );
    qa.record_review_result(&review1, &gate1);
    qa.check_and_transition(&gate1);

    qa.start_fix();
    qa.record_fix_result(None, None);

    qa.start_rereview();
    let mut review2 = make_review_summary();
    review2.add_finding(ReviewFinding {
        id: "partial-fix".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Partially fixed".to_string(),
        description: "Partially fixed".to_string(),
        location: None,
        suggestion: Some("Complete the fix".to_string()),
        resolved: false,
    });
    review2.finalize();
    let gate2 = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Still broken".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["partial-fix".to_string()],
        }],
    );

    let transition = qa.handle_rereview_result(&review2, &gate2);

    assert!(matches!(transition, QATransition::NeedsFix { .. }));
    let last_record = qa.history.last().unwrap();
    assert!(!last_record.still_blocking_ids.is_empty());
}

#[test]
fn test_clear_pending_finding() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.add_pending_finding(PendingFinding {
        id: "to-clear".to_string(),
        title: "Issue".to_string(),
        category: "Correctness".to_string(),
        severity: "High".to_string(),
        suggestion: None,
        created_at_iteration: 1,
    });
    qa.add_pending_finding(PendingFinding {
        id: "to-keep".to_string(),
        title: "Issue 2".to_string(),
        category: "Correctness".to_string(),
        severity: "High".to_string(),
        suggestion: None,
        created_at_iteration: 1,
    });

    assert_eq!(qa.pending_findings.len(), 2);

    qa.clear_pending_finding("to-clear");

    assert_eq!(qa.pending_findings.len(), 1);
    assert_eq!(qa.pending_findings[0].id, "to-keep");
}

#[test]
fn test_full_qa_cycle_with_multiple_findings() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let mut review1 = make_review_summary();
    review1.add_finding(ReviewFinding {
        id: "issue-a".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Issue A".to_string(),
        description: "Issue A".to_string(),
        location: None,
        suggestion: Some("Fix A".to_string()),
        resolved: false,
    });
    review1.add_finding(ReviewFinding {
        id: "issue-b".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::Medium,
        title: "Issue B".to_string(),
        description: "Issue B".to_string(),
        location: None,
        suggestion: Some("Fix B".to_string()),
        resolved: false,
    });
    review1.finalize();

    let gate1 = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Two issues".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["issue-a".to_string(), "issue-b".to_string()],
        }],
    );

    qa.record_review_result(&review1, &gate1);
    qa.check_and_transition(&gate1);

    assert_eq!(qa.pending_findings.len(), 2);

    qa.start_fix();
    qa.record_fix_result(None, None);

    qa.start_rereview();
    let mut review2 = make_review_summary();
    review2.add_finding(ReviewFinding {
        id: "issue-a".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Issue A fixed".to_string(),
        description: "Fixed".to_string(),
        location: None,
        suggestion: None,
        resolved: true,
    });
    review2.add_finding(ReviewFinding {
        id: "issue-b".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::Medium,
        title: "Issue B still present".to_string(),
        description: "Not fixed".to_string(),
        location: None,
        suggestion: Some("Fix B".to_string()),
        resolved: false,
    });
    review2.finalize();

    let gate2 = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Issue B not fixed".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["issue-b".to_string()],
        }],
    );

    let transition = qa.handle_rereview_result(&review2, &gate2);

    assert!(matches!(transition, QATransition::NeedsFix { .. }));
    assert_eq!(qa.pending_findings.len(), 1);
    assert_eq!(qa.pending_findings[0].id, "issue-b");

    qa.start_fix();
    qa.record_fix_result(None, None);

    qa.start_rereview();
    let mut review3 = make_review_summary();
    review3.add_finding(ReviewFinding {
        id: "issue-a".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Issue A fixed".to_string(),
        description: "Fixed".to_string(),
        location: None,
        suggestion: None,
        resolved: true,
    });
    review3.add_finding(ReviewFinding {
        id: "issue-b".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::Medium,
        title: "Issue B fixed".to_string(),
        description: "Fixed".to_string(),
        location: None,
        suggestion: None,
        resolved: true,
    });
    review3.finalize();

    let gate3 = make_gate_result(false, vec![]);
    let final_transition = qa.handle_rereview_result(&review3, &gate3);

    assert!(matches!(final_transition, QATransition::Approved));
    assert!(qa.pending_findings.is_empty());
    assert_eq!(qa.current_iteration, 1);
}

#[test]
fn test_record_docs_result_updates_merge_readiness() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);
    qa.record_review_result(&review, &gate);

    let docs_before = DocsCompleteness::not_evaluated(Some("task-1".to_string()));
    qa.record_docs_result(&docs_before);

    assert!(qa.last_merge_readiness.is_some());
    assert!(qa.last_docs.is_some());
}

#[test]
fn test_record_docs_result_updates_stale_merge_readiness() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Bug found".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec![],
        }],
    );
    qa.record_review_result(&review, &gate);

    let readiness_before = qa.last_merge_readiness.clone().unwrap();
    assert!(
        !readiness_before.ready,
        "Should be blocked by review findings"
    );

    let docs = DocsCompleteness {
        task_id: Some("task-1".to_string()),
        status: DocsStatus::Complete,
        signals: vec![],
        docs_required: true,
        satisfied: true,
        missing_types: vec![],
        changed_files: vec!["README.md".to_string(), "api.md".to_string()],
        evaluated_at: None,
    };
    qa.record_docs_result(&docs);

    let readiness_after = qa.last_merge_readiness.clone().unwrap();
    assert!(!readiness_after.ready, "Still blocked by review findings");
}

#[test]
fn test_record_docs_result_evaluates_canonically() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);
    qa.record_review_result(&review, &gate);

    let docs = DocsCompleteness::not_evaluated(Some("task-1".to_string()));
    qa.record_docs_result(&docs);

    let from_last = qa.last_merge_readiness.clone().unwrap();
    let from_evaluate = qa.evaluate_merge_readiness();

    assert_eq!(from_last.ready, from_evaluate.ready);
    assert_eq!(from_last.reasons.len(), from_evaluate.reasons.len());
}

#[test]
fn test_to_metadata_after_docs_agrees_with_top_level() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);
    qa.record_review_result(&review, &gate);

    let docs = DocsCompleteness {
        task_id: Some("task-1".to_string()),
        status: DocsStatus::Complete,
        signals: vec![],
        docs_required: true,
        satisfied: true,
        missing_types: vec![],
        changed_files: vec!["README.md".to_string()],
        evaluated_at: None,
    };
    qa.record_docs_result(&docs);

    let metadata = qa.to_metadata();
    let embedded_readiness = metadata.get("merge_readiness").unwrap();
    let top_level_readiness = qa.evaluate_merge_readiness();
    let top_level_json = serde_json::to_value(&top_level_readiness).unwrap();

    assert_eq!(
        embedded_readiness.get("ready").unwrap(),
        top_level_json.get("ready").unwrap(),
        "Embedded merge_readiness in qa_loop should match top-level"
    );
}

#[test]
fn test_qa_loop_roundtrip_after_docs_updates_merge_readiness() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);
    qa.record_review_result(&review, &gate);

    let docs = DocsCompleteness {
        task_id: Some("task-1".to_string()),
        status: DocsStatus::Complete,
        signals: vec![],
        docs_required: true,
        satisfied: true,
        missing_types: vec![],
        changed_files: vec!["README.md".to_string()],
        evaluated_at: None,
    };
    qa.record_docs_result(&docs);

    let metadata = qa.to_metadata();
    let wrapper = serde_json::json!({ "qa_loop": metadata });
    let serialized = serde_json::to_string(&wrapper).unwrap();
    let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();

    let restored = QALoop::from_metadata("task-1".to_string(), &deserialized).unwrap();
    let restored_readiness = restored.get_merge_readiness();
    let original_readiness = qa.get_merge_readiness();

    assert_eq!(
        restored_readiness.ready, original_readiness.ready,
        "Restored qa_loop should have same merge readiness"
    );
}

#[test]
fn test_docs_result_uses_all_phase_states() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let mut review = make_review_summary();
    review.add_finding(ReviewFinding {
        id: "f1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Bug".to_string(),
        description: "Bug desc".to_string(),
        location: None,
        suggestion: Some("Fix it".to_string()),
        resolved: false,
    });
    review.finalize();
    let gate = make_gate_result(false, vec![]);
    qa.record_review_result(&review, &gate);

    let mut validation = ValidationSummary::new(Some("task-1".to_string()));
    validation.passed = 5;
    validation.total = 10;
    validation.failed = 5;
    qa.update_from_validation(&validation);

    let docs = DocsCompleteness {
        task_id: Some("task-1".to_string()),
        status: DocsStatus::Complete,
        signals: vec![],
        docs_required: true,
        satisfied: true,
        missing_types: vec![],
        changed_files: vec!["README.md".to_string()],
        evaluated_at: None,
    };
    qa.record_docs_result(&docs);

    let readiness = qa.get_merge_readiness();
    assert!(qa.last_review.is_some());
    assert!(qa.last_validation.is_some());
    assert!(qa.last_docs.is_some());
    assert!(qa.last_merge_readiness.is_some());
    assert!(readiness.signals.validation.is_some());
}
