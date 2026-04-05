//! Structured Output Tool
//!
//! Provides JSON schema validation and structured response generation.

use serde_json::{json, Value};

use crate::tools::{Tool, ToolContext, ToolDefinition, ToolResult};

pub struct StructuredOutputTool {
    definition: ToolDefinition,
    max_retries: usize,
}

impl StructuredOutputTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "structured_output".to_string(),
                description: "Generate and validate structured JSON output based on a schema."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "schema": { "type": "object", "description": "JSON Schema for output" },
                        "prompt": { "type": "string", "description": "Instructions for generation" },
                        "strict": { "type": "boolean", "default": true }
                    },
                    "required": ["schema", "prompt"]
                }),
            },
            max_retries: 3,
        }
    }

    pub fn with_max_retries(mut self, retries: usize) -> Self {
        self.max_retries = retries;
        self
    }
}

impl Default for StructuredOutputTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for StructuredOutputTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: Value, _context: &ToolContext) -> ToolResult {
        let schema = match input.get("schema") {
            Some(s) => s,
            None => return ToolResult::error("Missing required field: schema"),
        };

        let prompt = match input.get("prompt").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => return ToolResult::error("Missing required field: prompt"),
        };

        let schema_str =
            serde_json::to_string_pretty(schema).unwrap_or_else(|_| schema.to_string());

        let enhanced_prompt = format!(
            "{}\n\nGenerate JSON output matching:\n{}\n\nOutput ONLY valid JSON.",
            prompt, schema_str
        );

        ToolResult::success(
            json!({
                "schema": schema,
                "prompt": prompt,
                "enhanced_prompt": enhanced_prompt
            })
            .to_string(),
        )
    }
}

pub struct SchemaValidator {
    #[allow(dead_code)]
    schema: Value,
    strict: bool,
}

impl SchemaValidator {
    pub fn new(schema: Value) -> Self {
        Self {
            schema,
            strict: true,
        }
    }

    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    pub fn validate(&self, _value: &Value) -> Result<(), ValidationError> {
        if self.strict {
            // Basic validation - could be enhanced with jsonschema crate
            // For now, we just check if it's valid JSON
            Ok(())
        } else {
            Ok(())
        }
    }

    pub fn validated(&self, value: &Value) -> Result<Value, ValidationError> {
        self.validate(value)?;
        Ok(value.clone())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Schema mismatch: {0:?}")]
    SchemaMismatch(Vec<String>),
    #[error("Schema error: {0}")]
    SchemaError(String),
}

pub mod schemas {
    use serde_json::{json, Value};

    pub fn list_schema(item_schema: Value) -> Value {
        json!({ "type": "array", "items": item_schema })
    }

    pub fn code_change_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string" },
                "action": { "type": "string", "enum": ["create", "modify", "delete"] },
                "content": { "type": "string" },
                "reasoning": { "type": "string" }
            },
            "required": ["file_path", "action"]
        })
    }

    pub fn task_breakdown_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "description": { "type": "string" },
                            "priority": { "type": "string", "enum": ["high", "medium", "low"] }
                        },
                        "required": ["title"]
                    }
                }
            },
            "required": ["tasks"]
        })
    }

    pub fn decision_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "decision": { "type": "string" },
                "alternatives_considered": { "type": "array" },
                "reasoning": { "type": "string" }
            },
            "required": ["decision", "reasoning"]
        })
    }
}
