use crate::app::App;
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

impl App {
    /// Create a new workspace for a task (Mirror for non-git, Worktree for git)
    pub fn create_task_workspace(&self, task_id: &str, branch_name: &str) -> Result<PathBuf> {
        let _home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        let cwd = self.cwd.clone().unwrap_or_else(|| ".".to_string());

        // Detect if this is strictly a git root
        let toplevel = Command::new("git")
            .arg("rev-parse")
            .arg("--show-toplevel")
            .current_dir(&cwd)
            .output()
            .map(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or(None);

        let is_git_root = if let Some(root) = toplevel {
            let root_path = std::path::Path::new(&root).canonicalize().ok();
            let cwd_path = std::path::Path::new(&cwd).canonicalize().ok();
            root_path.is_some() && cwd_path.is_some() && root_path == cwd_path
        } else {
            false
        };

        if is_git_root {
            self.create_git_worktree(task_id, branch_name)
        } else {
            self.create_shadow_mirror(task_id)
        }
    }

    fn create_git_worktree(&self, task_id: &str, branch_name: &str) -> Result<PathBuf> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        let worktree_base = home_dir.join(".d3vx").join("worktrees");
        let worktree_path = worktree_base.join(task_id);
        let cwd = self.cwd.clone().unwrap_or_else(|| ".".to_string());

        if !worktree_base.exists() {
            std::fs::create_dir_all(&worktree_base)?;
        }

        info!(
            "Creating git worktree at {} for branch {}",
            worktree_path.display(),
            branch_name
        );

        let output = Command::new("git")
            .arg("worktree")
            .arg("add")
            .arg(&worktree_path)
            .arg(branch_name)
            .current_dir(&cwd)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if err.contains("invalid reference") {
                let output = Command::new("git")
                    .arg("worktree")
                    .arg("add")
                    .arg("-b")
                    .arg(branch_name)
                    .arg(&worktree_path)
                    .current_dir(&cwd)
                    .output()?;

                if !output.status.success() {
                    return Err(anyhow!(
                        "Failed to create worktree with new branch: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
            } else {
                return Err(anyhow!("Failed to create worktree: {}", err));
            }
        }

        Ok(worktree_path)
    }

    fn create_shadow_mirror(&self, task_id: &str) -> Result<PathBuf> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        let shadow_base = home_dir.join(".d3vx").join("shadows");
        let shadow_path = shadow_base.join(task_id);
        let cwd = self.cwd.clone().unwrap_or_else(|| ".".to_string());

        if !shadow_base.exists() {
            std::fs::create_dir_all(&shadow_base)?;
        }

        info!(
            "Creating shadow mirror at {} for non-git project",
            shadow_path.display()
        );

        // We use a simplified copy for now. In a full implementation,
        // we'd use rsync or a selective copy to avoid huge folders.
        // For security and performance, we'll try to use cp -R
        let output = Command::new("cp")
            .arg("-R")
            .arg(&cwd)
            .arg(&shadow_path)
            .output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to create shadow mirror: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Initialize a temporary git repo inside the shadow so agent tools work correctly
        let _ = Command::new("git")
            .arg("init")
            .current_dir(&shadow_path)
            .output();

        Ok(shadow_path)
    }

    /// Remove a workspace
    pub fn remove_task_workspace(&self, task_id: &str) -> Result<()> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;

        // Try removing from both possible locations
        let worktree_path = home_dir.join(".d3vx").join("worktrees").join(task_id);
        let shadow_path = home_dir.join(".d3vx").join("shadows").join(task_id);

        if worktree_path.exists() {
            let cwd = self.cwd.clone().unwrap_or_else(|| ".".to_string());
            let _ = Command::new("git")
                .arg("worktree")
                .arg("remove")
                .arg("--force")
                .arg(&worktree_path)
                .current_dir(&cwd)
                .output();
        }

        if shadow_path.exists() {
            let _ = std::fs::remove_dir_all(&shadow_path);
        }

        Ok(())
    }
}
