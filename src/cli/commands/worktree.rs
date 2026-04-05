//! Worktree Command Implementations
//!
//! Git worktree management for isolated task execution:
//! list, review, merge, discard, and recover operations.

use anyhow::Result;
use std::path::PathBuf;

use crate::config::{load_config, LoadConfigOptions};
use crate::pipeline::conflicts::ConflictResolver;
use crate::pipeline::{OrchestratorConfig, PipelineOrchestrator};

pub(crate) async fn execute_worktree(
    list: bool,
    review: Option<&str>,
    merge: Option<&str>,
    discard: Option<&str>,
    recover: bool,
) -> Result<()> {
    if list {
        return list_worktrees().await;
    }
    if let Some(task_id) = review {
        return review_worktree(task_id).await;
    }
    if let Some(task_id) = merge {
        return merge_worktree(task_id).await;
    }
    if let Some(task_id) = discard {
        return discard_worktree(task_id).await;
    }
    if recover {
        return recover_worktrees().await;
    }

    anyhow::bail!("No action specified. Use --list, --review, --merge, --discard, or --recover")
}

fn open_task_store() -> Result<crate::store::database::Database> {
    crate::store::database::Database::open_default()
        .map_err(|e| anyhow::anyhow!("failed to open task database: {}", e))
}

async fn list_worktrees() -> Result<()> {
    let db = open_task_store()?;
    let store = crate::store::task::TaskStore::from_connection(db.connection());
    let tasks = store.list(crate::store::task::TaskListOptions {
        limit: Some(200),
        ..Default::default()
    })?;

    let mut found = false;
    for task in tasks
        .into_iter()
        .filter(|task| task.worktree_path.is_some())
    {
        found = true;
        println!(
            "{}  {}  {}  {}",
            task.id,
            task.state,
            task.worktree_branch.as_deref().unwrap_or("-"),
            task.worktree_path.as_deref().unwrap_or("-"),
        );
    }

    if !found {
        println!("No active worktrees found.");
    }
    Ok(())
}

async fn review_worktree(task_id: &str) -> Result<()> {
    let db = open_task_store()?;
    let store = crate::store::task::TaskStore::from_connection(db.connection());
    let task = store
        .get(task_id)?
        .ok_or_else(|| anyhow::anyhow!("task {} not found", task_id))?;
    let worktree_path = task
        .worktree_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("task {} has no workspace", task_id))?;

    let status = git_output(worktree_path, &["status", "--short"])?;
    let diff = git_output(worktree_path, &["diff", "--stat"])?;

    println!("Task: {}", task.id);
    println!("Title: {}", task.title);
    println!("Branch: {}", task.worktree_branch.as_deref().unwrap_or("-"));
    println!("Workspace: {}", worktree_path);
    println!(
        "\nStatus:\n{}",
        if status.trim().is_empty() {
            "clean"
        } else {
            status.trim()
        }
    );
    println!(
        "\nDiff:\n{}",
        if diff.trim().is_empty() {
            "no tracked changes"
        } else {
            diff.trim()
        }
    );
    Ok(())
}

async fn merge_worktree(task_id: &str) -> Result<()> {
    let config = load_config(LoadConfigOptions::default())?;
    let default_branch = config
        .integrations
        .as_ref()
        .and_then(|i| i.github.as_ref())
        .map(|g| g.default_branch.clone())
        .unwrap_or_else(|| config.git.main_branch.clone());

    let db = open_task_store()?;
    let store = crate::store::task::TaskStore::from_connection(db.connection());
    let task = store
        .get(task_id)?
        .ok_or_else(|| anyhow::anyhow!("task {} not found", task_id))?;
    let workspace_path = task
        .worktree_path
        .clone()
        .ok_or_else(|| anyhow::anyhow!("task {} has no workspace", task_id))?;
    let branch = task
        .worktree_branch
        .clone()
        .ok_or_else(|| anyhow::anyhow!("task {} has no worktree branch", task_id))?;

    ensure_worktree_merge_ready(&workspace_path, &default_branch).await?;

    let repo_path = task
        .repo_root
        .clone()
        .or(task.project_path.clone())
        .unwrap_or(std::env::current_dir()?.to_string_lossy().to_string());

    run_git_sync(&repo_path, &["fetch", "origin", &default_branch])?;
    run_git_sync(&repo_path, &["checkout", &default_branch])?;
    let pull_output = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("pull")
        .arg("--ff-only")
        .arg("origin")
        .arg(&default_branch)
        .output()?;
    if !pull_output.status.success() {
        return Err(anyhow::anyhow!(
            "failed to update {} before merge: {}",
            default_branch,
            String::from_utf8_lossy(&pull_output.stderr).trim()
        ));
    }

    run_git_sync(&repo_path, &["merge", "--no-ff", &branch])?;
    store.transition(task_id, crate::store::task::TaskState::Done)?;
    println!("Merged {} into {}", branch, default_branch);
    Ok(())
}

async fn discard_worktree(task_id: &str) -> Result<()> {
    let db = open_task_store()?;
    let store = crate::store::task::TaskStore::from_connection(db.connection());
    let task = store
        .get(task_id)?
        .ok_or_else(|| anyhow::anyhow!("task {} not found", task_id))?;
    if let Some(path) = task.worktree_path.as_deref() {
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("merge")
            .arg("--abort")
            .output();
    }

    if let Some(path) = task.worktree_path.as_deref() {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(std::env::current_dir()?)
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(path)
            .output()?;
        if !output.status.success() {
            eprintln!(
                "warning: worktree remove failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
    }

    if let Some(branch) = task.worktree_branch.as_deref() {
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(std::env::current_dir()?)
            .arg("branch")
            .arg("-D")
            .arg(branch)
            .output();
    }

    store.transition(task_id, crate::store::task::TaskState::Failed)?;
    println!("Discarded workspace for {}", task_id);
    Ok(())
}

async fn recover_worktrees() -> Result<()> {
    let config = load_config(LoadConfigOptions::default())?;
    let mut orch_config = OrchestratorConfig::default();
    orch_config.checkpoint_dir =
        PathBuf::from(crate::config::get_global_config_dir()).join("checkpoints");
    orch_config.github = config.integrations.as_ref().and_then(|i| i.github.clone());
    let orchestrator = PipelineOrchestrator::new(orch_config, None).await?;
    let recovered = orchestrator.recover_interrupted_tasks().await?;
    println!("Recovered {} interrupted tasks", recovered.len());
    for task in recovered {
        println!("{}  {}", task.id, task.title);
    }
    Ok(())
}

fn git_output(path: &str, args: &[&str]) -> Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git {:?} failed in {}: {}",
            args,
            path,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_sync(path: &str, args: &[&str]) -> Result<()> {
    let _ = git_output(path, args)?;
    Ok(())
}

async fn ensure_worktree_merge_ready(workspace_path: &str, base_branch: &str) -> Result<()> {
    run_git_sync(workspace_path, &["fetch", "origin", base_branch])?;

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .arg("merge")
        .arg("--no-commit")
        .arg("--no-ff")
        .arg(format!("origin/{}", base_branch))
        .output()?;

    if output.status.success() {
        let _ = run_git_sync(workspace_path, &["merge", "--abort"]);
        return Ok(());
    }

    let resolver = ConflictResolver::new();
    let status = resolver
        .check_conflicts(std::path::Path::new(workspace_path))
        .await?;
    let _ = run_git_sync(workspace_path, &["merge", "--abort"]);

    if status.has_conflicts {
        let report = resolver.write_conflict_report(
            std::path::Path::new(workspace_path),
            base_branch,
            &status.conflicted_files,
        )?;
        return Err(anyhow::anyhow!(
            "merge conflicts detected in {}. Report written to {}",
            status.conflicted_files.join(", "),
            report.display()
        ));
    }

    Err(anyhow::anyhow!(
        "pre-merge check against {} failed: {}",
        base_branch,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}
