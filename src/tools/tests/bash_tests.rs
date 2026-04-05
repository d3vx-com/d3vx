//! Tests for Bash Tool
//!
//! Covers command execution, timeout handling, and security features.

#[cfg(test)]
mod tests {
    use crate::tools::bash::BashTool;
    use crate::tools::types::{Tool, ToolContext};
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn create_test_context() -> ToolContext {
        ToolContext {
            cwd: std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .to_string(),
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
    // Tool Definition Tests
    // =========================================================================

    #[test]
    fn test_bash_tool_definition() {
        let tool = BashTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "Bash");
        assert!(def.description.to_lowercase().contains("bash"));
    }

    #[test]
    fn test_bash_tool_default() {
        let tool1 = BashTool::new();
        let tool2 = BashTool::default();

        assert_eq!(tool1.definition().name, tool2.definition().name);
    }

    // =========================================================================
    // Command Execution Tests
    // =========================================================================

    #[tokio::test]
    async fn test_simple_echo_command() {
        let tool = BashTool::new();
        let context = create_test_context();
        let input = serde_json::json!({
            "command": "echo 'Hello, World!'"
        });

        let result = tool.execute(input, &context).await;

        assert!(!result.is_error);
        assert!(result.content.contains("Hello, World!"));
    }

    #[tokio::test]
    async fn test_command_with_pipe() {
        let tool = BashTool::new();
        let context = create_test_context();
        let input = serde_json::json!({
            "command": "echo -e 'apple\\nbanana\\ncherry' | grep 'banana'"
        });

        let result = tool.execute(input, &context).await;

        assert!(!result.is_error);
        assert!(result.content.contains("banana"));
        assert!(!result.content.contains("apple"));
    }

    #[tokio::test]
    async fn test_command_exit_code() {
        let tool = BashTool::new();
        let context = create_test_context();
        let input = serde_json::json!({
            "command": "exit 0"
        });

        let result = tool.execute(input, &context).await;
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_command_nonzero_exit_code() {
        let tool = BashTool::new();
        let context = create_test_context();
        let input = serde_json::json!({
            "command": "exit 1"
        });

        let result = tool.execute(input, &context).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_command_with_output() {
        let tool = BashTool::new();
        let context = create_test_context();
        let input = serde_json::json!({
            "command": "ls -la"
        });

        let result = tool.execute(input, &context).await;

        assert!(!result.is_error);
        assert!(!result.content.is_empty());
    }

    // =========================================================================
    // Input Validation Tests
    // =========================================================================

    #[tokio::test]
    async fn test_empty_command_returns_error() {
        let tool = BashTool::new();
        let context = create_test_context();
        let input = serde_json::json!({
            "command": ""
        });

        let result = tool.execute(input, &context).await;

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_missing_command_returns_error() {
        let tool = BashTool::new();
        let context = create_test_context();
        let input = serde_json::json!({});

        let result = tool.execute(input, &context).await;

        assert!(result.is_error);
    }

    // =========================================================================
    // Working Directory Tests
    // =========================================================================

    #[tokio::test]
    async fn test_command_in_cwd() {
        let dir = tempdir().unwrap();
        let tool = BashTool::new();
        let context = ToolContext {
            cwd: dir.path().to_string_lossy().to_string(),
            env: HashMap::new(),
            trust_mode: false,
            bash_blocklist: vec![],
            swarm_membership: None,
            session_id: Some("test".to_string()),
            parent_session_id: None,
            agent_depth: 0,
            allow_parallel_spawn: true,
            sandbox_mode: crate::config::types::SandboxMode::Disabled,
            sandbox_config: None,
        };

        let input = serde_json::json!({
            "command": "pwd"
        });

        let result = tool.execute(input, &context).await;

        assert!(!result.is_error);
        assert!(result
            .content
            .contains(&dir.path().to_string_lossy().to_string()));
    }

    // =========================================================================
    // Security Tests
    // =========================================================================

    #[tokio::test]
    async fn test_blocklist_pattern_blocks_command() {
        use regex::Regex;

        let tool = BashTool::new();
        let context = ToolContext {
            cwd: std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            env: HashMap::new(),
            trust_mode: false,
            bash_blocklist: vec![Regex::new("rm -rf /").unwrap()],
            swarm_membership: None,
            session_id: Some("test".to_string()),
            parent_session_id: None,
            agent_depth: 0,
            allow_parallel_spawn: true,
            sandbox_mode: crate::config::types::SandboxMode::Disabled,
            sandbox_config: None,
        };

        let input = serde_json::json!({
            "command": "rm -rf /some/path"
        });

        let result = tool.execute(input, &context).await;

        assert!(result.is_error);
        assert!(result.content.to_lowercase().contains("blocked"));
    }

    #[tokio::test]
    async fn test_non_blocklisted_command_executes() {
        use regex::Regex;

        let tool = BashTool::new();
        let context = ToolContext {
            cwd: std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            env: HashMap::new(),
            trust_mode: false,
            bash_blocklist: vec![Regex::new("dangerous-cmd").unwrap()],
            swarm_membership: None,
            session_id: Some("test".to_string()),
            parent_session_id: None,
            agent_depth: 0,
            allow_parallel_spawn: true,
            sandbox_mode: crate::config::types::SandboxMode::Disabled,
            sandbox_config: None,
        };

        let input = serde_json::json!({
            "command": "echo 'safe'"
        });

        let result = tool.execute(input, &context).await;
        assert!(!result.is_error);
    }

    // =========================================================================
    // Environment Variable Tests
    // =========================================================================

    #[tokio::test]
    async fn test_command_with_env_vars() {
        let tool = BashTool::new();
        let mut env = HashMap::new();
        env.insert("MY_VAR".to_string(), "test_value".to_string());

        let context = ToolContext {
            cwd: std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            env,
            trust_mode: false,
            bash_blocklist: vec![],
            swarm_membership: None,
            session_id: Some("test".to_string()),
            parent_session_id: None,
            agent_depth: 0,
            allow_parallel_spawn: true,
            sandbox_mode: crate::config::types::SandboxMode::Disabled,
            sandbox_config: None,
        };

        let input = serde_json::json!({
            "command": "echo $MY_VAR"
        });

        let result = tool.execute(input, &context).await;

        assert!(!result.is_error);
        assert!(result.content.contains("test_value"));
    }
}
