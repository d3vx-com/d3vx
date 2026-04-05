//! Pre-commit hook traits and types
//!
//! This module defines the core types for the language-agnostic pre-commit hook system.

use std::path::PathBuf;
use thiserror::Error;

/// Supported programming languages/ecosystems
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    Java,
    Ruby,
    Php,
    CSharp,
    Unknown,
}

impl Language {
    /// Get the display name of the language
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::Rust => "Rust",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Python => "Python",
            Language::Go => "Go",
            Language::Java => "Java",
            Language::Ruby => "Ruby",
            Language::Php => "PHP",
            Language::CSharp => "C#",
            Language::Unknown => "Unknown",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Hook category/type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookCategory {
    /// Code formatting check
    Format,
    /// Linting/static analysis
    Lint,
    /// Test execution
    Test,
    /// Security/secret detection
    Security,
}

impl HookCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            HookCategory::Format => "Format",
            HookCategory::Lint => "Lint",
            HookCategory::Test => "Test",
            HookCategory::Security => "Security",
        }
    }
}

/// Context provided to pre-commit hooks
#[derive(Debug, Clone)]
pub struct HookContext {
    /// Files that have changed (staged or in working tree)
    pub changed_files: Vec<PathBuf>,
    /// Proposed commit message
    pub commit_message: String,
    /// Path to the worktree/repository root
    pub worktree_path: PathBuf,
    /// Detected languages in the project
    pub detected_languages: Vec<Language>,
    /// Timeout in seconds for hook execution
    pub timeout_seconds: u64,
}

impl Default for HookContext {
    fn default() -> Self {
        Self {
            changed_files: Vec::new(),
            commit_message: String::new(),
            worktree_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            detected_languages: Vec::new(),
            timeout_seconds: 60,
        }
    }
}

/// Result of running a pre-commit hook
#[derive(Debug, Clone)]
pub enum HookResult {
    /// Hook passed successfully
    Pass,
    /// Hook failed with an error message
    Fail(String),
    /// Hook was skipped (e.g., no relevant files, not applicable)
    Skip(String),
}

impl HookResult {
    /// Check if the result indicates success (Pass or Skip)
    pub fn is_success(&self) -> bool {
        matches!(self, HookResult::Pass | HookResult::Skip(_))
    }

    /// Get the error message if failed
    pub fn error_message(&self) -> Option<&str> {
        match self {
            HookResult::Fail(msg) => Some(msg),
            _ => None,
        }
    }

    /// Get the skip reason if skipped
    pub fn skip_reason(&self) -> Option<&str> {
        match self {
            HookResult::Skip(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Error type for hook operations
#[derive(Debug, Error)]
pub enum HookError {
    #[error("Hook execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Hook timed out after {0} seconds")]
    Timeout(u64),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Tool not found: {0}. Please install it to use this hook.")]
    ToolNotFound(String),

    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// Trait for pre-commit hooks
pub trait PreCommitHook: Send + Sync {
    /// Get the unique identifier for this hook
    fn id(&self) -> &str;

    /// Get the human-readable name of this hook
    fn name(&self) -> &str;

    /// Get the category of this hook
    fn category(&self) -> HookCategory;

    /// Get the language this hook applies to (None for language-agnostic hooks)
    fn language(&self) -> Option<Language>;

    /// Check if this hook is applicable given the context
    fn is_applicable(&self, ctx: &HookContext) -> bool {
        // Default: applicable if language matches or hook is language-agnostic
        match self.language() {
            None => true,
            Some(lang) => ctx.detected_languages.contains(&lang),
        }
    }

    /// Run the hook with the given context
    fn run(&self, ctx: &HookContext) -> Result<HookResult, HookError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_result_is_success() {
        assert!(HookResult::Pass.is_success());
        assert!(HookResult::Skip("reason".to_string()).is_success());
        assert!(!HookResult::Fail("error".to_string()).is_success());
    }

    #[test]
    fn test_hook_result_messages() {
        assert_eq!(
            HookResult::Fail("test error".to_string()).error_message(),
            Some("test error")
        );
        assert_eq!(HookResult::Pass.error_message(), None);

        assert_eq!(
            HookResult::Skip("no files".to_string()).skip_reason(),
            Some("no files")
        );
        assert_eq!(HookResult::Pass.skip_reason(), None);
    }

    #[test]
    fn test_language_display() {
        assert_eq!(Language::Rust.to_string(), "Rust");
        assert_eq!(Language::JavaScript.to_string(), "JavaScript");
        assert_eq!(Language::Python.to_string(), "Python");
    }

    #[test]
    fn test_hook_context_default() {
        let ctx = HookContext::default();
        assert!(ctx.changed_files.is_empty());
        assert!(ctx.commit_message.is_empty());
        assert!(ctx.detected_languages.is_empty());
        assert_eq!(ctx.timeout_seconds, 60);
    }
}
