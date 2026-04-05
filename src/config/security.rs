//! Security configuration for d3vx
//!
//! Handles command blocklists and other security-related settings.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use tracing::{debug, warn};

/// Security-related errors
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Command blocked by security policy: {0}")]
    Blocked(String),

    #[error("Failed to load security config: {0}")]
    ConfigLoad(String),

    #[error("Invalid regex pattern '{0}': {1}")]
    InvalidRegex(String, String),
}

/// Bash tool specific configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct BashToolConfig {
    /// Regex patterns for commands that are never allowed to execute
    #[serde(default)]
    pub blocklist: Vec<String>,
}

/// Top-level security configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct SecurityConfig {
    /// Bash tool security settings
    #[serde(default)]
    pub bash_tool: BashToolConfig,
}

impl SecurityConfig {
    /// Load security configuration from a TOML file
    ///
    /// # Arguments
    /// * `path` - Path to the security.toml file
    ///
    /// # Returns
    /// The loaded security configuration, or default if file doesn't exist
    pub fn load_from_file(path: &str) -> Result<Self, SecurityError> {
        let path = Path::new(path);

        if !path.exists() {
            debug!(
                "Security config file not found at {}, using defaults",
                path.display()
            );
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path).map_err(|e| {
            SecurityError::ConfigLoad(format!("Failed to read {}: {}", path.display(), e))
        })?;

        let config: SecurityConfig = toml::from_str(&content).map_err(|e| {
            SecurityError::ConfigLoad(format!("Failed to parse {}: {}", path.display(), e))
        })?;

        debug!("Loaded security config from {}", path.display());
        Ok(config)
    }

    /// Get the default security configuration path for a project
    pub fn get_default_path(project_root: &str) -> String {
        format!("{}/.d3vx/security.toml", project_root)
    }

    /// Compile blocklist patterns into regex objects
    ///
    /// # Returns
    /// A vector of compiled regex patterns, or an error if any pattern is invalid
    pub fn compile_blocklist(&self) -> Result<Vec<Regex>, SecurityError> {
        let mut compiled = Vec::new();

        for pattern in &self.bash_tool.blocklist {
            match Regex::new(pattern) {
                Ok(regex) => {
                    compiled.push(regex);
                    debug!("Compiled blocklist pattern: {}", pattern);
                }
                Err(e) => {
                    warn!("Invalid blocklist regex pattern '{}': {}", pattern, e);
                    return Err(SecurityError::InvalidRegex(pattern.clone(), e.to_string()));
                }
            }
        }

        Ok(compiled)
    }

    /// Check if a command matches any blocklist pattern
    ///
    /// # Arguments
    /// * `command` - The command to check
    /// * `compiled_patterns` - Pre-compiled regex patterns
    ///
    /// # Returns
    /// Ok(()) if command is allowed, Err(SecurityError::Blocked) if blocked
    pub fn check_command(command: &str, compiled_patterns: &[Regex]) -> Result<(), SecurityError> {
        for pattern in compiled_patterns {
            if pattern.is_match(command) {
                let pattern_str = pattern.to_string();
                debug!("Command '{}' blocked by pattern: {}", command, pattern_str);
                return Err(SecurityError::Blocked(pattern_str));
            }
        }
        Ok(())
    }
}

/// Default blocklist patterns for security
pub fn default_blocklist() -> Vec<String> {
    vec![
        r"^rm\s+-rf\s+.*$".to_string(),
        r"^git\s+push\s+.*--force.*$".to_string(),
        r"^git\s+reset\s+--hard.*$".to_string(),
        r"^sudo\s+.*$".to_string(),
        r"^chmod\s+.*$".to_string(),
        r"^chown\s+.*$".to_string(),
        r"^dd\s+if=.*of=/dev/.*$".to_string(),
        r"^:\(\)\{\s*:\|:\s*&\s*\};\s*:".to_string(), // Fork bomb
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_security_config() {
        let config = SecurityConfig::default();
        assert!(config.bash_tool.blocklist.is_empty());
    }

    #[test]
    fn test_load_from_nonexistent_file() {
        let result = SecurityConfig::load_from_file("/nonexistent/path/security.toml");
        assert!(result.is_ok());
        assert!(result.unwrap().bash_tool.blocklist.is_empty());
    }

    #[test]
    fn test_load_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("security.toml");

        let content = r#"
[bash_tool]
blocklist = [
    "^rm -rf .*$",
    "^sudo .*$",
]
"#;
        std::fs::write(&path, content).unwrap();

        let config = SecurityConfig::load_from_file(&path.to_string_lossy()).unwrap();
        assert_eq!(config.bash_tool.blocklist.len(), 2);
        assert!(config
            .bash_tool
            .blocklist
            .contains(&"^rm -rf .*$".to_string()));
    }

    #[test]
    fn test_compile_blocklist_valid() {
        let config = SecurityConfig {
            bash_tool: BashToolConfig {
                blocklist: vec![r"^sudo.*$".to_string(), r"^rm\s+-rf.*$".to_string()],
            },
        };

        let compiled = config.compile_blocklist().unwrap();
        assert_eq!(compiled.len(), 2);
    }

    #[test]
    fn test_compile_blocklist_invalid() {
        let config = SecurityConfig {
            bash_tool: BashToolConfig {
                blocklist: vec![r"[invalid(".to_string()],
            },
        };

        let result = config.compile_blocklist();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::InvalidRegex(_, _)
        ));
    }

    #[test]
    fn test_check_command_allowed() {
        let patterns = vec![
            Regex::new(r"^sudo.*$").unwrap(),
            Regex::new(r"^rm\s+-rf.*$").unwrap(),
        ];

        let result = SecurityConfig::check_command("ls -la", &patterns);
        assert!(result.is_ok());

        let result = SecurityConfig::check_command("echo 'hello'", &patterns);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_command_blocked() {
        let patterns = vec![
            Regex::new(r"^sudo.*$").unwrap(),
            Regex::new(r"^rm\s+-rf.*$").unwrap(),
        ];

        let result = SecurityConfig::check_command("sudo rm -rf /", &patterns);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SecurityError::Blocked(_)));

        let result = SecurityConfig::check_command("rm -rf /home/user", &patterns);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_command_partial_match() {
        // Test that patterns match from start of command
        let patterns = vec![Regex::new(r"^sudo.*$").unwrap()];

        // Should NOT block - "sudo" is not at the start
        let result = SecurityConfig::check_command("echo sudo", &patterns);
        assert!(result.is_ok());

        // Should block - "sudo" is at the start
        let result = SecurityConfig::check_command("sudo ls", &patterns);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_command_empty_patterns() {
        let patterns: Vec<Regex> = vec![];

        let result = SecurityConfig::check_command("rm -rf /", &patterns);
        assert!(result.is_ok());
    }

    #[test]
    fn test_default_blocklist_patterns() {
        let blocklist = default_blocklist();

        // Verify common dangerous patterns are included
        assert!(blocklist
            .iter()
            .any(|p| p.contains("rm") && p.contains("-rf")));
        assert!(blocklist.iter().any(|p| p.contains("sudo")));
        assert!(blocklist.iter().any(|p| p.contains("chmod")));
    }
}
