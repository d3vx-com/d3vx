//! Draft Change Tool
//!
//! Generates unified diffs and stores them in a draft file instead of modifying the filesystem.

use async_trait::async_trait;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};
use crate::utils::diff::generate_unified_diff;

/// Tool for drafting changes as unified diffs
pub struct DraftChangeTool {
    definition: ToolDefinition,
}

impl DraftChangeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "draft_change".to_string(),
                description: "Propose a code change as a unified diff. Stored in .d3vx/draft-{task_id}.patch. Does NOT modify the original file.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "The absolute path to the file to draft changes for"
                        },
                        "old_string": {
                            "type": "string",
                            "description": "The exact text to replace"
                        },
                        "new_string": {
                            "type": "string",
                            "description": "The text to replace it with"
                        },
                        "task_id": {
                            "type": "string",
                            "description": "The current task ID (used for naming the patch file)"
                        }
                    },
                    "required": ["file_path", "old_string", "new_string", "task_id"]
                }),
            },
        }
    }

    fn get_patch_path(&self, cwd: &str, task_id: &str) -> PathBuf {
        Path::new(cwd)
            .join(".d3vx")
            .join(format!("draft-{}.patch", task_id))
    }
}

impl Default for DraftChangeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DraftChangeTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: Value, context: &ToolContext) -> ToolResult {
        let file_path = input["file_path"].as_str().unwrap_or("");
        let old_string = input["old_string"].as_str().unwrap_or("");
        let new_string = input["new_string"].as_str().unwrap_or("");
        let task_id = input["task_id"].as_str().unwrap_or("unknown");

        if file_path.is_empty() || old_string.is_empty() || task_id == "unknown" {
            return ToolResult::error("file_path, old_string, and task_id are required");
        }

        // Resolve target file path
        let target_path = if Path::new(file_path).is_absolute() {
            PathBuf::from(file_path)
        } else {
            Path::new(&context.cwd).join(file_path)
        };

        if !target_path.exists() {
            return ToolResult::error(format!("Target file not found: {}", file_path));
        }

        // Read target file content
        let original_content = match fs::read_to_string(&target_path) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read target file: {}", e)),
        };

        if !original_content.contains(old_string) {
            return ToolResult::error("old_string not found in target file. Ensure exact match.");
        }

        // Generate the new content (locally, for diffing)
        let modified_content = original_content.replace(old_string, new_string);

        // Generate unified diff
        let diff = generate_unified_diff(file_path, &original_content, &modified_content);

        // Ensure .d3vx directory exists
        let d3vx_dir = Path::new(&context.cwd).join(".d3vx");
        if let Err(e) = fs::create_dir_all(&d3vx_dir) {
            return ToolResult::error(format!("Failed to create .d3vx directory: {}", e));
        }

        // Append to patch file
        let patch_path = self.get_patch_path(&context.cwd, task_id);
        let mut file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&patch_path)
        {
            Ok(f) => f,
            Err(e) => return ToolResult::error(format!("Failed to open patch file: {}", e)),
        };

        if let Err(e) = writeln!(file, "\n--- {}\n+++ {}\n{}", file_path, file_path, diff) {
            return ToolResult::error(format!("Failed to write to patch file: {}", e));
        }

        ToolResult::success(format!(
            "Successfully drafted change for {} and saved to {}",
            file_path,
            patch_path.display()
        ))
        .with_metadata(
            "patch_path",
            serde_json::json!(patch_path.to_string_lossy()),
        )
    }
}
