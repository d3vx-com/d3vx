//! File Change Log
//!
//! Tracks file modifications made by agent tools (Write, Edit, MultiEdit)
//! so that the undo picker can revert both conversation history AND file changes.

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{debug, warn};

/// A snapshot of a file before it was modified by a tool.
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    /// Absolute path to the file.
    pub file_path: String,
    /// Content before the tool ran. `None` means the file did not exist.
    pub old_content: Option<String>,
}

/// Tool names that modify files and should be tracked.
const FILE_MODIFYING_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit"];

/// Session-scoped log of file changes, keyed by message index.
///
/// Each entry records the file snapshots taken **before** a batch of tool
/// calls executed at a given conversation position. When the user picks an
/// undo point, we restore all files changed after that point.
#[derive(Debug, Clone, Default)]
pub struct FileChangeLog {
    /// Ordered list of (message_index, snapshots) pairs.
    entries: Vec<(usize, Vec<FileSnapshot>)>,
}

impl FileChangeLog {
    /// Create a new empty change log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Capture pre-change snapshots for a batch of tool calls.
    ///
    /// Call this **before** `execute_tools_concurrent` runs. It reads each
    /// file-modifying tool's target path and stores the current content
    /// (or `None` if the file doesn't exist yet).
    pub fn snapshot_for(
        &mut self,
        message_index: usize,
        calls: &[(String, String, serde_json::Value)],
        working_dir: &str,
    ) {
        let mut snapshots = Vec::new();

        for (_id, name, input) in calls {
            if !FILE_MODIFYING_TOOLS.contains(&name.as_str()) {
                continue;
            }

            let paths = Self::extract_file_paths(name, input, working_dir);
            for path in paths {
                let old_content = std::fs::read_to_string(&path).ok();
                if old_content.is_some() || name == "Write" {
                    snapshots.push(FileSnapshot {
                        file_path: path,
                        old_content,
                    });
                }
            }
        }

        if !snapshots.is_empty() {
            debug!(
                message_index,
                snapshot_count = snapshots.len(),
                "Captured file snapshots before tool execution"
            );
            self.entries.push((message_index, snapshots));
        }
    }

    /// Extract absolute file paths from a tool's input.
    fn extract_file_paths(
        tool_name: &str,
        input: &serde_json::Value,
        working_dir: &str,
    ) -> Vec<String> {
        match tool_name {
            "Write" | "Edit" | "MultiEdit" => input
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|p| vec![Self::resolve_path(p, working_dir)])
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// Resolve a potentially relative path to an absolute one.
    fn resolve_path(file_path: &str, working_dir: &str) -> String {
        let path = Path::new(file_path);
        if path.is_absolute() {
            file_path.to_string()
        } else {
            Path::new(working_dir)
                .join(path)
                .to_string_lossy()
                .to_string()
        }
    }

    /// Restore all files changed after `undo_index` to their pre-change state.
    ///
    /// Returns file paths that were successfully reverted.
    /// New files (no prior content) are deleted. Existing files get their
    /// original content written back. When a file appears in multiple
    /// snapshots, the **earliest** one wins.
    pub fn revert_to(&mut self, undo_index: usize) -> Vec<String> {
        let earliest = self.collect_earliest_snapshots(undo_index);
        let mut reverted = Vec::new();

        for (path, old_content) in &earliest {
            if self.restore_file(path, old_content) {
                reverted.push(path.clone());
            }
        }

        reverted.sort();
        reverted.dedup();
        reverted
    }

    /// Collect the earliest snapshot per unique file path after the undo index.
    fn collect_earliest_snapshots(&self, undo_index: usize) -> HashMap<String, Option<String>> {
        let mut earliest: HashMap<String, Option<String>> = HashMap::new();

        for (idx, snapshots) in &self.entries {
            if *idx > undo_index {
                for snap in snapshots {
                    earliest
                        .entry(snap.file_path.clone())
                        .or_insert(snap.old_content.clone());
                }
            }
        }

        earliest
    }

    /// Restore a single file to its pre-change state.
    fn restore_file(&self, path: &str, old_content: &Option<String>) -> bool {
        match old_content {
            Some(content) => self.write_content(path, content),
            None => self.delete_file(path),
        }
    }

    /// Write original content back to an existing file.
    fn write_content(&self, path: &str, content: &str) -> bool {
        if let Some(parent) = Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(path, content) {
            Ok(()) => {
                debug!(path, "Reverted file to pre-change state");
                true
            }
            Err(e) => {
                warn!(path, error = %e, "Failed to revert file");
                false
            }
        }
    }

    /// Delete a file that was newly created by the agent.
    fn delete_file(&self, path: &str) -> bool {
        match std::fs::remove_file(path) {
            Ok(()) => {
                debug!(path, "Deleted newly created file");
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
            Err(e) => {
                warn!(path, error = %e, "Failed to delete new file");
                false
            }
        }
    }

    /// Return unique file paths that were changed after the given index.
    pub fn files_after(&self, index: usize) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut paths = Vec::new();

        for (idx, snapshots) in &self.entries {
            if *idx > index {
                for snap in snapshots {
                    if seen.insert(snap.file_path.clone()) {
                        paths.push(snap.file_path.clone());
                    }
                }
            }
        }
        paths
    }

    /// Remove all entries with index > undo_index.
    pub fn truncate(&mut self, undo_index: usize) {
        self.entries.retain(|(idx, _)| *idx <= undo_index);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
