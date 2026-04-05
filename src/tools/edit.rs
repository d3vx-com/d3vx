//! Edit Tool
//!
//! Perform exact string replacements in a file.

use async_trait::async_trait;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::file_tracker::{FileReadTracker, StaleStatus};
use super::text_match;
use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Edit tool for performing string replacements in files
pub struct EditTool {
    definition: ToolDefinition,
    tracker: Arc<FileReadTracker>,
}

impl EditTool {
    pub fn new() -> Self {
        Self::with_tracker(Arc::new(FileReadTracker::new()))
    }

    pub fn with_tracker(tracker: Arc<FileReadTracker>) -> Self {
        Self {
            tracker,
            definition: ToolDefinition {
                name: "Edit".to_string(),
                description: concat!(
                    "Perform exact string replacements in a file. ",
                    "Replaces the first occurrence of old_string with new_string. ",
                    "The old_string must match exactly (including whitespace/indentation). ",
                    "Use for targeted edits instead of rewriting entire files."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "The absolute path to the file to edit"
                        },
                        "old_string": {
                            "type": "string",
                            "description": "The exact text to replace"
                        },
                        "new_string": {
                            "type": "string",
                            "description": "The text to replace it with"
                        },
                        "replace_all": {
                            "type": "boolean",
                            "description": "Replace all occurrences (default: false)",
                            "default": false
                        }
                    },
                    "required": ["file_path", "old_string", "new_string"]
                }),
            },
        }
    }
}

impl Default for EditTool {
    fn default() -> Self {
        Self::with_tracker(Arc::new(FileReadTracker::new()))
    }
}

#[async_trait]
impl Tool for EditTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let file_path = input["file_path"].as_str().unwrap_or("");
        let old_string = input["old_string"].as_str().unwrap_or("");
        let new_string = input["new_string"].as_str().unwrap_or("");
        let replace_all = input["replace_all"].as_bool().unwrap_or(false);

        if file_path.is_empty() {
            return ToolResult::error("file_path is required");
        }
        if old_string.is_empty() {
            return ToolResult::error("old_string is required");
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

        // Check if file was modified since last read
        match self.tracker.is_stale(&path) {
            StaleStatus::Stale { .. } => {
                return ToolResult::error(
                    "File has been unexpectedly modified. Read it again before attempting to write it."
                );
            }
            StaleStatus::NeverRead => {
                return ToolResult::error(
                    "File must be read before editing. Use the Read tool first.",
                );
            }
            StaleStatus::Fresh => {} // OK to proceed
        }

        // Read the file
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read file: {}", e)),
        };

        // Try approximate matching cascade: Exact → Normalized → LineAnchor → Similarity
        let mat = match text_match::find_match(&content, old_string) {
            Some(m) => m,
            None => {
                let hint = text_match::find_nearest(&content, old_string)
                    .map(|s| format!("\n\nNearest match:\n{}", &s[..s.len().min(500)]))
                    .unwrap_or_default();
                return ToolResult::error(format!(
                    "old_string not found in file (tried exact, normalized, line-anchor, and similarity matching).{hint}"
                ));
            }
        };

        if replace_all && mat.strategy != text_match::Strategy::Exact {
            // replace_all only works with exact matches to avoid replacing wrong regions
            return ToolResult::error(
                "replace_all requires an exact match. The old_string was found via approximate matching — please provide the exact string or remove replace_all.".to_string(),
            );
        }

        // Perform replacement using matched byte offsets
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            let mut new =
                String::with_capacity(content.len() - (mat.end - mat.start) + new_string.len());
            new.push_str(&content[..mat.start]);
            new.push_str(new_string);
            new.push_str(&content[mat.end..]);
            new
        };

        // Write the file
        match fs::write(&path, &new_content) {
            Ok(_) => {
                let count = if replace_all {
                    content.matches(old_string).count()
                } else {
                    1
                };
                let mut result = ToolResult::success(format!(
                    "Successfully replaced {} occurrence(s) in {}",
                    count,
                    path.display()
                ))
                .with_metadata("replacements", serde_json::json!(count));
                if mat.strategy != text_match::Strategy::Exact {
                    result = result
                        .with_metadata("matchStrategy", serde_json::json!(mat.strategy.label()));
                }
                result
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
    async fn test_edit_file() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("d3vx_test_edit.txt");
        fs::write(&temp_file, "Hello, World!\nGoodbye, World!").unwrap();

        let tracker = Arc::new(FileReadTracker::new());
        tracker.record_read(&temp_file, "Hello, World!\nGoodbye, World!");

        let tool = EditTool::with_tracker(tracker);
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": temp_file.to_string_lossy().to_string(),
                    "old_string": "Hello, World!",
                    "new_string": "Hi, Rust!"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);

        let content = fs::read_to_string(&temp_file).unwrap();
        assert!(content.contains("Hi, Rust!"));
        assert!(!content.contains("Hello, World!"));

        // Cleanup
        fs::remove_file(&temp_file).ok();
    }

    #[tokio::test]
    async fn test_edit_not_found() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("d3vx_test_edit_notfound.txt");
        fs::write(&temp_file, "Some content").unwrap();

        let tracker = Arc::new(FileReadTracker::new());
        tracker.record_read(&temp_file, "Some content");

        let tool = EditTool::with_tracker(tracker);
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": temp_file.to_string_lossy().to_string(),
                    "old_string": "Not in file",
                    "new_string": "Replacement"
                }),
                &context,
            )
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("not found"));

        // Cleanup
        fs::remove_file(&temp_file).ok();
    }

    #[tokio::test]
    async fn test_edit_never_read_rejected() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("d3vx_test_edit_never_read.txt");
        fs::write(&temp_file, "Hello, World!").unwrap();

        let tool = EditTool::new();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": temp_file.to_string_lossy().to_string(),
                    "old_string": "Hello, World!",
                    "new_string": "Hi, Rust!"
                }),
                &context,
            )
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("Read tool first"));

        // Cleanup
        fs::remove_file(&temp_file).ok();
    }

    #[tokio::test]
    async fn test_edit_stale_rejected() {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("d3vx_test_edit_stale.txt");
        fs::write(&temp_file, "Original content").unwrap();

        let tracker = Arc::new(FileReadTracker::new());
        tracker.record_read(&temp_file, "Original content");

        // Modify file after recording read
        fs::write(&temp_file, "Modified content").unwrap();

        let tool = EditTool::with_tracker(tracker);
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "file_path": temp_file.to_string_lossy().to_string(),
                    "old_string": "Modified content",
                    "new_string": "Replacement"
                }),
                &context,
            )
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("unexpectedly modified"));

        // Cleanup
        fs::remove_file(&temp_file).ok();
    }
}
