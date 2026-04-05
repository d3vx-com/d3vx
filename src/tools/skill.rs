//! Skill Tool

use serde_json::{json, Value};
use std::sync::Arc;

use crate::skills::{SkillContext, SkillExecutor, SkillRegistry};
use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};

pub struct SkillTool {
    definition: ToolDefinition,
    registry: Option<Arc<SkillRegistry>>,
}

impl SkillTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "skill".to_string(),
                description: "Load and execute a skill.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "args": { "type": "object", "additionalProperties": true }
                    },
                    "required": ["name"]
                }),
            },
            registry: None,
        }
    }

    pub fn with_registry(mut self, registry: Arc<SkillRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    pub async fn execute_skill(
        &self,
        name: &str,
        args: &Value,
        context: &ToolContext,
    ) -> Result<String, SkillToolError> {
        let registry = self
            .registry
            .as_ref()
            .ok_or_else(|| SkillToolError::RegistryNotSet)?;

        let skill = registry
            .get(name)
            .await
            .ok_or_else(|| SkillToolError::SkillNotFound(name.to_string()))?;

        let mut skill_context = SkillContext::new(
            args.get("prompt").and_then(|p| p.as_str()).unwrap_or(""),
            &context.cwd,
        );

        if let Some(obj) = args.as_object() {
            for (key, value) in obj {
                if let Some(s) = value.as_str() {
                    skill_context.set_var(key, s);
                } else {
                    skill_context.set_var(key, value.to_string());
                }
            }
        }

        let executor = SkillExecutor::new();
        let prepared = executor.prepare_execution(&skill, &skill_context);
        Ok(prepared.system_prompt)
    }

    pub async fn list_skills(&self) -> Result<Vec<String>, SkillToolError> {
        let registry = self
            .registry
            .as_ref()
            .ok_or_else(|| SkillToolError::RegistryNotSet)?;
        let skills = registry.list().await;
        Ok(skills.into_iter().map(|s| s.name).collect())
    }

    pub async fn find_skills(&self, input: &str) -> Result<Vec<String>, SkillToolError> {
        let registry = self
            .registry
            .as_ref()
            .ok_or_else(|| SkillToolError::RegistryNotSet)?;
        let skills = registry.find_by_trigger(input).await;
        Ok(skills.into_iter().map(|s| s.name.clone()).collect())
    }
}

impl Default for SkillTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for SkillTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: Value, context: &ToolContext) -> ToolResult {
        let name = match input.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => return ToolResult::error("Missing required field: name"),
        };

        match self.execute_skill(name, &input, context).await {
            Ok(content) => ToolResult::success(content),
            Err(e) => ToolResult::error(e.to_string()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SkillToolError {
    #[error("Skill registry not set")]
    RegistryNotSet,
    #[error("Skill not found: {0}")]
    SkillNotFound(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
}

#[derive(Debug, Clone, Default)]
pub struct SkillToolConfig {
    pub search_paths: Vec<String>,
    pub auto_load: bool,
}

impl SkillToolConfig {
    pub fn new() -> Self {
        Self {
            search_paths: vec!["~/.d3vx/skills".to_string(), ".d3vx/skills".to_string()],
            auto_load: true,
        }
    }
}
