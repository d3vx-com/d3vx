//! Glob Tool
//!
//! Find files matching a glob pattern.

use async_trait::async_trait;
use std::path::Path;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Glob tool for finding files by pattern
pub struct GlobTool {
    definition: ToolDefinition,
}

impl GlobTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "Glob".to_string(),
                description: concat!(
                    "Find files matching a glob pattern. ",
                    "Supports ** for recursive matching, * for single-level, and ? for single character. ",
                    "Returns matching file paths sorted by modification time. ",
                    "Use for finding files by name pattern."
                ).to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "The glob pattern to match (e.g., '**/*.ts', 'src/**/*.rs')"
                        },
                        "path": {
                            "type": "string",
                            "description": "The directory to search in (default: current directory)"
                        }
                    },
                    "required": ["pattern"]
                }),
            },
        }
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let pattern = input["pattern"].as_str().unwrap_or("");
        let search_path = input["path"].as_str().unwrap_or(&context.cwd);

        if pattern.is_empty() {
            return ToolResult::error("pattern is required");
        }

        // Resolve path relative to cwd
        let base_path = if Path::new(search_path).is_absolute() {
            Path::new(search_path).to_path_buf()
        } else {
            Path::new(&context.cwd).join(search_path)
        };

        // Walk with the panic-safe helper (globwalk 0.9 unwraps a
        // strip_prefix internally and aborts the process on certain
        // symlink / path-normalisation cases).
        let mut matches: Vec<std::path::PathBuf> =
            match crate::utils::glob_walk::walk_matching(&base_path, pattern, true) {
                Ok(m) => m,
                Err(e) => return ToolResult::error(format!("Invalid glob pattern: {}", e)),
            };

        // Sort by modification time (newest first)
        matches.sort_by(|a, b| {
            let meta_a = std::fs::metadata(a);
            let meta_b = std::fs::metadata(b);
            let time_a = meta_a.ok().and_then(|m| m.modified().ok());
            let time_b = meta_b.ok().and_then(|m| m.modified().ok());
            time_b.cmp(&time_a)
        });

        // Format output
        if matches.is_empty() {
            return ToolResult::success("No files found matching pattern.");
        }

        let output: Vec<String> = matches
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        ToolResult::success(output.join("\n"))
            .with_metadata("count", serde_json::json!(matches.len()))
            .with_metadata("pattern", serde_json::json!(pattern))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_glob_find_files() {
        let temp_dir =
            std::env::temp_dir().join(format!("d3vx_glob_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Create some test files
        fs::write(temp_dir.join("test1.txt"), "content1").unwrap();
        fs::write(temp_dir.join("test2.txt"), "content2").unwrap();
        fs::write(temp_dir.join("other.rs"), "rust content").unwrap();

        let tool = GlobTool::new();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "*.txt"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error, "Tool returned error: {}", result.content);
        assert!(
            result.content.contains("test1.txt"),
            "Result missing test1.txt: {}",
            result.content
        );
        assert!(
            result.content.contains("test2.txt"),
            "Result missing test2.txt: {}",
            result.content
        );
        assert!(!result.content.contains("other.rs"));

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }
}
