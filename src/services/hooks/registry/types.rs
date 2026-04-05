//! Hook registry for managing and running pre-commit hooks
//!
//! The registry manages a collection of hooks and runs them in sequence.
//! It automatically detects the project type and configures hooks accordingly.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::detector::ProjectDetector;
use super::hooks::{FormatHook, LintHook, TestHook};
use super::security_check::SecurityHook;
use super::traits::{HookCategory, HookContext, HookError, HookResult, PreCommitHook};

/// Result of running all hooks
#[derive(Debug, Clone)]
pub struct HooksRunResult {
    /// Overall success (all hooks passed or skipped)
    pub success: bool,
    /// Individual hook results
    pub results: HashMap<String, HookRunInfo>,
    /// Total execution time in milliseconds
    pub total_duration_ms: u64,
}

/// Information about a single hook run
#[derive(Debug, Clone)]
pub struct HookRunInfo {
    /// Hook ID
    pub id: String,
    /// Hook name
    pub name: String,
    /// Hook category
    pub category: HookCategory,
    /// Hook result
    pub result: HookResult,
    /// Execution time in milliseconds
    pub duration_ms: u64,
    /// Error message if the hook errored (not failed)
    pub error: Option<String>,
}

impl HookRunInfo {
    /// Check if this hook run was successful
    pub fn is_success(&self) -> bool {
        self.error.is_none() && self.result.is_success()
    }
}

/// Configuration for the hook registry
#[derive(Debug, Clone)]
pub struct HookRegistryConfig {
    /// Enable pre-commit hooks
    pub enabled: bool,
    /// Enable format check
    pub format: bool,
    /// Enable lint check
    pub lint: bool,
    /// Enable test check
    pub test: bool,
    /// Enable security/secret detection
    pub security: bool,
    /// Skip hooks if commit message indicates WIP
    pub skip_if_wip: bool,
    /// Timeout for hooks in seconds
    pub timeout_seconds: u64,
}

impl Default for HookRegistryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: true,
            lint: true,
            test: true,
            security: true,
            skip_if_wip: true,
            timeout_seconds: 60,
        }
    }
}

/// Registry for managing pre-commit hooks
pub struct HookRegistry {
    /// Registered hooks by ID
    hooks: HashMap<String, Arc<dyn PreCommitHook>>,
    /// Configuration
    config: HookRegistryConfig,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

