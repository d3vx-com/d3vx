//! Spawn Parallel Tool
//!
//! Allows the agent to request parallel execution of independent subtasks
//! with optional specialized agent types.

use crate::agent::specialists::AgentType;
use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Event sent when parallel agents are requested
pub struct SpawnParallelEvent {
    pub batch_id: String,
    pub parent_session_id: Option<String>,
    pub reasoning: String,
    pub select_best: bool,
    pub selection_criteria: Option<String>,
    pub tasks: Vec<SpawnTask>,
    pub response_tx: tokio::sync::oneshot::Sender<String>,
}

/// A single task to spawn with optional agent type
#[derive(Debug, Clone)]
pub struct SpawnTask {
    pub key: String,
    pub description: String,
    pub task: String,
    pub agent_type: AgentType,
    pub depends_on: Vec<String>,
    pub ownership: Option<String>,
    pub model: Option<String>,
    pub max_turns: Option<u32>,
}

impl std::fmt::Debug for SpawnParallelEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnParallelEvent")
            .field("batch_id", &self.batch_id)
            .field("parent_session_id", &self.parent_session_id)
            .field("reasoning", &self.reasoning)
            .field("select_best", &self.select_best)
            .field("selection_criteria", &self.selection_criteria)
            .field("tasks", &self.tasks)
            .finish()
    }
}

impl SpawnParallelEvent {
    pub fn new(
        batch_id: String,
        parent_session_id: Option<String>,
        reasoning: String,
        select_best: bool,
        selection_criteria: Option<String>,
        tasks: Vec<SpawnTask>,
        response_tx: tokio::sync::oneshot::Sender<String>,
    ) -> Self {
        Self {
            batch_id,
            parent_session_id,
            reasoning,
            select_best,
            selection_criteria,
            tasks,
            response_tx,
        }
    }
}

/// Tool for spawning parallel agents to execute independent subtasks simultaneously.
/// Each agent can optionally be specialized for a specific SDLC role.
pub struct SpawnParallelTool {
    sender: Option<Arc<std::sync::Mutex<mpsc::Sender<SpawnParallelEvent>>>>,
}

impl SpawnParallelTool {
    pub fn new() -> Self {
        Self { sender: None }
    }

    pub fn with_sender(sender: mpsc::Sender<SpawnParallelEvent>) -> Self {
        Self {
            sender: Some(Arc::new(std::sync::Mutex::new(sender))),
        }
    }

    pub fn set_sender(&mut self, sender: mpsc::Sender<SpawnParallelEvent>) {
        self.sender = Some(Arc::new(std::sync::Mutex::new(sender)));
    }
}

impl Default for SpawnParallelTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SpawnParallelTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "spawn_parallel_agents".to_string(),
            description: r#"SPAWN PARALLEL AGENTS - This is the ONLY way to run multiple agents simultaneously.

Use this tool to spawn 2-5 parallel agent loops that execute independent subtasks at the same time.

**Consolidation:** Once all agents complete, a **Compiled Parallel Execution Report** will be automatically added to the main session. This report pulls 'final summaries' from every agent and detects modified files in their worktrees, allowing you to synthesize the final result without manual inspection.

IMPORTANT: DO NOT use bash, shell scripts, or subprocess to spawn agents. Use this tool instead.

**When to use:**
- Task has independent parts that can run simultaneously
- You need faster completion through parallelism
- Multiple domains need different expertise (backend + frontend + tests)

**Agent Types:**
- backend: APIs, database, business logic
- frontend: UI, components, styling
- testing: Unit, integration, e2e tests
- documentation: README, docs, guides
- devops: CI/CD, deployment, Docker
- security: Security audits
- review: Code review
- data: Data pipelines, ETL
- mobile: iOS/Android
- general: Any task type

**Example:**
{
  "subtasks": [
    {"key": "backend", "description": "Backend API", "task": "Implement REST endpoints", "agent_type": "backend", "ownership": "src/api, db/schema.sql"},
    {"key": "frontend", "description": "Frontend UI", "task": "Build React forms", "agent_type": "frontend", "ownership": "src/ui"},
    {"key": "tests", "description": "Tests", "task": "Write integration tests", "agent_type": "testing", "depends_on": ["backend", "frontend"], "ownership": "tests/"}
  ],
  "reasoning": "These 3 components are independent and can run in parallel"
}"#
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "subtasks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "description": {
                                    "type": "string",
                                    "description": "Brief description of this subtask"
                                },
                                "key": {
                                    "type": "string",
                                    "description": "Stable identifier for this child task. Recommended when using dependencies.",
                                },
                                "task": {
                                    "type": "string",
                                    "description": "The actual task to execute for this subtask"
                                },
                                "agent_type": {
                                    "type": "string",
                                    "enum": ["general", "backend", "frontend", "testing", "documentation", "devops", "security", "review", "data", "mobile"],
                                    "description": "Optional specialized agent type for this task. The orchestrator decides based on task context.",
                                    "default": "general"
                                },
                                "depends_on": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Optional list of child task keys that must finish before this child starts."
                                },
                                "ownership": {
                                    "type": "string",
                                    "description": "Optional file/module ownership hint for this child task, for example 'src/api, db/schema.sql'."
                                },
                                "model": {
                                    "type": "string",
                                    "description": "Optional model override for this child task (e.g., 'claude-sonnet-4-6', 'claude-haiku-4-5')."
                                },
                                "max_turns": {
                                    "type": "number",
                                    "description": "Optional maximum number of tool-use turns for this child task (default: 50)."
                                }
                            },
                            "required": ["description", "task"]
                        },
                        "minItems": 2,
                        "maxItems": 5,
                        "description": "List of 2-5 independent subtasks to execute in parallel"
                    },
                    "reasoning": {
                        "type": "string",
                        "description": "Explain why these tasks are independent and can run in parallel"
                    },
                    "select_best": {
                        "type": "boolean",
                        "description": "If true, treat child outputs as competing candidates and automatically select the best result after all complete.",
                        "default": false
                    },
                    "selection_criteria": {
                        "type": "string",
                        "description": "Optional custom criteria for selecting the best candidate output when select_best is true."
                    }
                },
                "required": ["subtasks", "reasoning"]
            }),
        }
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        if !context.allow_parallel_spawn || context.agent_depth > 0 {
            return ToolResult::error(
                "Delegated agents cannot spawn more agents. Return your result to the parent coordinator instead.",
            );
        }

        let subtasks = match input["subtasks"].as_array() {
            Some(arr) => arr,
            None => {
                return ToolResult::error("Missing 'subtasks' array");
            }
        };

        if subtasks.len() < 2 {
            return ToolResult::error("Need at least 2 subtasks for parallel execution");
        }

        if subtasks.len() > 5 {
            return ToolResult::error("Maximum 5 parallel agents allowed");
        }

        let reasoning = input["reasoning"]
            .as_str()
            .unwrap_or("No reasoning provided");
        let select_best = input
            .get("select_best")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let selection_criteria = input
            .get("selection_criteria")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        // Parse subtasks with optional agent types
        let mut spawn_tasks = Vec::new();
        for (i, s) in subtasks.iter().enumerate() {
            let description = s
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or(&format!("Task {}", i + 1))
                .to_string();
            let key = s
                .get("key")
                .and_then(|k| k.as_str())
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty())
                .unwrap_or_else(|| format!("child-{}", i + 1));

            let task = match s.get("task").and_then(|t| t.as_str()) {
                Some(t) => t.to_string(),
                None => {
                    return ToolResult::error(&format!(
                        "Subtask {} is missing 'task' field",
                        i + 1
                    ));
                }
            };

            // Parse agent type (defaults to General if not specified or invalid)
            let agent_type = s
                .get("agent_type")
                .and_then(|at| at.as_str())
                .and_then(|at| parse_agent_type(at))
                .unwrap_or(AgentType::General);
            let depends_on = s
                .get("depends_on")
                .and_then(|deps| deps.as_array())
                .map(|deps| {
                    deps.iter()
                        .filter_map(|dep| dep.as_str())
                        .map(|dep| dep.to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let ownership = s
                .get("ownership")
                .and_then(|ownership| ownership.as_str())
                .map(|ownership| ownership.trim().to_string())
                .filter(|ownership| !ownership.is_empty());
            let model = s
                .get("model")
                .and_then(|m| m.as_str())
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty());
            let max_turns = s
                .get("max_turns")
                .and_then(|m| m.as_u64())
                .map(|m| m as u32);

            spawn_tasks.push(SpawnTask {
                key,
                description,
                task,
                agent_type,
                depends_on,
                ownership,
                model,
                max_turns,
            });
        }

        // Create response channel for blocking execution
        let (tx, rx) = tokio::sync::oneshot::channel();

        // Emit event to spawn agents
        tracing::info!(
            "SpawnParallelTool: {} tasks parsed, attempting to send event",
            spawn_tasks.len()
        );
        if let Some(ref sender_arc) = self.sender {
            if let Ok(sender) = sender_arc.lock() {
                let event = SpawnParallelEvent::new(
                    Uuid::new_v4().to_string(),
                    context.session_id.clone(),
                    reasoning.to_string(),
                    select_best,
                    selection_criteria.clone(),
                    spawn_tasks.clone(),
                    tx,
                );
                tracing::info!("SpawnParallelTool: trying to send event to channel");
                match sender.try_send(event) {
                    Ok(_) => {
                        tracing::info!(
                            "SpawnParallelTool: event sent successfully, waiting for response"
                        );
                    }
                    Err(e) => {
                        tracing::error!("SpawnParallelTool: failed to send event: {:?}", e);
                        return ToolResult::error(&format!(
                            "Failed to send spawn event to app: {:?}",
                            e
                        ));
                    }
                }
            } else {
                tracing::error!("SpawnParallelTool: failed to lock sender");
                return ToolResult::error("Failed to acquire tool lock");
            }
        } else {
            // No sender configured — graceful degradation (e.g., in tests or standalone mode)
            tracing::warn!(
                "SpawnParallelTool: sender not set, returning parsed tasks without dispatch"
            );
            let summary = spawn_tasks
                .iter()
                .enumerate()
                .map(|(i, t)| format!("  {}. {}", i + 1, t.description))
                .collect::<Vec<_>>()
                .join("\n");
            return ToolResult::success(&format!(
                "Spawning {} parallel specialist agents (no dispatcher connected):\n{}",
                spawn_tasks.len(),
                summary
            ));
        }

        // Wait for batch completion and compilation
        match rx.await {
            Ok(report) => ToolResult::success(report),
            Err(e) => ToolResult::error(&format!(
                "Agent batch execution failed or was cancelled: {:?}",
                e
            )),
        }
    }
}

/// Parse agent type from string
fn parse_agent_type(s: &str) -> Option<AgentType> {
    match s.to_lowercase().as_str() {
        "general" => Some(AgentType::General),
        "backend" => Some(AgentType::Backend),
        "frontend" => Some(AgentType::Frontend),
        "testing" | "test" | "qa" => Some(AgentType::Testing),
        "documentation" | "docs" | "doc" => Some(AgentType::Documentation),
        "devops" | "deployment" | "infra" => Some(AgentType::DevOps),
        "security" | "audit" => Some(AgentType::Security),
        "review" | "reviewer" | "code_review" => Some(AgentType::Review),
        "data" | "data_engineering" => Some(AgentType::Data),
        "mobile" | "ios" | "android" => Some(AgentType::Mobile),
        _ => None,
    }
}
