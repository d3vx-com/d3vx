//! Write Tool
//!
//! Write content to a file, creating it if it doesn't exist.

use async_trait::async_trait;
use std::fs;
use std::path::Path;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Write tool for creating or overwriting files
pub struct WriteTool {
    definition: ToolDefinition,
}

impl WriteTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "Write".to_string(),
                description: concat!(
                    "Write content to a file. ",
                    "Creates the file if it doesn't exist, overwrites if it does. ",
                    "Creates parent directories if needed. ",
                    "Use for creating new files or completely replacing file contents."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "The absolute path to the file to write"
                        },
                        "content": {
                            "type": "string",
                            "description": "The content to write to the file"
                        }
                    },
                    "required": ["file_path", "content"]
                }),
            },
        }
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let file_path = input["file_path"].as_str().unwrap_or("");
        let content = input["content"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult::error("file_path is required");
        }

        // Resolve path relative to cwd
        let path = if Path::new(file_path).is_absolute() {
            Path::new(file_path).to_path_buf()
        } else {
            Path::new(&context.cwd).join(file_path)
        };

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                match fs::create_dir_all(parent) {
                    Ok(_) => {}
                    Err(e) => {
                        return ToolResult::error(format!("Failed to create directories: {}", e))
                    }
                }
            }
        }

        // Write the file
        match fs::write(&path, content) {
            Ok(_) => {
                let bytes_written = content.len();
                ToolResult::success(format!(
                    "Successfully wrote {} bytes to {}",
                    bytes_written,
                    path.display()
                ))
                .with_metadata("bytes_written", serde_json::json!(bytes_written))
                .with_metadata(
                    "path",
                    serde_json::json!(path.to_string_lossy().to_string()),
                )
            }
            Err(e) => ToolResult::error(format!("Failed to write file: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_write_file() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("d3vx_test_write.txt");

        let tool = WriteTool::new();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": temp_file.to_string_lossy().to_string(),
                    "content": "Hello, World!\nLine 2\n"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);

        // Verify file was written
        let content = fs::read_to_string(&temp_file).unwrap();
        assert!(content.contains("Hello, World!"));

        // Cleanup
        fs::remove_file(&temp_file).ok();
    }

    #[tokio::test]
    async fn test_write_creates_directories() {
        let temp_dir = std::env::temp_dir();
        let nested_dir = temp_dir.join("d3vx_nested").join("deep").join("path");
        let temp_file = nested_dir.join("test.txt");

        let tool = WriteTool::new();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": temp_file.to_string_lossy().to_string(),
                    "content": "Nested content"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(temp_file.exists());

        // Cleanup
        fs::remove_dir_all(temp_dir.join("d3vx_nested")).ok();
    }
}
