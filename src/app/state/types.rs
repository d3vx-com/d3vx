//! Core application type definitions

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    Chat,
    Plan,
    Verbose,
    CommandPalette,
    DiffPreview,
    UndoPicker,
    SessionPicker,
    Settings,
    Help,
    Board,
    List,
}

/// Focused inspector tab in the right pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPaneTab {
    #[default]
    Agent,
    Diff,
    Batch,
    Trust,
}

/// Lightweight focus preset for the chat input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FocusMode {
    #[default]
    Chat,
    Build,
    Plan,
    Docs,
    Test,
    Review,
}

impl FocusMode {
    pub const ALL: [FocusMode; 6] = [
        FocusMode::Chat,
        FocusMode::Build,
        FocusMode::Plan,
        FocusMode::Docs,
        FocusMode::Test,
        FocusMode::Review,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FocusMode::Chat => "Chat",
            FocusMode::Build => "Build",
            FocusMode::Plan => "Plan",
            FocusMode::Docs => "Docs",
            FocusMode::Test => "Test",
            FocusMode::Review => "Review",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            FocusMode::Chat => "Ask directly or add flags like --vex --review.",
            FocusMode::Build => "Implementation-first. Optimize for concrete code changes.",
            FocusMode::Plan => "Read-first. Analyze, decompose, and avoid edits unless asked.",
            FocusMode::Docs => "Bias toward README, docs, examples, and developer guidance.",
            FocusMode::Test => "Reproduce, validate, and improve regression coverage.",
            FocusMode::Review => "Inspect diffs, risks, ownership, and merge readiness.",
        }
    }

    pub fn system_instruction(self) -> Option<&'static str> {
        match self {
            FocusMode::Chat => None,
            FocusMode::Build => Some(
                "Focus mode: BUILD. Prefer concrete implementation work, targeted edits, verification, and clear delivery of the requested change.",
            ),
            FocusMode::Plan => Some(
                "Focus mode: PLAN. Bias toward analysis, architecture, decomposition, and explicit next steps. Avoid file edits unless the user asks for execution.",
            ),
            FocusMode::Docs => Some(
                "Focus mode: DOCS. Bias toward documentation quality, onboarding clarity, examples, changelog/readme updates, and developer-facing explanation.",
            ),
            FocusMode::Test => Some(
                "Focus mode: TEST. Follow disciplined testing workflow: 1) Reproduce the problem with a failing test, 2) Implement fix, 3) Verify test passes, 4) Run broader validation (type check, lint, full test suite), 5) Report confidence level. Always prefer concrete test assertions over assumptions.",
            ),
            FocusMode::Review => Some(
                "Focus mode: REVIEW. Bias toward code review findings, risk detection, changed-file inspection, ownership clarity, and merge readiness.",
            ),
        }
    }

    pub fn cycle(self, reverse: bool) -> FocusMode {
        let index = Self::ALL.iter().position(|mode| *mode == self).unwrap_or(0);
        if reverse {
            Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
        } else {
            Self::ALL[(index + 1) % Self::ALL.len()]
        }
    }
}

/// Workspace Task Status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceStatus {
    Idle,
    Thinking,
    ReadyToMerge,
    MergeConflicts,
    Archive,
}

/// Type of workspace
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceType {
    Anchor,    // Main project directory
    Satellite, // Isolated worktree
    SubAgent,  // Parallel agent loop
}

/// Represents an isolated agent worktree/task
#[derive(Debug, Clone)]
pub struct WorkspaceTask {
    pub id: String,
    pub name: String,
    pub branch: String,
    pub path: String,
    pub workspace_type: WorkspaceType,
    pub changes_added: usize,
    pub changes_removed: usize,
    pub status: WorkspaceStatus,
    pub phase: Option<String>,
}

/// Represents a file change in git
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub added: usize,
    pub removed: usize,
}

/// Tracks the state of a tool being executed
#[derive(Debug, Clone)]
pub struct ToolExecutionState {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub start_time: Instant,
    pub is_executing: bool,
    pub output: Option<String>,
    pub is_error: bool,
    pub elapsed_ms: u64,
}

/// Notification type for toasts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    Info,
    Success,
    Error,
}

/// A transient notification shown in the UI
#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub notification_type: NotificationType,
    pub timestamp: Instant,
    pub duration: std::time::Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_app_mode_default() {
        assert_eq!(AppMode::default(), AppMode::Chat);
    }

    #[test]
    fn test_workspace_task_creation() {
        let task = WorkspaceTask {
            id: "test".to_string(),
            name: "Test Task".to_string(),
            branch: "main".to_string(),
            path: "/tmp/test".to_string(),
            workspace_type: WorkspaceType::Satellite,
            changes_added: 0,
            changes_removed: 0,
            status: WorkspaceStatus::Idle,
            phase: None,
        };
        assert_eq!(task.id, "test");
        assert_eq!(task.workspace_type, WorkspaceType::Satellite);
    }

    #[test]
    fn test_notification_behavior() {
        let now = Instant::now();
        let notification = Notification {
            message: "Hello".to_string(),
            notification_type: NotificationType::Success,
            timestamp: now,
            duration: Duration::from_secs(5),
        };
        assert_eq!(notification.message, "Hello");
        assert_eq!(notification.notification_type, NotificationType::Success);
        assert_eq!(notification.timestamp, now);
    }

    #[test]
    fn test_tool_execution_state() {
        let now = Instant::now();
        let tool = ToolExecutionState {
            id: "call_123".to_string(),
            name: "ReadTool".to_string(),
            input: serde_json::json!({"path": "file.txt"}),
            start_time: now,
            is_executing: true,
            output: None,
            is_error: false,
            elapsed_ms: 0,
        };
        assert_eq!(tool.id, "call_123");
        assert!(tool.is_executing);
    }
}
