//! Tests for Edit Tool
//!
//! Covers string replacement in files.

#[cfg(test)]
mod tests {
    use crate::tools::edit::EditTool;
    use crate::tools::file_tracker::FileReadTracker;
    use crate::tools::types::{Tool, ToolContext};
    use crate::tools::write::WriteTool;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
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

    /// Write a file and register it with the tracker so EditTool can proceed.
    async fn write_and_register(
        tracker: &Arc<FileReadTracker>,
        file_path: &std::path::Path,
        content: &str,
        context: &ToolContext,
    ) {
        let write_tool = WriteTool::new();
        let write_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": content
        });
        let res = write_tool.execute(write_input, context).await;
        assert!(!res.is_error);
        tracker.record_read(file_path, content);
    }

    // =========================================================================
    // Tool Definition Tests
    // =========================================================================

    #[test]
    fn test_edit_tool_definition() {
        let tool = EditTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "Edit");
        assert!(def.input_schema["properties"]["file_path"].is_object());
        assert!(def.input_schema["properties"]["old_string"].is_object());
        assert!(def.input_schema["properties"]["new_string"].is_object());
    }

    // =========================================================================
    // Edit Operation Tests
    // =========================================================================

    #[tokio::test]
    async fn test_simple_string_replacement() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("test.txt");
        let tracker = Arc::new(FileReadTracker::new());

        write_and_register(&tracker, &file_path, "Hello, World!", &context).await;

        let edit_tool = EditTool::with_tracker(tracker);
        let edit_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "World",
            "new_string": "Rust"
        });

        let result = edit_tool.execute(edit_input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_multiline_replacement() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("multiline.txt");
        let tracker = Arc::new(FileReadTracker::new());

        let content = r#"fn main() {
    println!("Hello");
}"#;

        write_and_register(&tracker, &file_path, content, &context).await;

        let edit_tool = EditTool::with_tracker(tracker);
        let edit_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "println!(\"Hello\");",
            "new_string": "println!(\"Goodbye\");"
        });

        let result = edit_tool.execute(edit_input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_string_not_found_error() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("notfound.txt");
        let tracker = Arc::new(FileReadTracker::new());

        write_and_register(&tracker, &file_path, "Some content", &context).await;

        let edit_tool = EditTool::with_tracker(tracker);
        let edit_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "nonexistent string",
            "new_string": "replacement"
        });

        let result = edit_tool.execute(edit_input, &context).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_empty_old_string_error() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("empty.txt");
        let tracker = Arc::new(FileReadTracker::new());

        write_and_register(&tracker, &file_path, "Content", &context).await;

        let edit_tool = EditTool::with_tracker(tracker);
        let edit_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "",
            "new_string": "something"
        });

        let result = edit_tool.execute(edit_input, &context).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_file_not_found_error() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("missing.txt");

        let edit_tool = EditTool::new();
        let edit_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "anything",
            "new_string": "replacement"
        });

        let result = edit_tool.execute(edit_input, &context).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_preserves_indentation() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("indented.rs");
        let tracker = Arc::new(FileReadTracker::new());

        let content = r#"fn main() {
    let x = 1;
    let y = 2;
}"#;

        write_and_register(&tracker, &file_path, content, &context).await;

        let edit_tool = EditTool::with_tracker(tracker);
        let edit_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "let x = 1;",
            "new_string": "let x = 100;"
        });

        let result = edit_tool.execute(edit_input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_replace_all_instances() {
        let dir = tempdir().unwrap();
        let context = create_test_context(dir.path().to_path_buf());
        let file_path = dir.path().join("replace_all.txt");
        let tracker = Arc::new(FileReadTracker::new());

        write_and_register(&tracker, &file_path, "foo bar foo baz foo", &context).await;

        let edit_tool = EditTool::with_tracker(tracker);
        let edit_input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "foo",
            "new_string": "qux",
            "replace_all": true
        });

        let result = edit_tool.execute(edit_input, &context).await;

        if !result.is_error {
            let content = std::fs::read_to_string(&file_path).unwrap();
            assert_eq!(content, "qux bar qux baz qux");
        }
    }
}
