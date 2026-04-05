use crate::app::state::FileChange;
use crate::app::App;
use crate::utils::diff::generate_unified_diff;
use anyhow::Result;
use std::fs;
use std::process::Command;

impl App {
    /// Refresh git status information for the right sidebar
    pub fn refresh_git_status(&mut self) -> Result<()> {
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
            // Canonicalize both to be sure
            let root_path = std::path::Path::new(&root).canonicalize().ok();
            let cwd_path = std::path::Path::new(&cwd).canonicalize().ok();
            root_path.is_some() && cwd_path.is_some() && root_path == cwd_path
        } else {
            false
        };

        if !is_git_root {
            self.active_branch = "None (Subdir Mode)".to_string();
            self.git_changes = Vec::new();
            self.selected_diff_index = 0;
            self.diff_preview = None;
            return Ok(());
        }

        // Get active branch
        if let Ok(output) = Command::new("git")
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .current_dir(&cwd)
            .output()
        {
            if output.status.success() {
                self.active_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }

        // Get git changes (porcelain)
        if let Ok(output) = Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(&cwd)
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut changes = Vec::new();
                for line in stdout.lines() {
                    // Git porcelain format: XY PATH
                    if line.len() > 3 {
                        let status_code = &line[..2];
                        let path = line[3..].to_string();

                        // Heuristic for added/removed if we really wanted to be specific,
                        // but porcelain only gives us index/worktree status.
                        // For a "glanceable" UI, we'll just indicate it's modified.
                        changes.push(FileChange {
                            path,
                            added: if status_code.contains('A') || status_code.contains('?') {
                                1
                            } else {
                                0
                            },
                            removed: if status_code.contains('D') { 1 } else { 0 },
                        });
                    }
                }
                self.git_changes = changes;
                if self.git_changes.is_empty() {
                    self.selected_diff_index = 0;
                    self.diff_preview = None;
                } else {
                    if self.selected_diff_index >= self.git_changes.len() {
                        self.selected_diff_index = self.git_changes.len().saturating_sub(1);
                    }
                    self.refresh_selected_diff_preview();
                }
            }
        }

        Ok(())
    }

    pub fn select_git_change(&mut self, index: usize) {
        if index >= self.git_changes.len() {
            return;
        }
        self.selected_diff_index = index;
        self.selected_right_pane_tab = crate::app::state::RightPaneTab::Diff;
        self.refresh_selected_diff_preview();
    }

    pub fn cycle_git_change(&mut self, direction: isize) {
        if self.git_changes.is_empty() {
            return;
        }

        let len = self.git_changes.len() as isize;
        let current = self.selected_diff_index.min(self.git_changes.len() - 1) as isize;
        let next = (current + direction).rem_euclid(len) as usize;
        self.select_git_change(next);
    }

    pub fn refresh_selected_diff_preview(&mut self) {
        let cwd = self.cwd.clone().unwrap_or_else(|| ".".to_string());
        self.diff_preview = self
            .git_changes
            .get(self.selected_diff_index)
            .and_then(|change| self.load_diff_preview(&cwd, &change.path));
    }

    fn load_diff_preview(
        &self,
        cwd: &str,
        relative_path: &str,
    ) -> Option<crate::ui::widgets::DiffView> {
        let diff_output = Command::new("git")
            .arg("diff")
            .arg("--")
            .arg(relative_path)
            .current_dir(cwd)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .unwrap_or_default();

        if !diff_output.trim().is_empty() {
            return Some(crate::ui::widgets::DiffView::new(
                relative_path,
                &diff_output,
            ));
        }

        let abs_path = std::path::Path::new(cwd).join(relative_path);
        if abs_path.is_file() {
            let file_content = fs::read_to_string(&abs_path).ok()?;
            let synthetic_diff = generate_unified_diff(relative_path, "", &file_content);
            if synthetic_diff.trim().is_empty() {
                None
            } else {
                Some(crate::ui::widgets::DiffView::new(
                    relative_path,
                    &synthetic_diff,
                ))
            }
        } else {
            None
        }
    }
}
