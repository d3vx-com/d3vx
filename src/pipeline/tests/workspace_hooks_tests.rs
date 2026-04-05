//! Tests for workspace post-create hooks

use std::fs;

use super::super::workspace_hooks::{
    load_workspace_config, HookCommand, SymlinkEntry, WorkspaceHookExecutor, WorkspaceHookResult,
    WorkspaceHooksConfig,
};

#[test]
fn test_default_config_is_empty() {
    let config = WorkspaceHooksConfig::default();
    assert!(config.symlinks.is_empty());
    assert!(config.commands.is_empty());
    assert!(config.copy_files.is_empty());
}

#[test]
fn test_config_serde_roundtrip() {
    let config = WorkspaceHooksConfig {
        symlinks: vec![SymlinkEntry {
            source: ".env".to_string(),
            target: ".env".to_string(),
            overwrite: true,
        }],
        commands: vec![HookCommand {
            command: "npm install".to_string(),
            working_dir: None,
            timeout_secs: Some(60),
            continue_on_error: false,
            description: Some("Install npm dependencies".to_string()),
        }],
        copy_files: vec![".env.local".to_string()],
    };
    let yaml = serde_yaml::to_string(&config).unwrap();
    let parsed: WorkspaceHooksConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(config, parsed);
}

#[test]
fn test_hook_result_default() {
    let result = WorkspaceHookResult::default();
    assert_eq!(result.symlinks_created, 0);
    assert_eq!(result.commands_run, 0);
    assert_eq!(result.commands_failed, 0);
    assert_eq!(result.files_copied, 0);
    assert!(result.errors.is_empty());
}

// --- Execute integration tests ---

fn setup_dirs() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let worktree_path = temp.path().join("worktree");
    fs::create_dir_all(&project_root).unwrap();
    fs::create_dir_all(&worktree_path).unwrap();
    (temp, project_root, worktree_path)
}

#[test]
fn test_execute_symlinks_only() {
    let (temp, project_root, worktree_path) = setup_dirs();
    fs::write(project_root.join(".env"), "KEY=value").unwrap();
    // Drop temp after use to satisfy compiler
    let _ = temp;

    let config = WorkspaceHooksConfig {
        symlinks: vec![SymlinkEntry {
            source: ".env".to_string(),
            target: ".env".to_string(),
            overwrite: false,
        }],
        commands: vec![],
        copy_files: vec![],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.symlinks_created, 1);
    assert_eq!(result.files_copied, 0);
    assert_eq!(result.commands_run, 0);
    assert!(result.errors.is_empty());

    // Verify symlink works
    let link_target = fs::read_link(worktree_path.join(".env")).unwrap();
    assert_eq!(link_target, project_root.join(".env"));
}

#[test]
fn test_execute_symlink_overwrite() {
    let (_temp, project_root, worktree_path) = setup_dirs();
    fs::write(project_root.join(".env"), "KEY=value").unwrap();
    fs::write(worktree_path.join(".env"), "old").unwrap();

    let config = WorkspaceHooksConfig {
        symlinks: vec![SymlinkEntry {
            source: ".env".to_string(),
            target: ".env".to_string(),
            overwrite: true,
        }],
        commands: vec![],
        copy_files: vec![],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.symlinks_created, 1);
    assert!(result.errors.is_empty());
}

#[test]
fn test_execute_symlink_no_overwrite_fails() {
    let (_temp, project_root, worktree_path) = setup_dirs();
    fs::write(project_root.join(".env"), "KEY=value").unwrap();
    fs::write(worktree_path.join(".env"), "old").unwrap();

    let config = WorkspaceHooksConfig {
        symlinks: vec![SymlinkEntry {
            source: ".env".to_string(),
            target: ".env".to_string(),
            overwrite: false,
        }],
        commands: vec![],
        copy_files: vec![],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.symlinks_created, 0);
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("already exists"));
}

#[test]
fn test_execute_symlink_missing_source() {
    let (_temp, project_root, worktree_path) = setup_dirs();

    let config = WorkspaceHooksConfig {
        symlinks: vec![SymlinkEntry {
            source: ".env".to_string(),
            target: ".env".to_string(),
            overwrite: false,
        }],
        commands: vec![],
        copy_files: vec![],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.symlinks_created, 0);
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("does not exist"));
}

#[test]
fn test_execute_commands_success() {
    let (_temp, project_root, worktree_path) = setup_dirs();

    let config = WorkspaceHooksConfig {
        symlinks: vec![],
        commands: vec![HookCommand {
            command: "echo hello".to_string(),
            working_dir: None,
            timeout_secs: Some(10),
            continue_on_error: false,
            description: Some("Test echo".to_string()),
        }],
        copy_files: vec![],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.commands_run, 1);
    assert_eq!(result.commands_failed, 0);
    assert!(result.errors.is_empty());
}

#[test]
fn test_execute_commands_failure_continue() {
    let (_temp, project_root, worktree_path) = setup_dirs();

    let config = WorkspaceHooksConfig {
        symlinks: vec![],
        commands: vec![
            HookCommand {
                command: "false".to_string(),
                working_dir: None,
                timeout_secs: Some(10),
                continue_on_error: true,
                description: Some("Will fail".to_string()),
            },
            HookCommand {
                command: "echo ok".to_string(),
                working_dir: None,
                timeout_secs: Some(10),
                continue_on_error: false,
                description: Some("Should run".to_string()),
            },
        ],
        copy_files: vec![],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.commands_run, 2);
    assert_eq!(result.commands_failed, 1);
    assert_eq!(result.errors.len(), 1);
}

#[test]
fn test_execute_commands_failure_stops() {
    let (_temp, project_root, worktree_path) = setup_dirs();

    let config = WorkspaceHooksConfig {
        symlinks: vec![],
        commands: vec![
            HookCommand {
                command: "false".to_string(),
                working_dir: None,
                timeout_secs: Some(10),
                continue_on_error: false,
                description: Some("Will fail".to_string()),
            },
            HookCommand {
                command: "echo ok".to_string(),
                working_dir: None,
                timeout_secs: Some(10),
                continue_on_error: false,
                description: Some("Should NOT run".to_string()),
            },
        ],
        copy_files: vec![],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.commands_run, 1);
    assert_eq!(result.commands_failed, 1);
}

#[test]
fn test_execute_copy_files_basic() {
    let (_temp, project_root, worktree_path) = setup_dirs();
    fs::write(project_root.join("config.json"), "{}").unwrap();

    let config = WorkspaceHooksConfig {
        symlinks: vec![],
        commands: vec![],
        copy_files: vec!["config.json".to_string()],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.files_copied, 1);
    assert!(result.errors.is_empty());

    let content = fs::read_to_string(worktree_path.join("config.json")).unwrap();
    assert_eq!(content, "{}");
}

#[test]
fn test_execute_copy_files_missing_source() {
    let (_temp, project_root, worktree_path) = setup_dirs();

    let config = WorkspaceHooksConfig {
        symlinks: vec![],
        commands: vec![],
        copy_files: vec!["missing.txt".to_string()],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.files_copied, 0);
    assert_eq!(result.errors.len(), 1);
}

#[test]
fn test_execute_full_pipeline() {
    let (_temp, project_root, worktree_path) = setup_dirs();
    fs::write(project_root.join(".env"), "KEY=value").unwrap();
    fs::write(project_root.join("setup.cfg"), "data").unwrap();

    let config = WorkspaceHooksConfig {
        symlinks: vec![SymlinkEntry {
            source: ".env".to_string(),
            target: ".env".to_string(),
            overwrite: false,
        }],
        commands: vec![HookCommand {
            command: "echo done".to_string(),
            working_dir: None,
            timeout_secs: Some(10),
            continue_on_error: false,
            description: None,
        }],
        copy_files: vec!["setup.cfg".to_string()],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.symlinks_created, 1);
    assert_eq!(result.files_copied, 1);
    assert_eq!(result.commands_run, 1);
    assert_eq!(result.commands_failed, 0);
    assert!(result.errors.is_empty());
}

#[test]
fn test_execute_empty_config() {
    let (_temp, project_root, worktree_path) = setup_dirs();

    let config = WorkspaceHooksConfig::default();
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.symlinks_created, 0);
    assert_eq!(result.files_copied, 0);
    assert_eq!(result.commands_run, 0);
    assert_eq!(result.commands_failed, 0);
    assert!(result.errors.is_empty());
}

#[test]
fn test_execute_working_dir_relative() {
    let (_temp, project_root, worktree_path) = setup_dirs();
    let subdir = worktree_path.join("frontend");
    fs::create_dir_all(&subdir).unwrap();

    let config = WorkspaceHooksConfig {
        symlinks: vec![],
        commands: vec![HookCommand {
            command: "pwd".to_string(),
            working_dir: Some("frontend".to_string()),
            timeout_secs: Some(10),
            continue_on_error: false,
            description: Some("Test subdir".to_string()),
        }],
        copy_files: vec![],
    };
    let result = WorkspaceHookExecutor::execute(&project_root, &worktree_path, &config);
    assert_eq!(result.commands_run, 1);
    assert_eq!(result.commands_failed, 0);
    assert!(result.errors.is_empty());
}

// --- Config loading tests ---

#[test]
fn test_load_workspace_config_missing_file() {
    let temp = tempfile::tempdir().unwrap();
    let config = load_workspace_config(temp.path());
    assert_eq!(config, WorkspaceHooksConfig::default());
}

#[test]
fn test_load_workspace_config_valid_file() {
    let temp = tempfile::tempdir().unwrap();
    let d3vx_dir = temp.path().join(".d3vx");
    fs::create_dir_all(&d3vx_dir).unwrap();

    let yaml = r#"
symlinks:
  - source: .env
    target: .env
    overwrite: true
commands:
  - command: npm install
    timeout_secs: 60
    continue_on_error: false
    description: Install dependencies
copy_files:
  - .env.local
"#;
    fs::write(d3vx_dir.join("workspace.yml"), yaml).unwrap();

    let config = load_workspace_config(temp.path());
    assert_eq!(config.symlinks.len(), 1);
    assert_eq!(config.symlinks[0].source, ".env");
    assert_eq!(config.commands.len(), 1);
    assert_eq!(config.commands[0].command, "npm install");
    assert_eq!(config.copy_files.len(), 1);
}

#[test]
fn test_load_workspace_config_invalid_yaml() {
    let temp = tempfile::tempdir().unwrap();
    let d3vx_dir = temp.path().join(".d3vx");
    fs::create_dir_all(&d3vx_dir).unwrap();
    fs::write(d3vx_dir.join("workspace.yml"), "{{invalid yaml}}").unwrap();

    let config = load_workspace_config(temp.path());
    assert_eq!(config, WorkspaceHooksConfig::default());
}
