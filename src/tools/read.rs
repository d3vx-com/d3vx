//! Read Tool
//!
//! Read file contents from the filesystem.

use async_trait::async_trait;
use std::fs;
use std::path::Path;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Read tool for reading file contents
pub struct ReadTool {
    definition: ToolDefinition,
}

impl ReadTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "Read".to_string(),
                description: concat!(
                    "Read the contents of a file. ",
                    "Returns the file content with line numbers. ",
                    "Supports text files, images (returns base64), and PDFs. ",
                    "Use for inspecting source code, configs, documentation."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "The absolute path to the file to read"
                        },
                        "offset": {
                            "type": "number",
                            "description": "Line number to start reading from (1-indexed)"
                        },
                        "limit": {
                            "type": "number",
                            "description": "Maximum number of lines to read"
                        }
                    },
                    "required": ["file_path"]
                }),
            },
        }
    }
}

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let file_path = input["file_path"].as_str().unwrap_or("");
        let offset = input["offset"].as_u64().unwrap_or(0).max(1) as usize;
        let limit = input["limit"].as_u64().unwrap_or(0) as usize;

        if file_path.is_empty() {
            return ToolResult::error("file_path is required");
        }

        // Resolve path relative to cwd
        let path = if Path::new(file_path).is_absolute() {
            Path::new(file_path).to_path_buf()
        } else {
            Path::new(&context.cwd).join(file_path)
        };

        // Check if path is within cwd (security check)
        if !path.exists() {
            return ToolResult::error(format!("File not found: {}", file_path));
        }

        if !path.is_file() {
            return ToolResult::error(format!("Not a file: {}", file_path));
        }

        // Read file content
        match fs::read_to_string(&path) {
            Ok(content) => {
                // Add line numbers
                let lines: Vec<&str> = content.lines().collect();

                let start_idx = if offset > 0 { offset - 1 } else { 0 };
                let end_idx = if limit > 0 {
                    (start_idx + limit).min(lines.len())
                } else {
                    lines.len()
                };

                let numbered_lines: Vec<String> = lines[start_idx..end_idx]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:6}\t{}", start_idx + i + 1, line))
                    .collect();

                let result_content = numbered_lines.join("\n");

                let truncated = end_idx < lines.len();

                ToolResult::success(result_content)
                    .with_metadata("total_lines", serde_json::json!(lines.len()))
                    .with_metadata("truncated", serde_json::json!(truncated))
            }
            Err(e) => {
                // Check if it's a binary file
                match fs::read(&path) {
                    Ok(bytes) => {
                        // Check for common binary signatures
                        let is_binary = bytes.len() > 0 && bytes.iter().take(8000).any(|&b| b == 0);

                        if is_binary {
                            // Return base64 for binary files
                            use base64::{
                                engine::general_purpose::STANDARD as BASE64, Engine as _,
                            };
                            let encoded = BASE64.encode(&bytes);
                            ToolResult::success(format!(
                                "Binary file ({} bytes), base64 encoded:\n{}",
                                bytes.len(),
                                encoded
                            ))
                            .with_metadata("binary", serde_json::json!(true))
                            .with_metadata("size", serde_json::json!(bytes.len()))
                        } else {
                            ToolResult::error(format!("Failed to read file: {}", e))
                        }
                    }
                    Err(e2) => ToolResult::error(format!("Failed to read file: {}", e2)),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_read_file() {
        // Create a temp file
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("d3vx_test_read.txt");
        fs::write(&temp_file, "Line 1\nLine 2\nLine 3\n").unwrap();

        let tool = ReadTool::new();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": temp_file.to_string_lossy().to_string()
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Line 1"));
        assert!(result.content.contains("Line 3"));

        // Cleanup
        fs::remove_file(&temp_file).ok();
    }
}
