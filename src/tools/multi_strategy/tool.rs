//! MultiStrategyTool — generates strategy-specific prompt variations
//! for parallel execution by the agent loop.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::strategy::{clamp_max_agents, parse_strategies};
use crate::tools::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Multi-strategy tool — a strategy planner that generates prompt variations
/// for parallel execution by the agent loop.
pub struct MultiStrategyTool {
    definition: ToolDefinition,
}

impl MultiStrategyTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "multi_strategy".to_string(),
                description: concat!(
                    "Generate multiple strategy-specific prompt variations for a task. ",
                    "Each strategy emphasizes a different implementation approach ",
                    "(concise, thorough, creative). Return structured prompts that ",
                    "can be run in parallel via spawn_parallel, then evaluated with ",
                    "best_of_n to select the best result."
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "The task to implement"
                        },
                        "strategies": {
                            "type": "array",
                            "items": {
                                "type": "string",
                                "enum": ["concise", "thorough", "creative"]
                            },
                            "description": "Which strategies to use (default: all three)"
                        },
                        "max_agents": {
                            "type": "integer",
                            "minimum": 2,
                            "maximum": 3,
                            "default": 2,
                            "description": "Maximum number of parallel agents (2-3)"
                        },
                        "evaluation_criteria": {
                            "type": "string",
                            "description": "How to pick the winning result (default: correctness and code quality)"
                        }
                    },
                    "required": ["task"]
                }),
            },
        }
    }
}

impl Default for MultiStrategyTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for MultiStrategyTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: Value, _context: &ToolContext) -> ToolResult {
        let task = match input.get("task").and_then(|t| t.as_str()) {
            Some(t) if !t.trim().is_empty() => t.trim(),
            _ => return ToolResult::error("Missing required field: task"),
        };

        let strategies = parse_strategies(input.get("strategies").unwrap_or(&Value::Null));
        let max_agents = clamp_max_agents(input.get("max_agents"));
        let evaluation_criteria = input
            .get("evaluation_criteria")
            .and_then(|v| v.as_str())
            .unwrap_or("correctness, code quality, and maintainability");

        // Limit to max_agents strategies
        let active_strategies: Vec<_> = strategies.iter().take(max_agents).collect();

        let strategy_outputs: Vec<Value> = active_strategies
            .iter()
            .map(|s| {
                json!({
                    "name": s.name(),
                    "prompt": s.generate_prompt(task),
                    "description": s.description()
                })
            })
            .collect();

        let strategy_count = strategy_outputs.len();

        tracing::debug!(
            strategy_count,
            max_agents,
            "multi_strategy: generated strategy prompts"
        );

        ToolResult::success(format!(
            "Generated {strategy_count} strategy variations for parallel execution."
        ))
        .with_metadata("task", json!(task))
        .with_metadata("strategies", json!(strategy_outputs))
        .with_metadata("evaluation_criteria", json!(evaluation_criteria))
        .with_metadata(
            "recommendation",
            json!("Run strategies in parallel via spawn_parallel_agents, then use best_of_n to select the best result"),
        )
    }
}
