//! Tests for [`AgentLoopDriver`] — the real-agent adapter used by the
//! eval runner.
//!
//! Uses a tiny mock provider so tests run in <100ms without touching
//! the network. The mock is dedicated to this file (not shared with
//! the agent-loop tests) to keep each test module self-contained.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream;

use crate::agent::tool_coordinator::ToolCoordinator;
use crate::agent::{AgentConfig, AgentLoop};
use crate::evals::agent_loop_driver::AgentLoopDriver;
use crate::evals::environment::EvalEnvironment;
use crate::evals::grader::GraderSpec;
use crate::evals::result::EvalResult;
use crate::evals::runner::{AgentDriver, EvalRunner};
use crate::evals::task::EvalTask;
use crate::providers::{
    traits::StreamResult, ComplexityTier, ModelInfo, Provider, ProviderError, StopReason,
    StreamEvent, TokenUsage,
};

/// Minimal Provider that replays a fixed stream of events per turn.
/// Each `send` call consumes one response from the queue; an empty
/// queue returns an empty stream (which ends the turn).
struct MockProvider {
    responses: std::sync::Mutex<Vec<Vec<Result<StreamEvent, ProviderError>>>>,
    model_info: ModelInfo,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            responses: std::sync::Mutex::new(Vec::new()),
            model_info: ModelInfo {
                id: "mock".into(),
                name: "Mock".into(),
                provider: "mock".into(),
                tier: ComplexityTier::Standard,
                context_window: 100_000,
                max_output_tokens: 4096,
                supports_tool_use: true,
                supports_vision: false,
                supports_streaming: true,
                supports_thinking: false,
                default_thinking_budget: None,
                cost_per_input_mtok: Some(1.0),
                cost_per_output_mtok: Some(2.0),
            },
        }
    }

    fn push(&self, events: Vec<StreamEvent>) {
        self.responses
            .lock()
            .unwrap()
            .push(events.into_iter().map(Ok).collect());
    }
}

#[async_trait]
impl Provider for MockProvider {
    async fn send(
        &self,
        _request: crate::providers::MessagesRequest,
    ) -> Result<StreamResult, ProviderError> {
        let mut q = self.responses.lock().unwrap();
        if q.is_empty() {
            return Ok(Box::pin(stream::iter(vec![])));
        }
        Ok(Box::pin(stream::iter(q.remove(0))))
    }

    fn name(&self) -> &str {
        "mock"
    }
    fn models(&self) -> Vec<ModelInfo> {
        vec![self.model_info.clone()]
    }
    fn model_info(&self, _id: &str) -> Option<ModelInfo> {
        Some(self.model_info.clone())
    }
    fn is_available(&self) -> bool {
        true
    }
    fn estimate_cost(
        &self,
        _m: &str,
        _u: &TokenUsage,
    ) -> Option<crate::providers::traits::CostEstimate> {
        None
    }
}

fn fresh_env_root(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "d3vx-evals-drv-{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn end_turn_response() -> Vec<StreamEvent> {
    // Minimum stream: zero text, EndTurn. Agent loop runs one iteration
    // and returns.
    vec![StreamEvent::MessageEnd {
        usage: TokenUsage {
            input_tokens: 5,
            output_tokens: 10,
            ..Default::default()
        },
        stop_reason: StopReason::EndTurn,
    }]
}

fn build_agent(provider: Arc<MockProvider>) -> Arc<AgentLoop> {
    let tools = Arc::new(ToolCoordinator::new());
    let cfg = AgentConfig {
        max_iterations: 5,
        ..AgentConfig::default()
    };
    Arc::new(AgentLoop::new(provider, tools, None, cfg))
}

fn empty_task(id: &str) -> EvalTask {
    EvalTask {
        id: id.into(),
        name: id.into(),
        description: None,
        instruction: "say hi".into(),
        setup: Vec::new(),
        graders: Vec::new(),
        budget_usd: None,
        max_iterations: None,
        timeout_secs: None,
        tags: Vec::new(),
    }
}

#[tokio::test]
async fn driver_runs_agent_and_returns_metrics_from_result() {
    let provider = Arc::new(MockProvider::new());
    provider.push(end_turn_response());

    let agent = build_agent(provider.clone());
    let driver = AgentLoopDriver::new(agent);
    let task = empty_task("t1");
    let env = EvalEnvironment::adopt("t1", std::env::temp_dir().join("dummy"));

    let metrics = driver.run(&task, &env).await.unwrap();
    assert_eq!(metrics.iterations, Some(1));
    assert_eq!(metrics.tool_calls, Some(0));
}

#[tokio::test]
async fn driver_points_agent_at_eval_workspace() {
    let provider = Arc::new(MockProvider::new());
    provider.push(end_turn_response());

    let agent = build_agent(provider.clone());
    let driver = AgentLoopDriver::new(agent.clone());

    let root = fresh_env_root("cwd");
    let env = EvalEnvironment::adopt("t1", root.join("workspace"));
    fs::create_dir_all(&env.workspace_path).unwrap();
    let task = empty_task("t1");

    driver.run(&task, &env).await.unwrap();

    let cfg = agent.config.read().await;
    assert_eq!(cfg.working_dir, env.workspace_path.to_string_lossy());
    drop(cfg);
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn driver_clears_history_between_tasks() {
    let provider = Arc::new(MockProvider::new());
    provider.push(end_turn_response());
    provider.push(end_turn_response());

    let agent = build_agent(provider.clone());
    let driver = AgentLoopDriver::new(agent.clone());
    let root = fresh_env_root("clear");

    let env = EvalEnvironment::adopt("a", root.join("a"));
    fs::create_dir_all(&env.workspace_path).unwrap();
    driver.run(&empty_task("a"), &env).await.unwrap();
    driver.run(&empty_task("b"), &env).await.unwrap();

    // After two runs with clear_history between, the conversation has
    // at most the last run's messages (one user turn).
    let messages = agent.get_messages().await;
    let user_turns = messages
        .iter()
        .filter(|m| matches!(m.role, crate::providers::Role::User))
        .count();
    assert_eq!(user_turns, 1, "history should have been cleared");
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn runner_end_to_end_passes_when_agent_satisfies_grader() {
    // The mock agent doesn't create files; we use a grader that the
    // environment's setup satisfies, so the agent only needs to not
    // destroy the state. This confirms the full pipeline wires through.
    let provider = Arc::new(MockProvider::new());
    provider.push(end_turn_response());

    let agent = build_agent(provider.clone());
    let driver = AgentLoopDriver::new(agent);

    let root = fresh_env_root("end2end");
    let mut task = empty_task("t1");
    task.setup = vec!["touch marker.txt".into()];
    task.graders = vec![GraderSpec::FileExists {
        path: "marker.txt".into(),
    }];

    let runner = EvalRunner::new(&root);
    let result: EvalResult = runner.run(&task, &driver).await;
    assert!(result.passed, "expected pass, harness err={:?}", result.harness_error);
    assert_eq!(result.iterations, Some(1));
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn runner_reports_harness_failure_when_agent_errors_out() {
    // Push no responses — the provider returns empty streams. The agent
    // loop interprets this as end-of-turn with no content; task
    // completes as pass *iff* the grader is satisfied. Here we use a
    // grader that requires a missing file so the grading step fails,
    // confirming the driver surfaces real runs cleanly.
    let provider = Arc::new(MockProvider::new());
    provider.push(end_turn_response());

    let agent = build_agent(provider.clone());
    let driver = AgentLoopDriver::new(agent);

    let root = fresh_env_root("fail");
    let mut task = empty_task("t1");
    task.graders = vec![GraderSpec::FileExists {
        path: "never-created.txt".into(),
    }];

    let runner = EvalRunner::new(&root);
    let result = runner.run(&task, &driver).await;
    assert!(!result.passed);
    assert!(result.harness_error.is_none(), "not a harness failure");
    assert_eq!(result.grader_outcomes.len(), 1);
    assert!(!result.grader_outcomes[0].passed);
    fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn driver_exposes_agent_handle() {
    let provider = Arc::new(MockProvider::new());
    let agent = build_agent(provider);
    let driver = AgentLoopDriver::new(agent.clone());
    assert!(Arc::ptr_eq(driver.agent(), &agent));
}
