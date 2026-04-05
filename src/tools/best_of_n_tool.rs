//! Best-of-N Tool

use serde_json::{json, Value};

use crate::agent::{BestOfNConfig, StepControl};
use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};

pub struct BestOfNTool {
    definition: ToolDefinition,
    config: BestOfNConfig,
}

impl BestOfNTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "best_of_n".to_string(),
                description:
                    "Request a loop-native best-of-N execution step that generates multiple variants and selects the strongest result."
                        .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "prompt": { "type": "string" },
                        "n": { "type": "integer", "default": 3, "minimum": 2, "maximum": 10 },
                        "strip_reasoning": { "type": "boolean", "default": true },
                        "selector_prompt": { "type": "string" }
                    },
                    "required": ["prompt"]
                }),
            },
            config: BestOfNConfig::default(),
        }
    }

    pub fn with_config(mut self, config: BestOfNConfig) -> Self {
        self.config = config;
        self
    }
}

impl Default for BestOfNTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for BestOfNTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: Value, _context: &ToolContext) -> ToolResult {
        let prompt = match input.get("prompt").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => return ToolResult::error("Missing required field: prompt"),
        };

        let n = input
            .get("n")
            .and_then(|n| n.as_u64())
            .map(|n| n as usize)
            .unwrap_or(self.config.n);

        let mut config = self.config.clone();
        config.n = n;

        if let Some(strip) = input.get("strip_reasoning").and_then(|s| s.as_bool()) {
            config.strip_reasoning = strip;
        }
        let selector_prompt = input
            .get("selector_prompt")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let step_control = StepControl::GenerateN {
            n: config.n,
            prompt: Some(prompt.to_string()),
            selector_prompt: selector_prompt.clone(),
        };

        ToolResult::success(format!(
            "Queued best-of-{} execution for the current agent loop.",
            config.n
        ))
        .with_metadata(
            "step_control",
            serde_json::to_value(step_control).unwrap_or_else(|_| json!({})),
        )
        .with_metadata("requested_by_tool", json!("best_of_n"))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BestOfNToolError {
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
}
