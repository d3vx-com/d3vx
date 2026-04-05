//! Security check hook implementation
//!
//! Performs basic secret detection to prevent committing sensitive data.
//! This hook is language-agnostic.

use std::fs;
use std::path::Path;
use regex::Regex;
use tracing::{debug, warn};

use super::traits::{HookCategory, HookContext, HookError, HookResult, Language, PreCommitHook};

/// Patterns that might indicate secrets
const SECRET_PATTERNS: &[(&str, &str)] = &[
    // AWS Access Key ID
    ("aws_access_key", r"(?i)AKIA[0-9A-Z]{16}"),
    // AWS Secret Access Key
    ("aws_secret_key", r"(?i)aws(.{0,20})?['\"][0-9a-zA-Z/+=]{40}['\"]"),
    // GitHub Personal Access Token
    ("github_token", r"(?i)ghp_[0-9a-zA-Z]{36}"),
    // GitHub OAuth Access Token
    ("github_oauth", r"(?i)gho_[0-9a-zA-Z]{36}"),
    // GitHub App Token
    ("github_app", r"(?i)(ghu|ghs)_[0-9a-zA-Z]{36}"),
    // Generic API key patterns
    ("api_key", r"(?i)(api[_-]?key|apikey|api[_-]?secret)['\"]?\s*[:=]\s*['\"]?[0-9a-zA-Z\-_]{20,}['\"]?"),
    // Private keys (RSA, etc.)
    ("private_key", r"-----BEGIN\s+(?:RSA\s+)?PRIVATE\s+KEY-----"),
    // Slack tokens
    ("slack_token", r"xox[baprs]-[0-9]{10,13}-[0-9]{10,13}-[a-zA-Z0-9]{24}"),
    // Stripe API keys
    ("stripe_key", r"(?i)sk_(live|test)_[0-9a-zA-Z]{24,}"),
    // Generic password in config
    ("password", r"(?i)(password|passwd|pwd)['\"]?\s*[:=]\s*['\"]?[^'\"]{8,}['\"]?"),
    // Database connection strings
    ("db_connection", r"(?i)(mysql|postgres|mongodb)://[^:]+:[^@]+@[^/]+"),
    // JWT secrets
    ("jwt_secret", r"(?i)jwt[_-]?secret['\"]?\s*[:=]\s*['\"]?[^'\"]{16,}['\"]?"),
];

/// Files to skip during secret scanning
const SKIP_PATTERNS: &[&str] = &[
    ".git/",
    "node_modules/",
    "target/",
    ".env.example",
    ".env.sample",
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
];

/// Detected secret information
#[derive(Debug, Clone)]
pub struct DetectedSecret {
    /// Type of secret detected
    pub secret_type: String,
    /// File path where the secret was found
    pub file_path: String,
    /// Line number where the secret was found
    pub line_number: usize,
    /// Snippet of the line (with secret redacted)
    pub snippet: String,
}

/// Hook that checks for secrets in changed files
pub struct SecurityHook {
    /// Compiled regex patterns
    patterns: Vec<(String, Regex)>,
}

impl SecurityHook {
    /// Create a new SecurityHook
    pub fn new() -> Self {
        let patterns = SECRET_PATTERNS
            .iter()
            .filter_map(|(name, pattern)| {
                Regex::new(pattern)
                    .map(|re| (name.to_string(), re))
                    .ok()
            })
            .collect();

        Self { patterns }
    }

    /// Check if a file should be skipped
    fn should_skip_file(&self, path: &str) -> bool {
        SKIP_PATTERNS.iter().any(|skip| path.contains(skip))
    }

    /// Redact a secret from a string
    fn redact_secret(&self, text: &str, pattern: &Regex) -> String {
        pattern.replace_all(text, "[REDACTED]").to_string()
    }

    /// Scan a single file for secrets
    fn scan_file(&self, path: &Path) -> Result<Vec<DetectedSecret>, HookError> {
        let mut secrets = Vec::new();
        let path_str = path.to_string_lossy();

        if self.should_skip_file(&path_str) {
            return Ok(secrets);
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Ok(secrets), // Skip binary files or unreadable files
        };

        for (line, line_content) in content.lines().enumerate() {
            for (secret_type, pattern) in &self.patterns {
                if pattern.is_match(line_content) {
                    secrets.push(DetectedSecret {
                        secret_type: secret_type.clone(),
                        file_path: path_str.to_string(),
                        line_number: line + 1,
                        snippet: self.redact_secret(line_content, pattern),
                    });
                }
            }
        }

        Ok(secrets)
    }

    /// Scan all changed files for secrets
    fn scan_files(&self, ctx: &HookContext) -> Result<Vec<DetectedSecret>, HookError> {
        let mut all_secrets = Vec::new();

        for file in &ctx.changed_files {
            if file.exists() && file.is_file() {
                let secrets = self.scan_file(file)?;
                all_secrets.extend(secrets);
            }
        }

        Ok(all_secrets)
    }
}

impl Default for SecurityHook {
    fn default() -> Self {
        Self::new()
    }
}

impl PreCommitHook for SecurityHook {
    fn id(&self) -> &str {
        "security"
    }

    fn name(&self) -> &str {
        "Security Check"
    }

    fn category(&self) -> HookCategory {
        HookCategory::Security
    }

    fn language(&self) -> Option<Language> {
        None // Language-agnostic
    }

    fn is_applicable(&self, ctx: &HookContext) -> bool {
        !ctx.changed_files.is_empty()
    }

    fn run(&self, ctx: &HookContext) -> Result<HookResult, HookError> {
        if ctx.changed_files.is_empty() {
            debug!("No files to scan for secrets");
            return Ok(HookResult::Skip("No changed files".to_string()));
        }

        debug!(files = ctx.changed_files.len(), "Scanning files for secrets");

        let secrets = self.scan_files(ctx)?;

        if secrets.is_empty() {
            debug!("No secrets detected");
            Ok(HookResult::Pass)
        } else {
            let mut message = String::from("Potential secrets detected in changed files:\n\n");

            for secret in &secrets {
                message.push_str(&format!(
                    "  - {} in {}:{}\n    {}\n",
                    secret.secret_type,
                    secret.file_path,
                    secret.line_number,
                    secret.snippet
                ));
            }

            message.push_str("\nPlease review and remove any actual secrets before committing.");
            message.push_str("\nIf these are false positives, consider adding patterns to your .d3vx/config.yml");

            warn!(secrets_found = secrets.len(), "Security check detected potential secrets");
            Ok(HookResult::Fail(message))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_context(files: Vec<&str>) -> HookContext {
        HookContext {
            changed_files: files.iter().map(PathBuf::from).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn test_security_hook_metadata() {
        let hook = SecurityHook::new();
        assert_eq!(hook.id(), "security");
        assert_eq!(hook.name(), "Security Check");
        assert_eq!(hook.category(), HookCategory::Security);
        assert_eq!(hook.language(), None);
    }

    #[test]
    fn test_security_hook_no_files() {
        let hook = SecurityHook::new();
        let ctx = make_context(vec![]);

        assert!(!hook.is_applicable(&ctx));
    }

    #[test]
    fn test_security_hook_with_files() {
        let hook = SecurityHook::new();
        let ctx = make_context(vec!["src/main.rs"]);

        assert!(hook.is_applicable(&ctx));
    }

    #[test]
    fn test_security_hook_should_skip() {
        let hook = SecurityHook::new();

        assert!(hook.should_skip_file(".git/config"));
        assert!(hook.should_skip_file("node_modules/package/index.js"));
        assert!(hook.should_skip_file("target/debug/main"));
        assert!(!hook.should_skip_file("src/main.rs"));
    }

    #[test]
    fn test_security_hook_detect_aws_key() {
        let hook = SecurityHook::new();

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("config.txt");
        fs::write(&file_path, "AWS_KEY=AKIAIOSFODNN7EXAMPLE\n").unwrap();

        let secrets = hook.scan_file(&file_path).unwrap();
        assert!(!secrets.is_empty());
        assert!(secrets[0].secret_type == "aws_access_key");
    }

    #[test]
    fn test_security_hook_detect_private_key() {
        let hook = SecurityHook::new();

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("key.pem");
        fs::write(&file_path, "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBgk\n").unwrap();

        let secrets = hook.scan_file(&file_path).unwrap();
        assert!(!secrets.is_empty());
        assert!(secrets[0].secret_type == "private_key");
    }

    #[test]
    fn test_security_hook_clean_file() {
        let hook = SecurityHook::new();

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("clean.rs");
        fs::write(&file_path, "fn main() {\n    println!(\"Hello\");\n}\n").unwrap();

        let secrets = hook.scan_file(&file_path).unwrap();
        assert!(secrets.is_empty());
    }
}
