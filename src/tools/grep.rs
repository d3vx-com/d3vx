//! Grep Tool
//!
//! Search for patterns in file contents.

use async_trait::async_trait;
use regex::Regex;
use std::path::Path;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Grep tool for searching file contents
pub struct GrepTool {
    definition: ToolDefinition,
}

impl GrepTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "Grep".to_string(),
                description: concat!(
                    "Search for a regex pattern in file contents. ",
                    "Supports full regex syntax. ",
                    "Returns matching lines with file paths and line numbers. ",
                    "Use for finding code patterns, text, or specific content."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "The regex pattern to search for"
                        },
                        "path": {
                            "type": "string",
                            "description": "File or directory to search (default: current directory)"
                        },
                        "glob": {
                            "type": "string",
                            "description": "Glob pattern to filter files (e.g., '*.rs', '*.ts')"
                        },
                        "output_mode": {
                            "type": "string",
                            "enum": ["content", "files_with_matches", "count"],
                            "description": "Output format (default: content)"
                        },
                        "-i": {
                            "type": "boolean",
                            "description": "Case insensitive search"
                        },
                        "-n": {
                            "type": "boolean",
                            "description": "Show line numbers (default: true for content mode)"
                        },
                        "-C": {
                            "type": "number",
                            "description": "Context lines around matches"
                        },
                        "-A": {
                            "type": "number",
                            "description": "Lines after match"
                        },
                        "-B": {
                            "type": "number",
                            "description": "Lines before match"
                        }
                    },
                    "required": ["pattern"]
                }),
            },
        }
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let pattern = input["pattern"].as_str().unwrap_or("");
        let search_path = input["path"].as_str().unwrap_or(&context.cwd);
        let glob_pattern = input["glob"].as_str();
        let output_mode = input["output_mode"].as_str().unwrap_or("content");
        let case_insensitive = input["-i"].as_bool().unwrap_or(false);
        let context_lines = input["-C"].as_u64().unwrap_or(0) as usize;
        let after_lines = input["-A"].as_u64().unwrap_or(0) as usize;
        let before_lines = input["-B"].as_u64().unwrap_or(0) as usize;

        if pattern.is_empty() {
            return ToolResult::error("pattern is required");
        }

        // Build regex
        let regex_pattern = if case_insensitive {
            format!("(?i){}", pattern)
        } else {
            pattern.to_string()
        };

        let regex = match Regex::new(&regex_pattern) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Invalid regex pattern: {}", e)),
        };

        // Resolve path
        let base_path = if Path::new(search_path).is_absolute() {
            Path::new(search_path).to_path_buf()
        } else {
            Path::new(&context.cwd).join(search_path)
        };

        // Collect files to search
        let files: Vec<std::path::PathBuf> = if base_path.is_file() {
            vec![base_path.clone()]
        } else {
            // Walk directory
            let mut files = Vec::new();
            if let Some(glob) = glob_pattern {
                // Use glob pattern
                if let Ok(walker) =
                    globwalk::GlobWalkerBuilder::from_patterns(&base_path, &[glob]).build()
                {
                    for entry in walker.flatten() {
                        if entry.path().is_file() {
                            files.push(entry.path().to_path_buf());
                        }
                    }
                }
            } else {
                // Walk all files
                for entry in walkdir::WalkDir::new(&base_path).into_iter().flatten() {
                    if entry.path().is_file() {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
            files
        };

        // Search files
        let mut matches: Vec<GrepMatch> = Vec::new();

        for file_path in &files {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                for (line_num, line) in content.lines().enumerate() {
                    if regex.is_match(line) {
                        matches.push(GrepMatch {
                            file: file_path.to_string_lossy().to_string(),
                            line_num: line_num + 1,
                            line: line.to_string(),
                        });
                    }
                }
            }
        }

        // Format output based on mode
        if matches.is_empty() {
            return ToolResult::success("No matches found.");
        }

        let output = match output_mode {
            "files_with_matches" => {
                let files: std::collections::HashSet<_> =
                    matches.iter().map(|m| m.file.clone()).collect();
                files.into_iter().collect::<Vec<_>>().join("\n")
            }
            "count" => {
                let counts: std::collections::HashMap<String, usize> =
                    matches
                        .iter()
                        .fold(std::collections::HashMap::new(), |mut acc, m| {
                            *acc.entry(m.file.clone()).or_insert(0) += 1;
                            acc
                        });
                counts
                    .iter()
                    .map(|(f, c)| format!("{}: {}", f, c))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => {
                // content mode
                let _context = context_lines.max(before_lines.max(after_lines));
                matches
                    .iter()
                    .map(|m| format!("{}:{}:{}", m.file, m.line_num, m.line))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        };

        ToolResult::success(output)
            .with_metadata("matches", serde_json::json!(matches.len()))
            .with_metadata("pattern", serde_json::json!(pattern))
    }
}

struct GrepMatch {
    file: String,
    line_num: usize,
    line: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_grep_find_pattern() {
        let temp_dir = std::env::temp_dir().join("d3vx_grep_test");
        fs::create_dir_all(&temp_dir).ok();

        // Create test file
        fs::write(
            temp_dir.join("test.txt"),
            "Hello World\nFoo Bar\nHello Rust\n",
        )
        .unwrap();

        let tool = GrepTool::new();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "Hello"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error);
        assert!(result.content.contains("Hello"));
        assert!(result.metadata["matches"].as_u64().unwrap() >= 2);

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_grep_files_with_matches() {
        let temp_dir =
            std::env::temp_dir().join(format!("d3vx_grep_files_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        fs::write(temp_dir.join("a.txt"), "needle\n").unwrap();
        fs::write(temp_dir.join("b.txt"), "haystack\n").unwrap();

        let tool = GrepTool::new();
        let context = ToolContext {
            cwd: temp_dir.to_string_lossy().to_string(),
            ..Default::default()
        };

        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "needle",
                    "output_mode": "files_with_matches"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error, "Tool returned error: {}", result.content);
        assert!(
            result.content.contains("a.txt"),
            "Result missing a.txt: {}",
            result.content
        );
        assert!(!result.content.contains("b.txt"));

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }
}
