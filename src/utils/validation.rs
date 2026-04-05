//! Validation utilities

use regex::Regex;
use std::path::Path;

/// Validate a file path is safe (no path traversal)
pub fn validate_safe_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // Check for path traversal attempts
    if path_str.contains("..") {
        return false;
    }

    // Check for null bytes
    if path_str.contains('\0') {
        return false;
    }

    true
}

/// Validate a command is safe for execution
pub fn validate_command(command: &str) -> bool {
    // Check for obviously dangerous patterns
    let dangerous = [
        "rm -rf /",
        "rm -rf /*",
        ":(){ :|:& };:", // Fork bomb
        "dd if=/dev/zero of=/dev/sda",
        "mkfs.ext4 /dev/sda",
    ];

    for pattern in dangerous {
        if command.contains(pattern) {
            return false;
        }
    }

    true
}

/// Validate a glob pattern is reasonable
pub fn validate_glob_pattern(pattern: &str) -> bool {
    // Reject patterns that might be too broad
    let too_broad = [
        "**/**/**", // Nested wildcards
    ];

    for reject in too_broad {
        if pattern.contains(reject) {
            return false;
        }
    }

    true
}

/// Validate an email address (basic check)
pub fn validate_email(email: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    re.is_match(email)
}

/// Validate a URL (basic check)
pub fn validate_url(url: &str) -> bool {
    // Must start with http:// or https://
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }

    // Should have a hostname
    if url.len() < 10 {
        return false;
    }

    true
}

/// Validate a model name is known
pub fn validate_model_name(name: &str) -> bool {
    let valid_prefixes = [
        "claude-",
        "gpt-",
        "gemini-",
        "llama",
        "mistral",
        "deepseek-",
        "qwen",
    ];

    for prefix in valid_prefixes {
        if name.to_lowercase().starts_with(prefix) {
            return true;
        }
    }

    // Also allow custom model names
    !name.is_empty()
}

/// Sanitize a string for safe display
pub fn sanitize_for_display(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// Sanitize a filename
pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_safe_path() {
        assert!(validate_safe_path(Path::new("/safe/path")));
        assert!(!validate_safe_path(Path::new("../etc/passwd")));
        assert!(!validate_safe_path(Path::new("/path/../../etc")));
    }

    #[test]
    fn test_validate_command() {
        assert!(validate_command("ls -la"));
        assert!(!validate_command("rm -rf /"));
        assert!(!validate_command("dd if=/dev/zero of=/dev/sda"));
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("test@example.com"));
        assert!(!validate_email("not-an-email"));
        assert!(!validate_email("@nodomain"));
    }

    #[test]
    fn test_validate_url() {
        assert!(validate_url("https://example.com"));
        assert!(validate_url("http://localhost:8080"));
        assert!(!validate_url("ftp://example.com"));
        assert!(!validate_url("not-a-url"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test.txt"), "test.txt");
        assert_eq!(sanitize_filename("test<script>.txt"), "testscript.txt");
        assert_eq!(
            sanitize_filename("normal_file-123.md"),
            "normal_file-123.md"
        );
    }
}
