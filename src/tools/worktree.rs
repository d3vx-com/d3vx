//! Worktree Tools
//!
//! Git worktree management for isolated task execution.
//! Provides tools to create and remove git worktrees with branch management.

use async_trait::async_trait;
use std::path::Path;
use std::process::Command;

use super::types::{Tool, ToolContext, ToolDefinition, ToolResult};

/// Tool for creating git worktrees with isolated branches
pub struct WorktreeCreateTool {
    definition: ToolDefinition,
}

impl WorktreeCreateTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "worktree_create".to_string(),
                description: concat!(
                    "Create a git worktree for isolated task execution. ",
                    "Creates a new directory at .d3vx-worktrees/d3vx-{id}/ ",
                    "with a new branch d3vx/{id} from HEAD."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Optional identifier for the worktree. If not provided, a short ID is generated."
                        }
                    }
                }),
            },
        }
    }
}

impl Default for WorktreeCreateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WorktreeCreateTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let provided_name = input["name"].as_str().unwrap_or("");
        let id = if provided_name.is_empty() {
            generate_short_id()
        } else {
            sanitize_identifier(provided_name)
        };

        if id.is_empty() {
            return ToolResult::error(
                "Invalid worktree name: must contain alphanumeric characters",
            );
        }

        let worktree_dir = format!(".d3vx-worktrees/d3vx-{}", id);
        let branch_name = format!("d3vx/{}", id);

        // Check if worktree already exists
        let worktree_path = Path::new(&context.cwd).join(&worktree_dir);
        if worktree_path.exists() {
            return ToolResult::error(format!("Worktree already exists at {}", worktree_dir));
        }

        // Ensure parent directory exists
        let parent = worktree_path.parent().unwrap_or(Path::new("."));
        if !parent.exists() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return ToolResult::error(format!(
                    "Failed to create worktree parent directory: {}",
                    e
                ));
            }
        }

        // Create worktree: git worktree add <path> -b <branch> HEAD
        let output = Command::new("git")
            .args(["worktree", "add", &worktree_dir, "-b", &branch_name, "HEAD"])
            .current_dir(&context.cwd)
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    ToolResult::success(format!(
                        "Created worktree at {} on branch {}\n{}",
                        worktree_dir,
                        branch_name,
                        stdout.trim()
                    ))
                    .with_metadata("path", serde_json::json!(worktree_dir))
                    .with_metadata("branch", serde_json::json!(branch_name))
                    .with_metadata("id", serde_json::json!(id))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    ToolResult::error(format!("Failed to create worktree: {}", stderr.trim()))
                }
            }
            Err(e) => ToolResult::error(format!("Failed to execute git worktree add: {}", e)),
        }
    }
}

/// Tool for removing git worktrees with optional branch cleanup
pub struct WorktreeRemoveTool {
    definition: ToolDefinition,
}

impl WorktreeRemoveTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "worktree_remove".to_string(),
                description: concat!(
                    "Remove a git worktree. Optionally removes the associated branch. ",
                    "Fails if uncommitted changes exist unless discard_changes is true."
                )
                .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Identifier of the worktree to remove"
                        },
                        "action": {
                            "type": "string",
                            "enum": ["keep", "remove"],
                            "description": "Whether to keep or remove the branch after removing the worktree (default: keep)"
                        },
                        "discard_changes": {
                            "type": "boolean",
                            "description": "If true, discard uncommitted changes. If false and changes exist, the operation fails (default: false)"
                        }
                    },
                    "required": ["name"]
                }),
            },
        }
    }
}

impl Default for WorktreeRemoveTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WorktreeRemoveTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let name = input["name"].as_str().unwrap_or("");
        if name.is_empty() {
            return ToolResult::error("name is required");
        }

        let action = input["action"].as_str().unwrap_or("keep");
        let discard_changes = input["discard_changes"].as_bool().unwrap_or(false);

        let id = sanitize_identifier(name);
        let worktree_dir = format!(".d3vx-worktrees/d3vx-{}", id);
        let branch_name = format!("d3vx/{}", id);
        let worktree_path = Path::new(&context.cwd).join(&worktree_dir);

        // Verify worktree exists
        if !worktree_path.exists() {
            return ToolResult::error(format!("Worktree not found: {}", worktree_dir));
        }

        // Check for uncommitted changes unless discard_changes is true
        if !discard_changes {
            let status_output = Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(&worktree_path)
                .output();

            match status_output {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if !stdout.trim().is_empty() {
                        return ToolResult::error(format!(
                            "Worktree has uncommitted changes. Set discard_changes=true to force removal, or commit/stash changes first.\nUncommitted files:\n{}",
                            stdout.trim()
                        ));
                    }
                }
                Err(e) => {
                    return ToolResult::error(format!("Failed to check worktree status: {}", e));
                }
            }
        }

        // Remove worktree: git worktree remove <path>
        let remove_output = Command::new("git")
            .args(["worktree", "remove", &worktree_dir])
            .current_dir(&context.cwd)
            .output();

        match remove_output {
            Ok(output) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    // Try force removal if discard_changes was requested
                    if discard_changes {
                        let force_output = Command::new("git")
                            .args(["worktree", "remove", "--force", &worktree_dir])
                            .current_dir(&context.cwd)
                            .output();
                        match force_output {
                            Ok(fo) if fo.status.success() => {}
                            Ok(fo) => {
                                let fo_stderr = String::from_utf8_lossy(&fo.stderr);
                                return ToolResult::error(format!(
                                    "Failed to force-remove worktree: {}",
                                    fo_stderr.trim()
                                ));
                            }
                            Err(e) => {
                                return ToolResult::error(format!(
                                    "Failed to execute force remove: {}",
                                    e
                                ));
                            }
                        }
                    } else {
                        return ToolResult::error(format!(
                            "Failed to remove worktree: {}",
                            stderr.trim()
                        ));
                    }
                }
            }
            Err(e) => {
                return ToolResult::error(format!("Failed to execute git worktree remove: {}", e));
            }
        }

        // Optionally remove branch
        if action == "remove" {
            let branch_output = Command::new("git")
                .args(["branch", "-D", &branch_name])
                .current_dir(&context.cwd)
                .output();

            match branch_output {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        // Branch removal failure is non-fatal; report it in the result
                        return ToolResult::success(format!(
                            "Removed worktree {} but failed to delete branch {}: {}",
                            worktree_dir,
                            branch_name,
                            stderr.trim()
                        ))
                        .with_metadata("worktree_path", serde_json::json!(worktree_dir))
                        .with_metadata("branch_removed", serde_json::json!(false));
                    }
                }
                Err(e) => {
                    return ToolResult::success(format!(
                        "Removed worktree {} but failed to delete branch {}: {}",
                        worktree_dir, branch_name, e
                    ))
                    .with_metadata("worktree_path", serde_json::json!(worktree_dir))
                    .with_metadata("branch_removed", serde_json::json!(false));
                }
            }
        }

        let message = if action == "remove" {
            format!(
                "Removed worktree {} and deleted branch {}",
                worktree_dir, branch_name
            )
        } else {
            format!(
                "Removed worktree {} (branch {} preserved)",
                worktree_dir, branch_name
            )
        };

        ToolResult::success(message)
            .with_metadata("worktree_path", serde_json::json!(worktree_dir))
            .with_metadata("branch_removed", serde_json::json!(action == "remove"))
    }
}

/// Generate a short random identifier using a truncated timestamp and random suffix
fn generate_short_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    // Use lower 16 bits of timestamp for shortness
    let short_ts = (ts & 0xFFFF) as u16;
    format!("{:04x}", short_ts)
}

/// Sanitize a user-provided identifier to be safe for branch/directory names
fn sanitize_identifier(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a temporary git repo for worktree testing
    fn setup_git_repo() -> (tempfile::TempDir, String) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().to_string_lossy().to_string();

        // Initialize git repo
        let init = Command::new("git")
            .args(["init"])
            .current_dir(&path)
            .output()
            .unwrap();
        assert!(init.status.success(), "git init failed");

        // Configure user for commits
        Command::new("git")
            .args(["config", "user.email", "test@d3vx.dev"])
            .current_dir(&path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&path)
            .output()
            .unwrap();

        // Create initial commit so HEAD exists
        fs::write(temp_dir.path().join("README.md"), "# test").unwrap();
        let add = Command::new("git")
            .args(["add", "README.md"])
            .current_dir(&path)
            .output()
            .unwrap();
        assert!(add.status.success(), "git add failed");

        let commit = Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&path)
            .output()
            .unwrap();
        assert!(commit.status.success(), "git commit failed");

        (temp_dir, path)
    }

    #[tokio::test]
    async fn test_create_worktree_with_name() {
        let (_temp_dir, path) = setup_git_repo();

        let tool = WorktreeCreateTool::new();
        let context = ToolContext {
            cwd: path.clone(),
            ..Default::default()
        };

        let result = tool
            .execute(serde_json::json!({"name": "my-task"}), &context)
            .await;

        assert!(!result.is_error, "Tool returned error: {}", result.content);
        assert!(result.content.contains("d3vx-my-task"));
        assert!(result.content.contains("d3vx/my-task"));
        assert_eq!(result.metadata["path"], ".d3vx-worktrees/d3vx-my-task");
        assert_eq!(result.metadata["branch"], "d3vx/my-task");

        // Verify worktree directory exists
        assert!(Path::new(&path)
            .join(".d3vx-worktrees/d3vx-my-task")
            .exists());

        // Cleanup worktree
        Command::new("git")
            .args(["worktree", "remove", ".d3vx-worktrees/d3vx-my-task"])
            .current_dir(&path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["branch", "-D", "d3vx/my-task"])
            .current_dir(&path)
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn test_create_worktree_without_name() {
        let (_temp_dir, path) = setup_git_repo();

        let tool = WorktreeCreateTool::new();
        let context = ToolContext {
            cwd: path.clone(),
            ..Default::default()
        };

        let result = tool.execute(serde_json::json!({}), &context).await;

        assert!(!result.is_error, "Tool returned error: {}", result.content);
        assert!(result.metadata["id"].is_string());

        let id = result.metadata["id"].as_str().unwrap();
        let worktree_path = Path::new(&path).join(format!(".d3vx-worktrees/d3vx-{}", id));
        assert!(worktree_path.exists());

        // Cleanup
        Command::new("git")
            .args([
                "worktree",
                "remove",
                &format!(".d3vx-worktrees/d3vx-{}", id),
            ])
            .current_dir(&path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["branch", "-D", &format!("d3vx/{}", id)])
            .current_dir(&path)
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn test_create_duplicate_worktree_fails() {
        let (_temp_dir, path) = setup_git_repo();

        let tool = WorktreeCreateTool::new();
        let context = ToolContext {
            cwd: path.clone(),
            ..Default::default()
        };

        // Create first worktree
        let result1 = tool
            .execute(serde_json::json!({"name": "dup"}), &context)
            .await;
        assert!(!result1.is_error);

        // Attempt duplicate
        let result2 = tool
            .execute(serde_json::json!({"name": "dup"}), &context)
            .await;
        assert!(result2.is_error);
        assert!(result2.content.contains("already exists"));

        // Cleanup
        Command::new("git")
            .args(["worktree", "remove", ".d3vx-worktrees/d3vx-dup"])
            .current_dir(&path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["branch", "-D", "d3vx/dup"])
            .current_dir(&path)
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn test_remove_worktree_keep_branch() {
        let (_temp_dir, path) = setup_git_repo();

        let create_tool = WorktreeCreateTool::new();
        let remove_tool = WorktreeRemoveTool::new();
        let context = ToolContext {
            cwd: path.clone(),
            ..Default::default()
        };

        // Create worktree
        create_tool
            .execute(serde_json::json!({"name": "removeme"}), &context)
            .await;

        // Remove worktree, keep branch
        let result = remove_tool
            .execute(
                serde_json::json!({"name": "removeme", "action": "keep"}),
                &context,
            )
            .await;

        assert!(!result.is_error, "Tool returned error: {}", result.content);
        assert!(result.content.contains("preserved"));
        assert!(!Path::new(&path)
            .join(".d3vx-worktrees/d3vx-removeme")
            .exists());

        // Verify branch still exists
        let branches = Command::new("git")
            .args(["branch", "--list", "d3vx/removeme"])
            .current_dir(&path)
            .output()
            .unwrap();
        let branch_list = String::from_utf8_lossy(&branches.stdout);
        assert!(branch_list.contains("d3vx/removeme"));

        // Cleanup branch
        Command::new("git")
            .args(["branch", "-D", "d3vx/removeme"])
            .current_dir(&path)
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn test_remove_worktree_delete_branch() {
        let (_temp_dir, path) = setup_git_repo();

        let create_tool = WorktreeCreateTool::new();
        let remove_tool = WorktreeRemoveTool::new();
        let context = ToolContext {
            cwd: path.clone(),
            ..Default::default()
        };

        // Create worktree
        create_tool
            .execute(serde_json::json!({"name": "deleteme"}), &context)
            .await;

        // Remove worktree and delete branch
        let result = remove_tool
            .execute(
                serde_json::json!({"name": "deleteme", "action": "remove"}),
                &context,
            )
            .await;

        assert!(!result.is_error, "Tool returned error: {}", result.content);
        assert!(result.content.contains("deleted branch"));
        assert_eq!(result.metadata["branch_removed"], true);

        // Verify branch is gone
        let branches = Command::new("git")
            .args(["branch", "--list", "d3vx/deleteme"])
            .current_dir(&path)
            .output()
            .unwrap();
        let branch_list = String::from_utf8_lossy(&branches.stdout);
        assert!(!branch_list.contains("d3vx/deleteme"));
    }

    #[tokio::test]
    async fn test_remove_nonexistent_worktree_fails() {
        let (_temp_dir, path) = setup_git_repo();

        let tool = WorktreeRemoveTool::new();
        let context = ToolContext {
            cwd: path,
            ..Default::default()
        };

        let result = tool
            .execute(serde_json::json!({"name": "nonexistent"}), &context)
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn test_remove_with_uncommitted_changes_fails() {
        let (_temp_dir, path) = setup_git_repo();

        let create_tool = WorktreeCreateTool::new();
        let remove_tool = WorktreeRemoveTool::new();
        let context = ToolContext {
            cwd: path.clone(),
            ..Default::default()
        };

        // Create worktree
        create_tool
            .execute(serde_json::json!({"name": "dirty"}), &context)
            .await;

        // Create uncommitted change in worktree
        let worktree_path = Path::new(&path).join(".d3vx-worktrees/d3vx-dirty");
        fs::write(worktree_path.join("untracked.txt"), "dirty content").unwrap();

        // Removal should fail without discard_changes
        let result = remove_tool
            .execute(serde_json::json!({"name": "dirty"}), &context)
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("uncommitted changes"));

        // Cleanup with force
        Command::new("git")
            .args([
                "worktree",
                "remove",
                "--force",
                ".d3vx-worktrees/d3vx-dirty",
            ])
            .current_dir(&path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["branch", "-D", "d3vx/dirty"])
            .current_dir(&path)
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn test_remove_with_discard_changes_succeeds() {
        let (_temp_dir, path) = setup_git_repo();

        let create_tool = WorktreeCreateTool::new();
        let remove_tool = WorktreeRemoveTool::new();
        let context = ToolContext {
            cwd: path.clone(),
            ..Default::default()
        };

        // Create worktree
        create_tool
            .execute(serde_json::json!({"name": "force-dirty"}), &context)
            .await;

        // Create uncommitted change
        let worktree_path = Path::new(&path).join(".d3vx-worktrees/d3vx-force-dirty");
        fs::write(worktree_path.join("untracked.txt"), "will be discarded").unwrap();

        // Removal with discard_changes should succeed
        let result = remove_tool
            .execute(
                serde_json::json!({"name": "force-dirty", "discard_changes": true}),
                &context,
            )
            .await;

        assert!(!result.is_error, "Tool returned error: {}", result.content);
        assert!(!Path::new(&path)
            .join(".d3vx-worktrees/d3vx-force-dirty")
            .exists());

        // Cleanup branch
        Command::new("git")
            .args(["branch", "-D", "d3vx/force-dirty"])
            .current_dir(&path)
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn test_remove_empty_name_fails() {
        let tool = WorktreeRemoveTool::new();
        let context = ToolContext::default();

        let result = tool
            .execute(serde_json::json!({"name": ""}), &context)
            .await;

        assert!(result.is_error);
        assert!(result.content.contains("name is required"));
    }

    #[tokio::test]
    async fn test_sanitize_identifier() {
        assert_eq!(sanitize_identifier("hello-world"), "hello-world");
        assert_eq!(sanitize_identifier("my_task"), "my_task");
        assert_eq!(sanitize_identifier("task 123!@#"), "task123");
        assert_eq!(sanitize_identifier(""), "");
        assert_eq!(sanitize_identifier("!@#$%"), "");
    }

    #[tokio::test]
    async fn test_generate_short_id() {
        let id = generate_short_id();
        assert!(!id.is_empty());
        assert_eq!(id.len(), 4);
        // Should be hex
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn test_tool_names() {
        assert_eq!(WorktreeCreateTool::new().name(), "worktree_create");
        assert_eq!(WorktreeRemoveTool::new().name(), "worktree_remove");
    }
}
