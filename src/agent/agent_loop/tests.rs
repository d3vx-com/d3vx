//! Tests for the agent loop module.

use super::*;
use crate::agent::tool_coordinator::ToolCoordinator;
use crate::providers::traits::StreamResult;
use crate::providers::{
    ComplexityTier, ModelInfo, ProviderError, Role, StopReason, StreamEvent, TokenUsage,
};
use std::sync::Arc;

#[test]
fn test_agent_config_default() {
    let config = AgentConfig::default();

    assert_eq!(config.model, "claude-sonnet-4-20250514");
    assert_eq!(config.max_iterations, config::DEFAULT_MAX_ITERATIONS);
    assert!(!config.session_id.is_empty());
}

#[tokio::test]
async fn test_agent_loop_pause_resume() {
    let provider = Arc::new(MockProvider::new());
    let tools = Arc::new(ToolCoordinator::new());
    let config = AgentConfig::default();

    let agent = AgentLoop::new(provider, tools, None, config);

    assert!(!agent.is_paused().await);

    agent.pause().await;
    assert!(agent.is_paused().await);

    agent.resume().await;
    assert!(!agent.is_paused().await);
}

#[tokio::test]
async fn test_agent_loop_add_message() {
    let provider = Arc::new(MockProvider::new());
    let tools = Arc::new(ToolCoordinator::new());
    let config = AgentConfig::default();

    let agent = AgentLoop::new(provider, tools, None, config);

    agent.add_user_message("Hello").await;

    let messages = agent.get_messages().await;
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, Role::User);
}

#[tokio::test]
async fn test_agent_loop_clear_history() {
    let provider = Arc::new(MockProvider::new());
    let tools = Arc::new(ToolCoordinator::new());
    let config = AgentConfig::default();

    let agent = AgentLoop::new(provider, tools, None, config);

    agent.add_user_message("Hello").await;
    assert_eq!(agent.get_messages().await.len(), 1);

    agent.clear_history().await;
    assert!(agent.get_messages().await.is_empty());
}

#[tokio::test]
async fn test_agent_loop_subscribe() {
    let provider = Arc::new(MockProvider::new());
    let tools = Arc::new(ToolCoordinator::new());
    let config = AgentConfig::default();

    let (agent_loop, mut _events) = AgentLoop::with_events(provider, tools, None, config);

    agent_loop.emit(AgentEvent::Text {
        text: "test".to_string(),
    });

    let event = _events.recv().await.expect("Failed to receive event");

    match event {
        AgentEvent::Text { text } => assert_eq!(text, "test"),
        _ => panic!("Expected Text event"),
    }
}

/// Mock provider for testing
struct MockProvider {
    model_info: ModelInfo,
    responses: std::sync::Mutex<Vec<Vec<Result<StreamEvent, ProviderError>>>>,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            model_info: ModelInfo {
                id: "test-model".to_string(),
                name: "Test Model".to_string(),
                provider: "mock".to_string(),
                tier: ComplexityTier::Standard,
                context_window: 100_000,
                max_output_tokens: 4_096,
                supports_tool_use: true,
                supports_vision: false,
                supports_streaming: true,
                supports_thinking: false,
                default_thinking_budget: None,
                cost_per_input_mtok: Some(1.0),
                cost_per_output_mtok: Some(2.0),
            },
            responses: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn add_response(&self, events: Vec<StreamEvent>) {
        self.responses
            .lock()
            .unwrap()
            .push(events.into_iter().map(Ok).collect());
    }

    #[allow(dead_code)]
    fn add_stream_error(&self, error: ProviderError) {
        self.responses.lock().unwrap().push(vec![Err(error)]);
    }

    fn add_mixed_response(&self, events: Vec<Result<StreamEvent, ProviderError>>) {
        self.responses.lock().unwrap().push(events);
    }
}

#[async_trait::async_trait]
impl crate::providers::Provider for MockProvider {
    async fn send(
        &self,
        _request: crate::providers::MessagesRequest,
    ) -> Result<StreamResult, ProviderError> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Ok(Box::pin(futures::stream::iter(vec![])));
        }

        let events = responses.remove(0);
        Ok(Box::pin(futures::stream::iter(events)))
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![self.model_info.clone()]
    }

    fn model_info(&self, _model_id: &str) -> Option<ModelInfo> {
        Some(self.model_info.clone())
    }

    fn is_available(&self) -> bool {
        true
    }

    fn estimate_cost(
        &self,
        _model: &str,
        _usage: &TokenUsage,
    ) -> Option<crate::providers::traits::CostEstimate> {
        None
    }
}

#[tokio::test]
async fn test_agent_loop_streaming_recovery() {
    let provider = Arc::new(MockProvider::new());
    let tools = Arc::new(ToolCoordinator::new());
    let config = AgentConfig {
        max_iterations: 5,
        ..AgentConfig::default()
    };

    provider.add_mixed_response(vec![
        Ok(StreamEvent::TextDelta {
            text: "Hello, I am".to_string(),
        }),
        Err(ProviderError::StreamError(
            "api_error: Internal Network Failure".to_string(),
        )),
    ]);

    provider.add_response(vec![
        StreamEvent::TextDelta {
            text: " Claude, how can I help?".to_string(),
        },
        StreamEvent::MessageEnd {
            usage: TokenUsage {
                input_tokens: 10,
                output_tokens: 10,
                ..Default::default()
            },
            stop_reason: StopReason::EndTurn,
        },
    ]);

    let agent = AgentLoop::new(provider, tools, None, config);
    agent.add_user_message("Who are you?").await;

    let result = agent
        .run()
        .await
        .expect("Agent should recover and complete");

    assert!(result.text.contains("Hello, I am"));
    assert!(result.text.contains("[Network interruption. Retrying...]"));
    assert!(result.text.contains("Claude, how can I help?"));
    assert_eq!(result.iterations, 2);
}

#[tokio::test]
async fn test_agent_loop_max_iterations() {
    let provider = Arc::new(MockProvider::new());
    let tools = Arc::new(ToolCoordinator::new());
    let config = AgentConfig {
        max_iterations: 2,
        ..AgentConfig::default()
    };

    for _ in 0..5 {
        provider.add_response(vec![
            StreamEvent::ToolUseStart {
                id: "t1".to_string(),
                name: "ReadTool".to_string(),
            },
            StreamEvent::ToolUseEnd {
                id: "t1".to_string(),
                name: "ReadTool".to_string(),
                input: serde_json::json!({"path": "test.txt"}),
            },
            StreamEvent::MessageEnd {
                usage: TokenUsage::default(),
                stop_reason: StopReason::ToolUse,
            },
        ]);
    }

    let agent = AgentLoop::new(provider, tools, None, config);
    agent.add_user_message("Keep reading").await;

    let result = agent
        .run()
        .await
        .expect("Agent should finish even if at max iterations");
    assert_eq!(result.iterations, 2);
}
