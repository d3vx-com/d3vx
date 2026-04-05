//! Tests for Glob and Grep Tools
//!
//! Covers file pattern matching and content search.

#[cfg(test)]
mod tests {
    use crate::tools::glob::GlobTool;
    use crate::tools::grep::GrepTool;
    use crate::tools::types::{Tool, ToolContext};
    use crate::tools::write::WriteTool;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn create_test_context(cwd: PathBuf) -> ToolContext {
        ToolContext {
            cwd: cwd.to_string_lossy().to_string(),
            env: HashMap::new(),
            trust_mode: false,
            bash_blocklist: vec![],
            swarm_membership: None,
            session_id: Some("test-session".to_string()),
            parent_session_id: None,
            agent_depth: 0,
            allow_parallel_spawn: true,
            sandbox_mode: crate::config::types::SandboxMode::Disabled,
            sandbox_config: None,
        }
    }

    async fn setup_test_files(dir: &std::path::Path) {
        let write_tool = WriteTool::new();
        let context = create_test_context(dir.to_path_buf());

        // Create test files
        let files = vec![
            ("src/main.rs", "fn main() { println!(\"Hello\"); }"),
            ("src/lib.rs", "pub fn helper() {}"),
            ("src/utils/mod.rs", "pub mod string_utils;"),
            ("tests/test_main.rs", "#[test] fn test_main() {}"),
            ("README.md", "# Project\n\nDescription here."),
            ("Cargo.toml", "[package]\nname = \"test\""),
        ];

        for (path, content) in files {
            let full_path = dir.join(path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let input = serde_json::json!({
                "file_path": full_path.to_str().unwrap(),
                "content": content
            });
            write_tool.execute(input, &context).await;
        }
    }

    // =========================================================================
    // Glob Tool Tests
    // =========================================================================

    #[test]
    fn test_glob_tool_definition() {
        let tool = GlobTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "Glob");
        assert!(def.input_schema["properties"]["pattern"].is_object());
    }

    #[tokio::test]
    async fn test_glob_find_rust_files() {
        let dir = tempdir().unwrap();
        setup_test_files(dir.path()).await;
        let context = create_test_context(dir.path().to_path_buf());

        let glob_tool = GlobTool::new();
        let input = serde_json::json!({
            "pattern": "**/*.rs"
        });

        let result = glob_tool.execute(input, &context).await;
        assert!(!result.is_error);
        assert!(result.content.contains(".rs"));
    }

    #[tokio::test]
    async fn test_glob_find_by_name() {
        let dir = tempdir().unwrap();
        setup_test_files(dir.path()).await;
        let context = create_test_context(dir.path().to_path_buf());

        let glob_tool = GlobTool::new();
        let input = serde_json::json!({
            "pattern": "**/main.rs"
        });

        let result = glob_tool.execute(input, &context).await;
        assert!(!result.is_error);
        assert!(result.content.contains("main.rs"));
    }

    #[tokio::test]
    async fn test_glob_find_markdown() {
        let dir = tempdir().unwrap();
        setup_test_files(dir.path()).await;
        let context = create_test_context(dir.path().to_path_buf());

        let glob_tool = GlobTool::new();
        let input = serde_json::json!({
            "pattern": "*.md"
        });

        let result = glob_tool.execute(input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let dir = tempdir().unwrap();
        setup_test_files(dir.path()).await;
        let context = create_test_context(dir.path().to_path_buf());

        let glob_tool = GlobTool::new();
        let input = serde_json::json!({
            "pattern": "**/*.nonexistent"
        });

        let result = glob_tool.execute(input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_glob_missing_pattern() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());

        let glob_tool = GlobTool::new();
        let input = serde_json::json!({});

        let result = glob_tool.execute(input, &context).await;
        assert!(result.is_error);
    }

    // =========================================================================
    // Grep Tool Tests
    // =========================================================================

    #[test]
    fn test_grep_tool_definition() {
        let tool = GrepTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "Grep");
        assert!(def.input_schema["properties"]["pattern"].is_object());
    }

    #[tokio::test]
    async fn test_grep_find_pattern() {
        let dir = tempdir().unwrap();
        setup_test_files(dir.path()).await;
        let context = create_test_context(dir.path().to_path_buf());

        let grep_tool = GrepTool::new();
        let input = serde_json::json!({
            "pattern": "fn",
            "path": dir.path().to_str().unwrap()
        });

        let result = grep_tool.execute(input, &context).await;
        assert!(!result.is_error);
        assert!(result.content.contains("fn"));
    }

    #[tokio::test]
    async fn test_grep_case_sensitive() {
        let dir = tempdir().unwrap();
        setup_test_files(dir.path()).await;
        let context = create_test_context(dir.path().to_path_buf());

        let grep_tool = GrepTool::new();
        let input = serde_json::json!({
            "pattern": "Fn",  // Note: capital F
            "path": dir.path().to_str().unwrap(),
            "case_insensitive": false
        });

        let result = grep_tool.execute(input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        let dir = tempdir().unwrap();
        setup_test_files(dir.path()).await;
        let context = create_test_context(dir.path().to_path_buf());

        let grep_tool = GrepTool::new();
        let input = serde_json::json!({
            "pattern": "HELLO",  // Note: all caps
            "path": dir.path().to_str().unwrap(),
            "case_insensitive": true
        });

        let result = grep_tool.execute(input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_grep_with_file_pattern() {
        let dir = tempdir().unwrap();
        setup_test_files(dir.path()).await;
        let context = create_test_context(dir.path().to_path_buf());

        let grep_tool = GrepTool::new();
        let input = serde_json::json!({
            "pattern": "fn",
            "path": dir.path().to_str().unwrap(),
            "glob": "*.rs"
        });

        let result = grep_tool.execute(input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let dir = tempdir().unwrap();
        setup_test_files(dir.path()).await;
        let context = create_test_context(dir.path().to_path_buf());

        let grep_tool = GrepTool::new();
        let input = serde_json::json!({
            "pattern": "nonexistent_pattern_xyz123",
            "path": dir.path().to_str().unwrap()
        });

        let result = grep_tool.execute(input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_grep_missing_pattern() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());

        let grep_tool = GrepTool::new();
        let input = serde_json::json!({});

        let result = grep_tool.execute(input, &context).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_grep_regex_pattern() {
        let dir = tempdir().unwrap();
        setup_test_files(dir.path()).await;
        let context = create_test_context(dir.path().to_path_buf());

        let grep_tool = GrepTool::new();
        let input = serde_json::json!({
            "pattern": "fn\\s+\\w+",
            "path": dir.path().to_str().unwrap()
        });

        let result = grep_tool.execute(input, &context).await;
        assert!(!result.is_error);
    }
}
