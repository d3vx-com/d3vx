//! Tests for the reaction engine.

use super::*;

#[test]
fn test_reaction_event_task_id() {
    let event = ReactionEvent::CIFailure {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        context: "ci/tests".to_string(),
        description: "Tests failed".to_string(),
        target_url: None,
        task_id: Some("TASK-001".to_string()),
    };
    assert_eq!(event.task_id(), Some("TASK-001"));
}
#[test]
fn test_reaction_event_type() {
    let event = ReactionEvent::CIFailure {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        context: "ci/tests".to_string(),
        description: "Tests failed".to_string(),
        target_url: None,
        task_id: None,
    };
    assert_eq!(event.event_type(), "ci_failure");
}
#[test]
fn test_reaction_config_default() {
    let config = ReactionConfig::default();
    assert!(config.globally_enabled);
    assert!(config.ci_failure.enabled);
    assert_eq!(config.ci_failure.max_retries, 3);
    assert!(config.merge_conflict.notify_always);
    assert!(!config.merge_conflict.auto_resolve);
}
#[test]
fn test_reaction_config_yaml() {
    let yaml = r#"
ci_failure:
  enabled: true
  max_retries: 5
  auto_fix: false
merge_conflict:
  auto_resolve: true
"#;
    let config = ReactionConfig::from_yaml(yaml).unwrap();
    assert_eq!(config.ci_failure.max_retries, 5);
    assert!(!config.ci_failure.auto_fix);
    assert!(config.merge_conflict.auto_resolve);
}

#[tokio::test]
async fn test_ci_failure_auto_fix() {
    let engine = ReactionEngine::new();
    let event = ReactionEvent::CIFailure {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        context: "ci/tests".to_string(),
        description: "Tests failed".to_string(),
        target_url: None,
        task_id: Some("TASK-001".to_string()),
    };

    let result = engine.process_event(event).await;
    assert_eq!(result.reaction, ReactionType::AutoFix);
    assert!(result.reason.contains("auto-fix"));
}
#[tokio::test]
async fn test_ci_failure_max_retries_exceeded() {
    let config = ReactionConfig::default().with_ci_failure(CIFailureConfig {
        max_retries: 2,
        ..Default::default()
    });
    let engine = ReactionEngine::with_config(config);

    let event = ReactionEvent::CIFailure {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        context: "ci/tests".to_string(),
        description: "Tests failed".to_string(),
        target_url: None,
        task_id: Some("TASK-001".to_string()),
    };

    let result = engine.process_event(event.clone()).await;
    assert_eq!(result.reaction, ReactionType::AutoFix);

    let result = engine.process_event(event.clone()).await;
    assert_eq!(result.reaction, ReactionType::AutoFix);

    let result = engine.process_event(event.clone()).await;
    assert_eq!(result.reaction, ReactionType::Notify);
    assert!(result.reason.contains("Max retries"));
}
#[tokio::test]
async fn test_review_comment_trivial() {
    let engine = ReactionEngine::new();
    let event = ReactionEvent::ReviewComment {
        pr_number: 42,
        repository: "owner/repo".to_string(),
        author: "reviewer".to_string(),
        body: "There's a typo in the variable name".to_string(),
        changes_requested: true,
        task_id: Some("TASK-001".to_string()),
    };

    let result = engine.process_event(event).await;
    assert_eq!(result.reaction, ReactionType::AutoFix);
    assert!(result.reason.contains("Trivial"));
}
#[tokio::test]
async fn test_review_comment_complex() {
    let engine = ReactionEngine::new();
    let event = ReactionEvent::ReviewComment {
        pr_number: 42,
        repository: "owner/repo".to_string(),
        author: "reviewer".to_string(),
        body: "This is a security vulnerability that needs architectural changes".to_string(),
        changes_requested: true,
        task_id: Some("TASK-001".to_string()),
    };

    let result = engine.process_event(event).await;
    assert_eq!(result.reaction, ReactionType::Escalate);
    assert!(result.reason.contains("Complex"));
}
#[tokio::test]
async fn test_merge_conflict_notify() {
    let engine = ReactionEngine::new();
    let event = ReactionEvent::MergeConflict {
        worktree_path: "/path/to/worktree".to_string(),
        base_branch: "main".to_string(),
        conflicted_files: vec!["src/lib.rs".to_string()],
        task_id: "TASK-001".to_string(),
    };

    let result = engine.process_event(event).await;
    assert_eq!(result.reaction, ReactionType::Notify);
}
#[tokio::test]
async fn test_merge_conflict_auto_resolve() {
    let config = ReactionConfig::default().with_merge_conflict(MergeConflictConfig {
        auto_resolve: true,
        ..Default::default()
    });
    let engine = ReactionEngine::with_config(config);

    let event = ReactionEvent::MergeConflict {
        worktree_path: "/path/to/worktree".to_string(),
        base_branch: "main".to_string(),
        conflicted_files: vec!["src/lib.rs".to_string()],
        task_id: "TASK-001".to_string(),
    };

    let result = engine.process_event(event).await;
    assert_eq!(result.reaction, ReactionType::AutoFix);
    assert!(result.reason.contains("auto-resolution"));
}
#[tokio::test]
async fn test_agent_idle_notify() {
    let engine = ReactionEngine::new();
    let event = ReactionEvent::AgentIdle {
        worker_id: 1,
        task_id: "TASK-001".to_string(),
        idle_duration_secs: 300,
        last_phase: Some("implement".to_string()),
    };

    let result = engine.process_event(event).await;
    assert_eq!(result.reaction, ReactionType::Checkpoint);
}
#[tokio::test]
async fn test_agent_idle_max_exceeded() {
    let config = ReactionConfig::default().with_agent_idle(AgentIdleConfig {
        max_idle_secs: 600,
        ..Default::default()
    });
    let engine = ReactionEngine::with_config(config);

    let event = ReactionEvent::AgentIdle {
        worker_id: 1,
        task_id: "TASK-001".to_string(),
        idle_duration_secs: 1200,
        last_phase: Some("implement".to_string()),
    };

    let result = engine.process_event(event).await;
    assert_eq!(result.reaction, ReactionType::Cancel);
    assert!(result.reason.contains("exceeds maximum"));
}

#[tokio::test]
async fn test_audit_trail() {
    let engine = ReactionEngine::new();
    let event = ReactionEvent::CIFailure {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        context: "ci/tests".to_string(),
        description: "Tests failed".to_string(),
        target_url: None,
        task_id: Some("TASK-001".to_string()),
    };

    engine.process_event(event.clone()).await;
    engine.process_event(event.clone()).await;

    let trail = engine.get_audit_trail().await;
    assert_eq!(trail.len(), 2);

    let task_trail = engine.get_audit_trail_for_task("TASK-001").await;
    assert_eq!(task_trail.len(), 2);
}

#[tokio::test]
async fn test_stats() {
    let engine = ReactionEngine::new();
    let event = ReactionEvent::CIFailure {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        context: "ci/tests".to_string(),
        description: "Tests failed".to_string(),
        target_url: None,
        task_id: Some("TASK-001".to_string()),
    };

    engine.process_event(event.clone()).await;

    let stats = engine.stats().await;
    assert_eq!(stats.total_events, 1);
    assert_eq!(stats.auto_fix_attempts, 1);
}

#[test]
fn test_notification_payload_from_result() {
    let event = ReactionEvent::CIFailure {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        context: "ci/tests".to_string(),
        description: "Tests failed".to_string(),
        target_url: None,
        task_id: Some("TASK-001".to_string()),
    };
    let result = ReactionResult::new(
        event,
        ReactionType::Notify,
        "Max retries exceeded".to_string(),
    );

    let payload = NotificationPayload::from_result(&result);
    assert_eq!(payload.severity, NotificationSeverity::Warning);
    assert!(payload.recommended_action.is_some());
}

#[test]
fn test_reaction_result_builder() {
    let event = ReactionEvent::CIFailure {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        context: "ci/tests".to_string(),
        description: "Tests failed".to_string(),
        target_url: None,
        task_id: Some("TASK-001".to_string()),
    };

    let result = ReactionResult::new(event.clone(), ReactionType::AutoFix, "Fixing".to_string())
        .with_executed()
        .with_metadata("key".to_string(), "value".to_string());

    assert!(result.executed);
    assert_eq!(result.metadata.get("key"), Some(&"value".to_string()));
}

#[tokio::test]
async fn test_disabled_reactions() {
    let engine = ReactionEngine::with_config(ReactionConfig::disabled());
    let event = ReactionEvent::CIFailure {
        repository: "owner/repo".to_string(),
        branch: "main".to_string(),
        commit_sha: "abc123".to_string(),
        context: "ci/tests".to_string(),
        description: "Tests failed".to_string(),
        target_url: None,
        task_id: Some("TASK-001".to_string()),
    };

    let result = engine.process_event(event).await;
    assert_eq!(result.reaction, ReactionType::NoAction);
    assert!(result.reason.contains("globally disabled"));
}
