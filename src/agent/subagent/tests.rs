//! Sub-agent tests

use super::*;
use crate::config::CleanupConfig;
use crate::providers::{
    CostEstimate, MessagesRequest, ModelInfo, Provider, ProviderError, StreamResult, TokenUsage,
};
use async_trait::async_trait;
use chrono::Utc;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

struct MockProvider;
#[async_trait]
impl Provider for MockProvider {
    async fn send(&self, _request: MessagesRequest) -> Result<StreamResult, ProviderError> {
        Err(ProviderError::Unavailable("Mock".to_string()))
    }
    fn name(&self) -> &str {
        "mock"
    }
    fn models(&self) -> Vec<ModelInfo> {
        vec![]
    }
    fn model_info(&self, _model_id: &str) -> Option<ModelInfo> {
        None
    }
    fn is_available(&self) -> bool {
        true
    }
    fn estimate_cost(&self, _model: &str, _usage: &TokenUsage) -> Option<CostEstimate> {
        None
    }
}

#[tokio::test]
async fn test_subagent_cleanup_logic() {
    let manager = SubAgentManager::new();
    let temp_dir = tempdir().unwrap();
    // Create a sub-directory with the expected name pattern to satisfy the guard
    let worktree_dir = temp_dir.path().join("d3vx_worktrees_test_agent");
    fs::create_dir_all(&worktree_dir).unwrap();
    let worktree_path = worktree_dir.to_string_lossy().to_string();

    // Create a dummy file in the worktree
    fs::write(worktree_dir.join("dummy.txt"), "hello").unwrap();

    let handle = SubAgentHandle {
        id: "test-agent".to_string(),
        task: "test-task".to_string(),
        status: SubAgentStatus::Completed,
        start_time: Utc::now() - chrono::Duration::hours(1),
        end_time: Some(Utc::now() - chrono::Duration::hours(1)),
        iterations: 1,
        last_activity: Utc::now() - chrono::Duration::hours(1), // 1 hour ago
        error: None,
        result: Some("success".to_string()),
        parent_session_id: None,
        worktree_path: Some(worktree_path.clone()),
        current_action: None,
    };

    // Inject handle
    {
        let mut agents = manager.agents.write().await;
        agents.insert(handle.id.clone(), handle);
    }

    // Configure cleanup: retain 0 completed, prune intensity 1.0 (immediate)
    let config = CleanupConfig {
        retention_period_secs: 0,
        cleanup_interval_secs: 60,
        max_retained: 0,
    };

    // Run cleanup
    manager.cleanup(&config).await;

    // Verify handle was pruned
    let agents = manager.agents.read().await;
    assert!(agents.get("test-agent").is_none());

    // Verify worktree was deleted
    assert!(!worktree_dir.exists());
}

#[tokio::test]
async fn test_subagent_cleanup_retention() {
    let manager = SubAgentManager::new();

    let handle = SubAgentHandle {
        id: "test-agent-recent".to_string(),
        task: "test-task".to_string(),
        status: SubAgentStatus::Completed,
        start_time: Utc::now(),
        end_time: Some(Utc::now()),
        iterations: 1,
        last_activity: Utc::now(), // Very recent
        error: None,
        result: Some("success".to_string()),
        parent_session_id: None,
        worktree_path: None,
        current_action: None,
    };

    {
        let mut agents = manager.agents.write().await;
        agents.insert(handle.id.clone(), handle);
    }

    let config = CleanupConfig {
        retention_period_secs: 300,
        cleanup_interval_secs: 60,
        max_retained: 10,
    };

    manager.cleanup(&config).await;

    let agents = manager.agents.read().await;
    assert!(agents.get("test-agent-recent").is_some());
}

#[tokio::test]
async fn test_mock_provider_usage() {
    let provider = Arc::new(MockProvider);
    assert_eq!(provider.name(), "mock");
    assert!(provider.is_available());
}
