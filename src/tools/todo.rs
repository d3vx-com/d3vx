//! TodoWrite Tool
//!
//! Manage a markdown todo list.

use async_trait::async_trait;
use std::fs;
use std::path::Path;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Status markers for todo items
const STATUS_TODO: &str = "[ ]";
const STATUS_IN_PROGRESS: &str = "[/]";
const STATUS_DONE: &str = "[x]";

/// TodoWrite tool for managing todo lists
pub struct TodoWriteTool {
    definition: ToolDefinition,
}

impl TodoWriteTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "TodoWrite".to_string(),
                description: concat!(
                    "Write or update a markdown todo list. ",
                    "Creates the file if it does not exist. ",
                    "Use this to track progress on multi-step tasks."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "todos": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": {
                                        "type": "string",
                                        "description": "The todo item text"
                                    },
                                    "status": {
                                        "type": "string",
                                        "enum": ["todo", "in_progress", "done"],
                                        "default": "todo",
                                        "description": "Status of the todo item"
                                    },
                                    "activeForm": {
                                        "type": "string",
                                        "description": "Present tense form of the task (e.g., 'Writing tests')"
                                    }
                                },
                                "required": ["content"]
                            },
                            "minItems": 1,
                            "description": "Todo items to write"
                        },
                        "file_path": {
                            "type": "string",
                            "default": ".d3vx/todo.md",
                            "description": "Path to todo file"
                        }
                    },
                    "required": ["todos"]
                }),
            },
        }
    }
}

impl Default for TodoWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the status marker for a given status string
fn get_status_marker(status: &str) -> &'static str {
    match status {
        "in_progress" => STATUS_IN_PROGRESS,
        "done" => STATUS_DONE,
        _ => STATUS_TODO,
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let todos = match input["todos"].as_array() {
            Some(arr) if !arr.is_empty() => arr,
            Some(_) => return ToolResult::error("todos array must not be empty"),
            None => return ToolResult::error("todos is required"),
        };

        let file_path = input["file_path"].as_str().unwrap_or(".d3vx/todo.md");

        // Resolve path relative to cwd
        let path = if Path::new(file_path).is_absolute() {
            Path::new(file_path).to_path_buf()
        } else {
            Path::new(&context.cwd).join(file_path)
        };

        // Read existing content if file exists
        let existing = if path.exists() {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        // Generate the todo lines
        let mut todo_lines: Vec<String> = Vec::new();
        let mut counts = Counters::default();

        for todo in todos {
            let content = todo["content"].as_str().unwrap_or("");
            if content.is_empty() {
                continue;
            }

            let status = todo["status"].as_str().unwrap_or("todo");
            let marker = get_status_marker(status);

            // Track counts
            match status {
                "in_progress" => counts.in_progress += 1,
                "done" => counts.done += 1,
                _ => counts.todo += 1,
            }

            // Format the line
            if let Some(active_form) = todo["activeForm"].as_str() {
                if status == "in_progress" {
                    todo_lines.push(format!("- {} {} ({})", marker, content, active_form));
                } else {
                    todo_lines.push(format!("- {} {}", marker, content));
                }
            } else {
                todo_lines.push(format!("- {} {}", marker, content));
            }
        }

        // Build the final content
        let content = if existing.trim().is_empty() {
            // Create fresh file with header
            format!("# Todo\n\n{}\n", todo_lines.join("\n"))
        } else {
            // Append to existing
            format!("{}\n{}\n", existing.trim_end(), todo_lines.join("\n"))
        };

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return ToolResult::error(format!("Failed to create directory: {}", e));
            }
        }

        // Write the file
        match fs::write(&path, &content) {
            Ok(_) => {
                let total = counts.todo + counts.in_progress + counts.done;
                ToolResult::success(format!(
                    "Updated {}: {} item(s) - {} todo, {} in progress, {} done",
                    path.display(),
                    total,
                    counts.todo,
                    counts.in_progress,
                    counts.done
                ))
                .with_metadata(
                    "filesChanged",
                    serde_json::json!([path.to_string_lossy().to_string()]),
                )
            }
            Err(e) => ToolResult::error(format!("Failed to write todo: {}", e)),
        }
    }
}

/// Counters for todo item statuses
#[derive(Default)]
struct Counters {
    todo: usize,
    in_progress: usize,
    done: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_todo_basic() {
        let tool = TodoWriteTool::new();
        let temp_dir = std::env::temp_dir();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "todos": [
                        {"content": "Write tests"},
                        {"content": "Fix bugs", "status": "in_progress"},
                        {"content": "Deploy", "status": "done"}
                    ]
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("3 item(s)"));
        assert!(result.content.contains("1 todo"));
        assert!(result.content.contains("1 in progress"));
        assert!(result.content.contains("1 done"));
    }

    #[tokio::test]
    async fn test_todo_with_active_form() {
        let tool = TodoWriteTool::new();
        let temp_dir = std::env::temp_dir();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "todos": [
                        {"content": "Write tests", "status": "in_progress", "activeForm": "Writing tests"}
                    ]
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_todo_empty_array() {
        let tool = TodoWriteTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(serde_json::json!({"todos": []}), &context)
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("empty"));
    }

    #[tokio::test]
    async fn test_todo_missing_todos() {
        let tool = TodoWriteTool::new();
        let context = ToolContext::default();

        let result = tool.execute(serde_json::json!({}), &context).await;

        assert!(result.is_error);
        assert!(result.content.contains("required"));
    }

    #[tokio::test]
    async fn test_todo_creates_directory() {
        let tool = TodoWriteTool::new();
        let temp_dir = std::env::temp_dir();
        let unique_dir = temp_dir.join(format!("d3vx_test_todo_{}", uuid::Uuid::new_v4()));
        let context = ToolContext {
            cwd: unique_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "todos": [{"content": "Test item"}],
                    "file_path": ".d3vx/nested/todo.md"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);

        // Verify file was created
        let todo_path = unique_dir.join(".d3vx/nested/todo.md");
        assert!(todo_path.exists());

        // Cleanup
        fs::remove_dir_all(&unique_dir).ok();
    }
}
