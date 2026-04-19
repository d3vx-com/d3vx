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
fn test_qa_loop_initial_state() {
    let qa = QALoop::new("task-1".to_string(), QAConfig::default());
    assert_eq!(qa.state, QAState::Pending);
    assert_eq!(qa.current_iteration, 0);
    assert!(qa.can_continue());
}

#[test]
fn test_review_transitions_to_approved() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());
    qa.start_review();

    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);

    qa.record_review_result(&review, &gate);
    let transition = qa.check_and_transition(&gate);

    assert!(matches!(transition, QATransition::Approved));
    assert_eq!(qa.state, QAState::Approved);
}

#[test]
fn test_review_triggers_fix_loop() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());
    qa.start_review();

    let review = make_review_summary();
    let gate = make_gate_result(
        true,
        vec![BlockingReason {
            code: "SECURITY_ISSUE".to_string(),
            message: "Security finding".to_string(),
            category: crate::pipeline::review_summary::FindingCategory::Security,
            finding_ids: vec!["f1".to_string()],
        }],
    );

    qa.record_review_result(&review, &gate);
    let transition = qa.check_and_transition(&gate);

    assert!(matches!(
        transition,
        QATransition::NeedsFix {
            reasons,
            iteration: 1
        } if reasons.len() == 1
    ));
    assert_eq!(qa.state, QAState::AwaitingFix);
}

#[test]
fn test_max_retries_escalates() {
    let mut qa = QALoop::new(
        "task-1".to_string(),
        QAConfig {
            max_retries: 2,
            require_validation: false,
            auto_fix_on_nonblocking: false,
        },
    );

    for _ in 0..2 {
        qa.start_review();
        let review = make_review_summary();
        let gate = make_gate_result(true, vec![]);
        qa.record_review_result(&review, &gate);
        qa.check_and_transition(&gate);
    }

    assert_eq!(qa.current_iteration, 2);
    assert!(qa.should_escalate());
    assert!(matches!(
        qa.state,
        QAState::AwaitingFix | QAState::Escalated
    ));
}

#[test]
fn test_fix_then_approve() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let review1 = make_review_summary();
    let gate1 = make_gate_result(true, vec![]);
    qa.record_review_result(&review1, &gate1);
    qa.check_and_transition(&gate1);

    qa.start_fix();
    qa.record_fix_result(None, None);

    qa.start_rereview();
    let review2 = make_review_summary();
    let gate2 = make_gate_result(false, vec![]);
    qa.record_review_result(&review2, &gate2);
    let transition = qa.check_and_transition(&gate2);

    assert!(matches!(transition, QATransition::Approved));
    assert_eq!(qa.history.len(), 3);
}

#[test]
fn test_history_records_duration() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());
    qa.start_review();

    let review = make_review_summary();
    let gate = make_gate_result(false, vec![]);
    qa.record_review_result(&review, &gate);

    let record = &qa.history[0];
    assert_eq!(record.iteration, 1);
    assert!(record.duration_ms.is_some());
    assert!(!record.blocked);
}

#[test]
fn test_metadata_roundtrip() {
    let qa = QALoop::new("task-1".to_string(), QAConfig::default());
    let inner = qa.to_metadata();
    let metadata = serde_json::json!({ "qa_loop": inner });

    let restored = QALoop::from_metadata("task-1".to_string(), &metadata).unwrap();
    assert_eq!(restored.task_id, qa.task_id);
    assert_eq!(restored.max_retries, qa.max_retries);
}

#[test]
fn test_status_display() {
    let qa = QALoop::new("task-1".to_string(), QAConfig::default());
    let status = qa.current_status();

    assert_eq!(status.state, QAState::Pending);
    assert!(!status.is_merge_ready);

    let display = status.display_summary();
    assert!(display.contains("pending"));
}

#[test]
fn test_blocking_issues_trigger_fix_loop() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();

    let mut review = make_review_summary();
    review.add_finding(ReviewFinding {
        id: "sec-1".to_string(),
        category: FindingCategory::Security,
        severity: ReviewSeverity::High,
        title: "SQL Injection vulnerability".to_string(),
        description: "User input not sanitized".to_string(),
        location: None,
        suggestion: Some("Use parameterized queries".to_string()),
        resolved: false,
    });
    review.finalize();

    let gate = GateResult {
        blocked: true,
        reasons: vec![BlockingReason {
            code: "SECURITY_ISSUE".to_string(),
            message: "SQL Injection vulnerability".to_string(),
            category: FindingCategory::Security,
            finding_ids: vec!["sec-1".to_string()],
        }],
        warnings: vec![],
        ready: false,
    };

    qa.record_review_result(&review, &gate);
    let transition = qa.check_and_transition(&gate);

    match transition {
        QATransition::NeedsFix { reasons, iteration } => {
            assert_eq!(iteration, 1);
            assert_eq!(reasons.len(), 1);
            assert_eq!(reasons[0].code, "SECURITY_ISSUE");
        }
        _ => panic!("Expected NeedsFix transition"),
    }

    assert_eq!(qa.state, QAState::AwaitingFix);
    assert_eq!(qa.pending_findings.len(), 1);
    assert_eq!(qa.pending_findings[0].id, "sec-1");
}

#[test]
fn test_successful_rereview_unblocks_merge() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let mut review1 = make_review_summary();
    review1.add_finding(ReviewFinding {
        id: "bug-1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Null pointer exception".to_string(),
        description: "Object not initialized".to_string(),
        location: None,
        suggestion: Some("Initialize object before use".to_string()),
        resolved: false,
    });
    review1.finalize();

    let gate1 = GateResult {
        blocked: true,
        reasons: vec![BlockingReason {
            code: "BUG_FOUND".to_string(),
            message: "Bug found".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["bug-1".to_string()],
        }],
        warnings: vec![],
        ready: false,
    };

    qa.record_review_result(&review1, &gate1);
    assert!(matches!(
        qa.check_and_transition(&gate1),
        QATransition::NeedsFix { .. }
    ));
    assert_eq!(qa.state, QAState::AwaitingFix);

    qa.start_fix();
    qa.record_fix_result(None, None);

    qa.start_rereview();
    let mut review2 = make_review_summary();
    review2.add_finding(ReviewFinding {
        id: "bug-1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Bug fixed".to_string(),
        description: "Object now properly initialized".to_string(),
        location: None,
        suggestion: None,
        resolved: true,
    });
    review2.finalize();

    let gate2 = GateResult {
        blocked: false,
        reasons: vec![],
        warnings: vec![],
        ready: true,
    };

    qa.record_review_result(&review2, &gate2);
    let transition = qa.check_and_transition(&gate2);

    assert!(matches!(transition, QATransition::Approved));
    assert_eq!(qa.state, QAState::Approved);
    assert!(qa.current_status().is_merge_ready);
}

#[test]
fn test_fix_mode_sets_appropriate_instruction() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.add_pending_finding(PendingFinding {
        id: "sec-1".to_string(),
        title: "SQL Injection".to_string(),
        category: "Security".to_string(),
        severity: "High".to_string(),
        suggestion: Some("Use parameterized queries".to_string()),
        created_at_iteration: 1,
    });

    qa.start_fix();
    qa.record_fix_result(None, None);

    assert_eq!(qa.state, QAState::InFix);
    assert_eq!(qa.history.len(), 1);
    assert!(matches!(qa.history[0].phase, QAPhase::Fix));
}

#[test]
fn test_multiple_findings_all_recorded() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();

    let mut review = make_review_summary();
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
    review.add_finding(ReviewFinding {
        id: "f2".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Memory leak".to_string(),
        description: "...".to_string(),
        location: None,
        suggestion: None,
        resolved: false,
    });
    review.finalize();

    let gate = GateResult {
        blocked: true,
        reasons: vec![
            BlockingReason {
                code: "SECURITY".to_string(),
                message: "Security issue".to_string(),
                category: FindingCategory::Security,
                finding_ids: vec!["f1".to_string()],
            },
            BlockingReason {
                code: "BUG".to_string(),
                message: "Bug".to_string(),
                category: FindingCategory::Correctness,
                finding_ids: vec!["f2".to_string()],
            },
        ],
        warnings: vec![],
        ready: false,
    };

    qa.record_review_result(&review, &gate);
    let transition = qa.check_and_transition(&gate);

    assert!(matches!(transition, QATransition::NeedsFix { reasons, .. } if reasons.len() == 2));
    assert_eq!(qa.pending_findings.len(), 2);
}

#[test]
fn test_history_preserves_all_iterations() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    for i in 1..=2 {
        qa.start_review();
        let review = make_review_summary();
        let blocked = i == 1;
        let gate = make_gate_result(blocked, vec![]);
        qa.record_review_result(&review, &gate);
        qa.check_and_transition(&gate);

        if blocked {
            qa.start_fix();
            qa.record_fix_result(None, None);
        }
    }

    assert_eq!(qa.history.len(), 3);
    assert_eq!(qa.history[0].phase, QAPhase::Review);
    assert_eq!(qa.history[1].phase, QAPhase::Fix);
    assert_eq!(qa.history[2].phase, QAPhase::Review);
}

#[test]
fn test_blocked_review_creates_pending_fixes() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let mut review = make_review_summary();
    review.add_finding(ReviewFinding {
        id: "f1".to_string(),
        category: FindingCategory::Security,
        severity: ReviewSeverity::High,
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
            message: "SQL Injection vulnerability".to_string(),
            category: FindingCategory::Security,
            finding_ids: vec!["f1".to_string()],
        }],
    );

    qa.record_review_result(&review, &gate);
    let transition = qa.check_and_transition(&gate);

    assert!(matches!(transition, QATransition::NeedsFix { .. }));
    assert_eq!(qa.state, QAState::AwaitingFix);
    assert_eq!(qa.pending_findings.len(), 1);
    assert_eq!(qa.pending_findings[0].id, "f1");
    assert_eq!(qa.pending_findings[0].created_at_iteration, 1);
}

#[test]
fn test_fix_mode_picks_up_pending_findings() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.add_pending_finding(PendingFinding {
        id: "f1".to_string(),
        title: "SQL Injection".to_string(),
        category: "Security".to_string(),
        severity: "High".to_string(),
        suggestion: Some("Use parameterized queries".to_string()),
        created_at_iteration: 1,
    });

    qa.start_fix();
    assert_eq!(qa.state, QAState::InFix);
    assert_eq!(qa.pending_findings.len(), 1);

    let pending_ids = qa.get_pending_finding_ids();
    assert_eq!(pending_ids, vec!["f1"]);
}

#[test]
fn test_rereview_clears_resolved_findings() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.add_pending_finding(PendingFinding {
        id: "f1".to_string(),
        title: "Bug fixed".to_string(),
        category: "Correctness".to_string(),
        severity: "High".to_string(),
        suggestion: None,
        created_at_iteration: 1,
    });

    let mut review = make_review_summary();
    review.add_finding(ReviewFinding {
        id: "f1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Bug fixed".to_string(),
        description: "Fixed".to_string(),
        location: None,
        suggestion: None,
        resolved: true,
    });
    review.finalize();

    let gate = make_gate_result(false, vec![]);
    let transition = qa.handle_rereview_result(&review, &gate);

    assert!(matches!(transition, QATransition::Approved));
    assert_eq!(qa.state, QAState::Approved);
    assert!(qa.pending_findings.is_empty());
}

#[test]
fn test_repeated_failures_escalate_cleanly() {
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
            finding_ids: vec!["f1".to_string()],
        }],
    );

    for iteration in 1..=3 {
        qa.start_review();
        let review = make_review_summary();
        qa.record_review_result(&review, &gate_blocked);
        qa.check_and_transition(&gate_blocked);

        if iteration < 3 {
            assert_eq!(qa.state, QAState::AwaitingFix);
            qa.start_fix();
            qa.record_fix_result(None, None);
        }
    }

    assert!(qa.should_escalate());
    assert_eq!(qa.state, QAState::Escalated);
    assert!(qa.escalation_reason.is_some());
    assert!(qa.current_status().needs_escalation);

    let status = qa.current_status();
    let display = status.display_summary();
    assert!(display.contains("Escalated"));
}

#[test]
fn test_full_fix_loop_approve_after_retry() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_review();
    let mut review1 = make_review_summary();
    review1.add_finding(ReviewFinding {
        id: "f1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Bug in code".to_string(),
        description: "Bug description".to_string(),
        location: None,
        suggestion: Some("Fix the bug".to_string()),
        resolved: false,
    });
    review1.finalize();

    let gate1 = make_gate_result(
        true,
        vec![BlockingReason {
            code: "BUG".to_string(),
            message: "Bug in code".to_string(),
            category: FindingCategory::Correctness,
            finding_ids: vec!["f1".to_string()],
        }],
    );

    qa.record_review_result(&review1, &gate1);
    let t1 = qa.check_and_transition(&gate1);
    assert!(matches!(t1, QATransition::NeedsFix { .. }));
    assert_eq!(qa.current_iteration, 1);

    qa.start_fix();
    qa.record_fix_result(None, None);

    qa.start_rereview();
    let mut review2 = make_review_summary();
    review2.add_finding(ReviewFinding {
        id: "f1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Bug fixed".to_string(),
        description: "Fixed now".to_string(),
        location: None,
        suggestion: None,
        resolved: true,
    });
    review2.finalize();

    let gate2 = make_gate_result(false, vec![]);
    qa.record_review_result(&review2, &gate2);
    let t2 = qa.check_and_transition(&gate2);

    assert!(matches!(t2, QATransition::Approved));
    assert_eq!(qa.state, QAState::Approved);
    assert!(qa.pending_findings.is_empty());
    assert_eq!(qa.history.len(), 3);
}

#[test]
fn test_fix_resolution_verification() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.add_pending_finding(PendingFinding {
        id: "f1".to_string(),
        title: "Bug 1".to_string(),
        category: "Correctness".to_string(),
        severity: "High".to_string(),
        suggestion: None,
        created_at_iteration: 1,
    });
    qa.add_pending_finding(PendingFinding {
        id: "f2".to_string(),
        title: "Bug 2".to_string(),
        category: "Correctness".to_string(),
        severity: "High".to_string(),
        suggestion: None,
        created_at_iteration: 1,
    });

    let mut review = make_review_summary();
    review.add_finding(ReviewFinding {
        id: "f1".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Bug 1 fixed".to_string(),
        description: "Fixed".to_string(),
        location: None,
        suggestion: None,
        resolved: true,
    });
    review.add_finding(ReviewFinding {
        id: "f2".to_string(),
        category: FindingCategory::Correctness,
        severity: ReviewSeverity::High,
        title: "Bug 2 still present".to_string(),
        description: "Still broken".to_string(),
        location: None,
        suggestion: Some("Fix it".to_string()),
        resolved: false,
    });

    let resolution = qa.verify_fix_resolution(&review);

    assert!(!resolution.all_resolved);
    assert_eq!(resolution.resolved_ids, vec!["f1"]);
    assert_eq!(resolution.still_blocking_ids, vec!["f2"]);
}

#[test]
fn test_validation_update_influences_state() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.start_fix();

    let mut validation = ValidationSummary::new(Some("task-1".to_string()));
    validation.confidence = crate::pipeline::validation_summary::Confidence::Low;
    validation.failed = 2;
    validation.total = 3;

    qa.update_from_validation(&validation);

    assert_eq!(qa.state, QAState::AwaitingFix);
    assert!(qa.last_validation.is_some());
}

#[test]
fn test_pending_findings_no_duplicates() {
    let mut qa = QALoop::new("task-1".to_string(), QAConfig::default());

    qa.add_pending_finding(PendingFinding {
        id: "f1".to_string(),
        title: "Bug 1".to_string(),
        category: "Correctness".to_string(),
        severity: "High".to_string(),
        suggestion: None,
        created_at_iteration: 1,
    });

    qa.add_pending_finding(PendingFinding {
        id: "f1".to_string(),
        title: "Bug 1 again".to_string(),
        category: "Correctness".to_string(),
        severity: "High".to_string(),
        suggestion: None,
        created_at_iteration: 1,
    });

    assert_eq!(qa.pending_findings.len(), 1);
}
