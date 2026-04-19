//! Conflict Resolver
//!
//! Detects and handles merge conflicts in isolated worktrees.

use crate::agent::{build_system_prompt, AgentLoop};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::info;

/// Result of a conflict detection check.
pub struct ConflictStatus {
    pub has_conflicts: bool,
    pub conflicted_files: Vec<String>,
}

pub struct ConflictResolver;

impl ConflictResolver {
    pub fn new() -> Self {
        Self
    }

    /// Check if the given worktree has merge conflicts.
    pub async fn check_conflicts(&self, worktree_path: &Path) -> Result<ConflictStatus> {
        let output = std::process::Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(worktree_path)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut conflicted_files = Vec::new();

        for line in stdout.lines() {
            if line.starts_with("UU ") || line.starts_with("AA ") {
                if let Some(file) = line.split_whitespace().last() {
                    conflicted_files.push(file.to_string());
                }
            }
        }

        Ok(ConflictStatus {
            has_conflicts: !conflicted_files.is_empty(),
            conflicted_files,
        })
    }

    /// Resolve conflicts by instructing the agent.
    pub async fn resolve_with_agent(
        &self,
        _worktree_path: &Path,
        files: &[String],
    ) -> Result<String> {
        info!("Attempting to resolve conflicts in files: {:?}", files);
        // This would build a prompt for the agent to resolve specific conflicts.
        Ok(format!(
            "Conflict resolution instructions generated for: {:?}",
            files
        ))
    }

    /// Attempt to resolve conflicts with a configured agent running inside the worktree.
    pub async fn attempt_resolution(
        &self,
        agent: Option<Arc<AgentLoop>>,
        worktree_path: &Path,
        base_branch: &str,
        files: &[String],
    ) -> Result<Option<String>> {
        let Some(agent) = agent else {
            return Ok(None);
        };

        let original_prompt = agent.system_prompt().await;
        let original_working_dir = agent.working_dir().await;
        let original_messages = agent.get_messages().await;

        let conflict_prompt = format!(
            "{}\n\nYou are resolving a git merge conflict in `{}` against base branch `{}`.\n\
Only work inside this directory. Resolve the conflict markers in these files:\n{}\n\n\
Requirements:\n\
- inspect the conflicted files\n\
- make the minimum correct edits\n\
- remove all conflict markers\n\
- use tools to verify `git status --porcelain`\n\
- do not commit, push, or open a PR\n\
- at the end, summarize what changed and any residual risk",
            build_system_prompt(&worktree_path.to_string_lossy(), None),
            worktree_path.display(),
            base_branch,
            files
                .iter()
                .map(|file| format!("- {}", file))
                .collect::<Vec<_>>()
                .join("\n")
        );

        agent.clear_history().await;
        agent
            .set_working_dir(worktree_path.to_string_lossy().to_string())
            .await;
        agent.set_system_prompt(conflict_prompt).await;
        agent
            .add_user_message("Resolve the merge conflict now.")
            .await;

        let run_result = agent.run().await;
        let agent_text = match run_result {
            Ok(result) => {
                if let Some(reason) = result.safety_stop_reason() {
                    agent.clear_history().await;
                    for message in original_messages {
                        agent.add_message(message).await;
                    }
                    agent.set_system_prompt(original_prompt).await;
                    agent.set_working_dir(original_working_dir).await;
                    return Err(anyhow::anyhow!(
                        "agent conflict resolution stopped for safety: {}",
                        reason
                    ));
                }
                result.text
            }
            Err(error) => {
                agent.clear_history().await;
                for message in original_messages {
                    agent.add_message(message).await;
                }
                agent.set_system_prompt(original_prompt).await;
                agent.set_working_dir(original_working_dir).await;
                return Err(anyhow::anyhow!(
                    "agent conflict resolution failed: {}",
                    error
                ));
            }
        };

        agent.clear_history().await;
        for message in original_messages {
            agent.add_message(message).await;
        }
        agent.set_system_prompt(original_prompt).await;
        agent.set_working_dir(original_working_dir).await;

        Ok(Some(agent_text))
    }

    /// Write a conflict report for human or agent follow-up.
    pub fn write_conflict_report(
        &self,
        workspace_path: &Path,
        base_branch: &str,
        files: &[String],
    ) -> Result<PathBuf> {
        let report_dir = workspace_path.join(".d3vx");
        std::fs::create_dir_all(&report_dir)?;
        let report_path = report_dir.join("merge-conflicts.md");
        let mut body = format!(
            "# Merge Conflict Report\n\nBase branch: `{}`\nWorkspace: `{}`\n\nConflicted files:\n",
            base_branch,
            workspace_path.display()
        );
        for file in files {
            body.push_str(&format!("- `{}`\n", file));
        }
        body.push_str("\nResolve these files, run tests, then retry the merge/PR flow.\n");
        std::fs::write(&report_path, body)?;
        Ok(report_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_conflict_report() {
        let temp = tempfile::tempdir().unwrap();
        let resolver = ConflictResolver::new();
        let report = resolver
            .write_conflict_report(
                temp.path(),
                "main",
                &["src/lib.rs".to_string(), "README.md".to_string()],
            )
            .unwrap();

        let content = std::fs::read_to_string(report).unwrap();
        assert!(content.contains("main"));
        assert!(content.contains("src/lib.rs"));
        assert!(content.contains("README.md"));
    }
}
