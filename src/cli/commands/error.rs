//! User-Facing Error Types
//!
//! Wraps raw errors into structured, actionable messages.
//! Every error a user sees should answer:
//!   1. What went wrong  (message)
//!   2. Why it happened  (context, optional)
//!   3. How to fix it    (hint)

use std::fmt;

/// A structured, user-facing error with a hint and optional docs link.
#[derive(Debug)]
pub struct AppError {
    /// Plain-English description of what went wrong.
    pub message: String,
    /// Actionable next step the user should take.
    pub hint: String,
    /// Optional URL to relevant documentation.
    pub docs_url: Option<&'static str>,
}

impl AppError {
    pub fn new(
        message: impl Into<String>,
        hint: impl Into<String>,
        docs_url: Option<&'static str>,
    ) -> Self {
        Self {
            message: message.into(),
            hint: hint.into(),
            docs_url,
        }
    }

    // ── Common constructors ────────────────────────────────────────────────

    pub fn missing_api_key(provider: &str, env_var: &str) -> Self {
        Self::new(
            format!("No API key found for provider '{provider}'"),
            format!(
                "Set the environment variable:\n  export {env_var}=\"your-key-here\"\n\
                 Then run `d3vx doctor` to verify."
            ),
            Some("https://github.com/d3vx/d3vx-terminal#multi-provider-llm-support"),
        )
    }

    pub fn not_a_git_repo() -> Self {
        Self::new(
            "Current directory is not a git repository",
            "Initialize a repo first:\n  git init && git commit --allow-empty -m \"init\"\n\
             Background tasks (--vex) require git worktree support.",
            None,
        )
    }

    pub fn provider_connection_failed(provider: &str, detail: &str) -> Self {
        Self::new(
            format!("Could not connect to provider '{provider}': {detail}"),
            "Check that your API key is valid and you have network access.\n\
             Run `d3vx doctor` for a full environment check.",
            None,
        )
    }

    pub fn config_write_failed(path: &str, detail: &str) -> Self {
        Self::new(
            format!("Could not write config to '{path}': {detail}"),
            "Check that the directory exists and you have write permission.",
            None,
        )
    }

    pub fn setup_required() -> Self {
        Self::new(
            "d3vx is not configured yet",
            "Run `d3vx setup` to walk through provider selection and API key setup.",
            None,
        )
    }

    /// Convert to an `anyhow` error so callers can use `?` normally.
    pub fn into_anyhow(self) -> anyhow::Error {
        anyhow::anyhow!("{}", self)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error: {}\n\nHint: {}", self.message, self.hint)?;
        if let Some(url) = self.docs_url {
            write!(f, "\nDocs: {}", url)?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_includes_hint() {
        let err = AppError::missing_api_key("anthropic", "ANTHROPIC_API_KEY");
        let s = err.to_string();
        assert!(s.contains("Error:"), "should contain 'Error:'");
        assert!(s.contains("Hint:"), "should contain 'Hint:'");
        assert!(s.contains("ANTHROPIC_API_KEY"), "should include env var name");
    }

    #[test]
    fn test_display_includes_docs_url_when_present() {
        let err = AppError::missing_api_key("anthropic", "ANTHROPIC_API_KEY");
        assert!(err.docs_url.is_some());
        assert!(err.to_string().contains("Docs:"));
    }

    #[test]
    fn test_display_no_docs_url_when_absent() {
        let err = AppError::not_a_git_repo();
        assert!(err.docs_url.is_none());
        assert!(!err.to_string().contains("Docs:"));
    }

    #[test]
    fn test_into_anyhow_conversion() {
        let err = AppError::setup_required();
        let anyhow_err: anyhow::Error = err.into();
        assert!(anyhow_err.to_string().contains("not configured"));
    }
}
