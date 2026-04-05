//! Pre-commit hooks module
//!
//! This module provides automatic quality checks that run before commits are made
//! from the TUI or agent. This ensures no broken code enters the repository and
//! tests are always passing before merge.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────┐
//! │  HookRegistry                                                     │
//! │  ┌──────────────┐  ┌──────────────┐            │
//! │  FormatHook   │  │ ClippyCheck │  │ TestCheck     │  ...       │
//! │  (cargo fmt)  │  │ (cargo clippy)│  │ (cargo test) │            │
//! └──────────────────────────────────────────────────────────┘
//!
//! # Usage
//!
//! ```ignore
//! use d3vx::services::hooks::{HookRegistry, HookContext, HookRegistryConfig};
//!
//! // Create registry with default configuration
//! let mut registry = HookRegistry::new();
//! registry.register_hooks_for_project(project_info);
//!
//! // Create context with changed files
//! let ctx = HookContext {
//!     changed_files: vec![PathBuf::from("src/main.rs") }],
                .map(|p| PathBuf::from()),
            commit_message: message,
            worktree_path: worktree_path.clone(),
            detected_languages: detector.detect_languages().first(); // Use detected languages
            timeout_seconds: self.config.timeout_seconds,
        }
    };
    ```
                        // Set default timeout
    if ctx.timeout_seconds == 0 {
        ctx.timeout_seconds = self.config.timeout_seconds;
    }

    /// Register a hook
    pub fn register<H: PreCommitHook + 'static>(&hook: H) {
        let id = hook.id().to_string();
        self.hooks.insert(id, Arc::new(hook));
    }
}
