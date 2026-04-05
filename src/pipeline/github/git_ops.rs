//! Git Operations for GitHub Integration
//!
//! Low-level git commands used during branch push, merge preparation,
//! and workspace validation before PR creation or auto-merge.

use crate::agent::AgentLoop;
use crate::pipeline::conflicts::ConflictResolver;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;

pub async fn push_branch(project_path: &str, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("push")
        .arg("-u")
        .arg("origin")
        .arg(branch)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to push branch {}: {}",
            branch,
            stderr.trim()
        ));
    }

    Ok(())
}

pub async fn ensure_branch_merge_ready(
    workspace_path: &str,
    base_branch: &str,
    agent: Option<Arc<AgentLoop>>,
) -> Result<()> {
    run_git(workspace_path, &["fetch", "origin", base_branch]).await?;

    let merge_attempt = Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .arg("merge")
        .arg("--no-commit")
        .arg("--no-ff")
        .arg(format!("origin/{}", base_branch))
        .output()
        .await?;

    if merge_attempt.status.success() {
        let _ = run_git(workspace_path, &["merge", "--abort"]).await;
        return Ok(());
    }

    let resolver = ConflictResolver::new();
    let mut conflict_status = resolver.check_conflicts(Path::new(workspace_path)).await?;
    let _ = run_git(workspace_path, &["merge", "--abort"]).await;

    if conflict_status.has_conflicts {
        let agent_attempt = resolver
            .attempt_resolution(
                agent,
                Path::new(workspace_path),
                base_branch,
                &conflict_status.conflicted_files,
            )
            .await?;

        if agent_attempt.is_some() {
            conflict_status = resolver.check_conflicts(Path::new(workspace_path)).await?;
            validate_workspace_for_merge(workspace_path).await?;
            if !conflict_status.has_conflicts {
                return Ok(());
            }
        }

        let report_path = resolver.write_conflict_report(
            Path::new(workspace_path),
            base_branch,
            &conflict_status.conflicted_files,
        )?;
        let handoff = if let Some(agent_summary) = agent_attempt {
            format!(
                "Agent attempted a resolution but conflicts remain. Summary: {}",
                agent_summary
            )
        } else {
            resolver
                .resolve_with_agent(Path::new(workspace_path), &conflict_status.conflicted_files)
                .await?
        };
        return Err(anyhow::anyhow!(
            "Merge conflicts against {} in {}. {} Report: {}",
            base_branch,
            conflict_status.conflicted_files.join(", "),
            handoff,
            report_path.display()
        ));
    }

    let stderr = String::from_utf8_lossy(&merge_attempt.stderr);
    Err(anyhow::anyhow!(
        "Pre-PR merge check failed against {}: {}",
        base_branch,
        stderr.trim()
    ))
}

pub async fn validate_workspace_for_merge(workspace_path: &str) -> Result<()> {
    let diff_check = Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .arg("diff")
        .arg("--check")
        .output()
        .await?;

    if !diff_check.status.success() {
        return Err(anyhow::anyhow!(
            "workspace {} failed git diff --check: {}",
            workspace_path,
            String::from_utf8_lossy(&diff_check.stdout).trim()
        ));
    }

    let grep_output = Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .arg("grep")
        .arg("-n")
        .arg("-E")
        .arg("^(<<<<<<<|=======|>>>>>>>)")
        .arg("--")
        .output()
        .await?;

    if grep_output.status.success() && !grep_output.stdout.is_empty() {
        return Err(anyhow::anyhow!(
            "workspace {} still contains conflict markers: {}",
            workspace_path,
            String::from_utf8_lossy(&grep_output.stdout).trim()
        ));
    }

    Ok(())
}

pub async fn run_git(workspace_path: &str, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_path)
        .args(args)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "git {:?} failed in {}: {}",
            args,
            workspace_path,
            stderr.trim()
        ));
    }

    Ok(())
}
