//! MultiEdit Tool
//!
//! Apply multiple find-and-replace edits to a single file atomically.

use async_trait::async_trait;
use std::fs;
use std::path::Path;

use super::text_match;
use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// MultiEdit tool for atomic multi-section file editing
pub struct MultiEditTool {
    definition: ToolDefinition,
}

impl MultiEditTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "MultiEdit".to_string(),
                description: concat!(
                    "Apply multiple find-and-replace edits to a single file atomically. ",
                    "All edits are validated before any are applied. ",
                    "More efficient than calling EditTool multiple times. ",
                    "Use this when you need to make multiple non-contiguous changes to a file."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Relative or absolute path to the file to edit"
                        },
                        "edits": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "old_string": {
                                        "type": "string",
                                        "description": "The exact text to find and replace"
                                    },
                                    "new_string": {
                                        "type": "string",
                                        "description": "The replacement text"
                                    }
                                },
                                "required": ["old_string", "new_string"]
                            },
                            "minItems": 1,
                            "description": "List of find-and-replace operations to apply atomically"
                        }
                    },
                    "required": ["file_path", "edits"]
                }),
            },
        }
    }
}

impl Default for MultiEditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for MultiEditTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let file_path = input["file_path"].as_str().unwrap_or("");
        let edits = match input["edits"].as_array() {
            Some(arr) if !arr.is_empty() => arr,
            Some(_) => return ToolResult::error("edits array must not be empty"),
            None => return ToolResult::error("edits is required"),
        };

        if file_path.is_empty() {
            return ToolResult::error("file_path is required");
        }

        // Resolve path relative to cwd
        let path = if Path::new(file_path).is_absolute() {
            Path::new(file_path).to_path_buf()
        } else {
            Path::new(&context.cwd).join(file_path)
        };

        // Check file exists
        if !path.exists() {
            return ToolResult::error(format!("File not found: {}", file_path));
        }

        // Read the file
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
        };

        // Parse and validate all edits using approximate matching
        let mut parsed_edits: Vec<(text_match::Match, &str)> = Vec::new();
        let mut failed: Vec<String> = Vec::new();

        for (i, edit) in edits.iter().enumerate() {
            let old_string = edit["old_string"].as_str().unwrap_or("");
            let new_string = edit["new_string"].as_str().unwrap_or("");

            if old_string.is_empty() {
                failed.push(format!("Edit {}: old_string is required", i + 1));
                continue;
            }

            match text_match::find_match(&content, old_string) {
                Some(m) => parsed_edits.push((m, new_string)),
                None => {
                    let hint = text_match::find_nearest(&content, old_string)
                        .map(|s| format!(" (nearest: {}...)", &s[..s.len().min(80)]))
                        .unwrap_or_default();
                    failed.push(format!(
                        "Edit {}: old_string not found in file{hint}",
                        i + 1
                    ));
                }
            }
        }

        // If any validation failed, return error without applying any edits
        if !failed.is_empty() {
            return ToolResult::error(format!(
                "Validation failed - no edits applied:\n{}",
                failed.join("\n")
            ));
        }

        // Sort matches by start offset descending so replacements don't shift later offsets
        parsed_edits.sort_by(|a, b| b.0.start.cmp(&a.0.start));

        // Apply all edits back-to-front using byte offsets
        let mut new_content = content;
        let mut applied: Vec<String> = Vec::new();

        for (i, (mat, new_string)) in parsed_edits.iter().enumerate() {
            let before_len = new_content.len();
            new_content = format!(
                "{}{}{}",
                &new_content[..mat.start],
                new_string,
                &new_content[mat.end..]
            );
            applied.push(format!(
                "Edit {}: replaced {} chars -> {} chars (via {})",
                i + 1,
                mat.end - mat.start,
                new_string.len(),
                mat.strategy.label(),
            ));
        }

        // Write the file
        match fs::write(&path, &new_content) {
            Ok(_) => ToolResult::success(format!(
                "Applied {} edits to {}:\n{}",
                applied.len(),
                path.display(),
                applied.join("\n")
            ))
            .with_metadata(
                "filesChanged",
                serde_json::json!([path.to_string_lossy().to_string()]),
            )
            .with_metadata("editsApplied", serde_json::json!(applied.len())),
            Err(e) => ToolResult::error(format!("Failed to write file: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_multi_edit_basic() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("d3vx_test_multi_edit.txt");
        fs::write(&temp_file, "Hello, World!\nGoodbye, World!").unwrap();

        let tool = MultiEditTool::new();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": temp_file.to_string_lossy().to_string(),
                    "edits": [
                        {"old_string": "Hello", "new_string": "Hi"},
                        {"old_string": "Goodbye", "new_string": "Bye"}
                    ]
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Applied 2 edits"));

        let content = fs::read_to_string(&temp_file).unwrap();
        assert!(content.contains("Hi, World!"));
        assert!(content.contains("Bye, World!"));

        // Cleanup
        fs::remove_file(&temp_file).ok();
    }

    #[tokio::test]
    async fn test_multi_edit_partial_failure() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("d3vx_test_multi_edit_fail.txt");
        fs::write(&temp_file, "Hello, World!").unwrap();

        let tool = MultiEditTool::new();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": temp_file.to_string_lossy().to_string(),
                    "edits": [
                        {"old_string": "Hello", "new_string": "Hi"},
                        {"old_string": "NonExistent", "new_string": "Bye"}
                    ]
                }),
                &context,
            )
            .await;

        // Should fail because one edit couldn't be found
        assert!(result.is_error);
        assert!(result.content.contains("Validation failed"));

        // File should be unchanged
        let content = fs::read_to_string(&temp_file).unwrap();
        assert!(content.contains("Hello, World!"));

        // Cleanup
        fs::remove_file(&temp_file).ok();
    }

    #[tokio::test]
    async fn test_multi_edit_file_not_found() {
        let tool = MultiEditTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": "/nonexistent/file.txt",
                    "edits": [{"old_string": "a", "new_string": "b"}]
                }),
                &context,
            )
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn test_multi_edit_empty_edits() {
        let tool = MultiEditTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": "/some/file.txt",
                    "edits": []
                }),
                &context,
            )
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("empty"));
    }

    #[tokio::test]
    async fn test_multi_edit_missing_required() {
        let tool = MultiEditTool::new();
        let context = ToolContext::default();

        // Missing file_path
        let result = tool
            .execute(
                serde_json::json!({
                    "edits": [{"old_string": "a", "new_string": "b"}]
                }),
                &context,
            )
            .await;
        assert!(result.is_error);

        // Missing edits
        let result = tool
            .execute(serde_json::json!({"file_path": "/some/file.txt"}), &context)
            .await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_multi_edit_metadata() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("d3vx_test_multi_edit_meta.txt");
        fs::write(&temp_file, "Hello").unwrap();

        let tool = MultiEditTool::new();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": temp_file.to_string_lossy().to_string(),
                    "edits": [{"old_string": "Hello", "new_string": "Hi"}]
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert_eq!(
            result.metadata.get("editsApplied"),
            Some(&serde_json::json!(1))
        );

        // Cleanup
        fs::remove_file(&temp_file).ok();
    }
}
