//! Question Tool
//!
//! Ask the user for clarification before proceeding.

use async_trait::async_trait;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Question tool for asking clarifying questions
pub struct QuestionTool {
    definition: ToolDefinition,
}

impl QuestionTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "Question".to_string(),
                description: concat!(
                    "Ask the user a clarifying question before proceeding. ",
                    "Use this when you are uncertain about the user's intent instead of guessing. ",
                    "The response will come back as the user's next message."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "question": {
                            "type": "string",
                            "description": "The clarifying question to ask the user."
                        },
                        "options": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional multiple-choice options for the user."
                        },
                        "default_answer": {
                            "type": "string",
                            "description": "A suggested default answer if the user just presses Enter."
                        }
                    },
                    "required": ["question"]
                }),
            },
        }
    }
}

impl Default for QuestionTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for QuestionTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, _context: &ToolContext) -> ToolResult {
        let question = input["question"].as_str().unwrap_or("");
        let options = input["options"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>());
        let default_answer = input["default_answer"].as_str();

        if question.is_empty() {
            return ToolResult::error("question is required");
        }

        let mut formatted = format!("Question for you:\n\n{}", question);

        if let Some(ref opts) = options {
            if !opts.is_empty() {
                formatted.push_str("\n\nOptions:\n");
                for (i, opt) in opts.iter().enumerate() {
                    formatted.push_str(&format!("  {}. {}\n", i + 1, opt));
                }
            }
        }

        if let Some(default) = default_answer {
            formatted.push_str(&format!("\n(Default: {})", default));
        }

        ToolResult::success(formatted).with_metadata("requiresUserInput", serde_json::json!(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_question_basic() {
        let tool = QuestionTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "question": "Which framework should I use?"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Which framework"));
        assert!(result.content.contains("Question for you"));
    }

    #[tokio::test]
    async fn test_question_with_options() {
        let tool = QuestionTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "question": "Choose an option:",
                    "options": ["React", "Vue", "Svelte"]
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Options:"));
        assert!(result.content.contains("1. React"));
        assert!(result.content.contains("2. Vue"));
        assert!(result.content.contains("3. Svelte"));
    }

    #[tokio::test]
    async fn test_question_with_default() {
        let tool = QuestionTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "question": "Continue?",
                    "default_answer": "yes"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Default: yes"));
    }

    #[tokio::test]
    async fn test_question_empty() {
        let tool = QuestionTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(serde_json::json!({"question": ""}), &context)
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("required"));
    }

    #[tokio::test]
    async fn test_question_metadata() {
        let tool = QuestionTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "question": "Test?"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert_eq!(
            result.metadata.get("requiresUserInput"),
            Some(&serde_json::json!(true))
        );
    }
}
