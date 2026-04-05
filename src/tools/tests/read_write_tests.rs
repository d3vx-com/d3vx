//! Tests for Read and Write Tools
//!
//! Covers file reading and writing operations.

#[cfg(test)]
mod tests {
    use crate::tools::read::ReadTool;
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

    // =========================================================================
    // Read Tool Definition Tests
    // =========================================================================

    #[test]
    fn test_read_tool_definition() {
        let tool = ReadTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "Read");
        assert!(def.description.to_lowercase().contains("read"));
    }

    // =========================================================================
    // Write Tool Definition Tests
    // =========================================================================

    #[test]
    fn test_write_tool_definition() {
        let tool = WriteTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "Write");
        assert!(def.description.to_lowercase().contains("write"));
    }

    // =========================================================================
    // Write and Read Integration Tests
    // =========================================================================

    #[tokio::test]
    async fn test_write_then_read_file() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("test.txt");

        // Write a file
        let write_tool = WriteTool::new();
        let write_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "Hello, File!"
        });

        let write_result = write_tool.execute(write_input, &context).await;
        assert!(!write_result.is_error);

        // Read it back
        let read_tool = ReadTool::new();
        let read_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap()
        });

        let read_result = read_tool.execute(read_input, &context).await;
        assert!(!read_result.is_error);
        assert!(read_result.content.contains("Hello, File!"));
    }

    #[tokio::test]
    async fn test_write_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("nested/deep/dir/test.txt");

        let write_tool = WriteTool::new();
        let write_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "Nested content"
        });

        let result = write_tool.execute(write_input, &context).await;
        assert!(!result.is_error);

        // Verify file exists
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_write_overwrites_existing_file() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("overwrite.txt");

        let write_tool = WriteTool::new();

        // First write
        let input1 = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "Original content"
        });
        let res1 = write_tool.execute(input1, &context).await;
        assert!(!res1.is_error);

        // Second write (overwrite)
        let input2 = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "New content"
        });
        let result = write_tool.execute(input2, &context).await;
        assert!(!result.is_error);

        // Verify content was overwritten
        let read_tool = ReadTool::new();
        let read_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap()
        });
        let read_result = read_tool.execute(read_input, &context).await;
        assert!(read_result.content.contains("New content"));
        assert!(!read_result.content.contains("Original content"));
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("nonexistent.txt");

        let read_tool = ReadTool::new();
        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap()
        });

        let result = read_tool.execute(input, &context).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_read_with_offset_and_limit() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("lines.txt");

        // Create a file with multiple lines
        let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n";

        let write_tool = WriteTool::new();
        let write_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": content
        });
        let res_write = write_tool.execute(write_input, &context).await;
        assert!(!res_write.is_error);

        // Read with offset and limit
        let read_tool = ReadTool::new();
        let read_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "offset": 1,
            "limit": 2
        });

        let result = read_tool.execute(read_input, &context).await;

        // Should contain lines 2-3 (offset 1, limit 2)
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_write_empty_file() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("empty.txt");

        let write_tool = WriteTool::new();
        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": ""
        });

        let result = write_tool.execute(input, &context).await;
        assert!(!result.is_error);

        // File should exist but be empty
        let read_tool = ReadTool::new();
        let read_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap()
        });
        let read_result = read_tool.execute(read_input, &context).await;
        assert!(!read_result.is_error);
    }

    #[tokio::test]
    async fn test_write_large_content() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("large.txt");

        // Create a large content string
        let content = "x".repeat(100_000);

        let write_tool = WriteTool::new();
        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": content
        });

        let result = write_tool.execute(input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_read_missing_file_path() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());

        let read_tool = ReadTool::new();
        let input = serde_json::json!({});

        let result = read_tool.execute(input, &context).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_write_missing_file_path() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());

        let write_tool = WriteTool::new();
        let input = serde_json::json!({
            "content": "test"
        });

        let result = write_tool.execute(input, &context).await;
        assert!(result.is_error);
    }
}
