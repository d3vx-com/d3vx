//! String utilities

// ============================================================================
// COMMON CONSTANTS - Reduces repeated string allocations
// ============================================================================

/// Default working directory when none is specified
pub const CURRENT_DIR: &str = ".";

/// Default worktrees directory name
pub const WORKTREES_DIR: &str = ".d3vx-worktrees";

/// Default memory directory name
pub const MEMORY_DIR: &str = ".d3vx/memory";

/// Default skills directory name
pub const SKILLS_DIR: &str = ".d3vx/skills";

/// Get current directory or default
#[inline]
pub fn current_dir_or_default(dir: Option<&str>) -> &str {
    dir.unwrap_or(CURRENT_DIR)
}

// ============================================================================
// STRING UTILITIES
// ============================================================================

/// Truncate a string to a maximum length
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Capitalize first letter
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

/// Convert snake_case to camelCase
pub fn snake_to_camel(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }

    result
}

/// Convert camelCase to snake_case
pub fn camel_to_snake(s: &str) -> String {
    let mut result = String::new();

    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }

    result
}

/// Strip ANSI color codes
pub fn strip_ansi(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    re.replace_all(s, "").to_string()
}

/// Check if string contains only ASCII
pub fn is_ascii(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii())
}

/// Parse a size string like "10MB" or "1GB"
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let multiplier: u64;
    let number_part: &str;

    if s.ends_with("KB") {
        multiplier = 1024;
        number_part = &s[..s.len() - 2];
    } else if s.ends_with("MB") {
        multiplier = 1024 * 1024;
        number_part = &s[..s.len() - 2];
    } else if s.ends_with("GB") {
        multiplier = 1024 * 1024 * 1024;
        number_part = &s[..s.len() - 2];
    } else if s.ends_with("K") {
        multiplier = 1000;
        number_part = &s[..s.len() - 1];
    } else if s.ends_with("M") {
        multiplier = 1_000_000;
        number_part = &s[..s.len() - 1];
    } else if s.ends_with("G") {
        multiplier = 1_000_000_000;
        number_part = &s[..s.len() - 1];
    } else if s.ends_with("B") {
        multiplier = 1;
        number_part = &s[..s.len() - 1];
    } else {
        multiplier = 1;
        number_part = s;
    }

    number_part
        .trim()
        .parse::<f64>()
        .ok()
        .map(|n| (n * multiplier as f64) as u64)
}

/// Format bytes as human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize("HELLO"), "HELLO");
        assert_eq!(capitalize(""), "");
    }

    #[test]
    fn test_snake_to_camel() {
        assert_eq!(snake_to_camel("hello_world"), "helloWorld");
        assert_eq!(snake_to_camel("my_function_name"), "myFunctionName");
    }

    #[test]
    fn test_camel_to_snake() {
        assert_eq!(camel_to_snake("helloWorld"), "hello_world");
        assert_eq!(camel_to_snake("MyFunction"), "my_function");
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1KB"), Some(1024));
        assert_eq!(parse_size("1MB"), Some(1024 * 1024));
        assert_eq!(parse_size("1GB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("100B"), Some(100));
        assert_eq!(parse_size("invalid"), None);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500B");
        assert_eq!(format_bytes(1024), "1.00KB");
        assert_eq!(format_bytes(1048576), "1.00MB");
    }

    // ── Additional edge cases ─────────────────────────────────

    #[test]
    fn test_truncate_exact_boundary() {
        // String exactly at max_len
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_zero_max() {
        // Zero max length, should truncate all content except "..."
        assert_eq!(truncate("hello", 3), "...");
    }

    #[test]
    fn test_truncate_just_under() {
        assert_eq!(truncate("abcde", 5), "abcde");
    }

    #[test]
    fn test_current_dir_or_default() {
        assert_eq!(current_dir_or_default(Some("/tmp")), "/tmp");
        assert_eq!(current_dir_or_default(None), ".");
    }

    #[test]
    fn test_capitalize_single_char() {
        assert_eq!(capitalize("h"), "H");
        assert_eq!(capitalize("H"), "H");
    }

    #[test]
    fn test_snake_to_camel_leading_underscore() {
        assert_eq!(snake_to_camel("_private"), "Private");
    }

    #[test]
    fn test_snake_to_camel_no_underscore() {
        assert_eq!(snake_to_camel("hello"), "hello");
    }

    #[test]
    fn test_snake_to_camel_empty() {
        assert_eq!(snake_to_camel(""), "");
    }

    #[test]
    fn test_camel_to_snake_empty() {
        assert_eq!(camel_to_snake(""), "");
    }

    #[test]
    fn test_camel_to_snake_leading_uppercase() {
        assert_eq!(camel_to_snake("HelloWorld"), "hello_world");
    }

    #[test]
    fn test_parse_size_si_units() {
        assert_eq!(parse_size("1K"), Some(1000));
        assert_eq!(parse_size("1M"), Some(1_000_000));
        assert_eq!(parse_size("1G"), Some(1_000_000_000));
    }

    #[test]
    fn test_parse_size_plain_number() {
        assert_eq!(parse_size("42"), Some(42));
        assert_eq!(parse_size("0"), Some(0));
    }

    #[test]
    fn test_parse_size_float() {
        assert_eq!(parse_size("1.5KB"), Some(1536));
    }

    #[test]
    fn test_parse_size_whitespace_trim() {
        assert_eq!(parse_size(" 1KB "), Some(1024));
    }

    #[test]
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0B");
    }

    #[test]
    fn test_format_bytes_gb() {
        assert_eq!(format_bytes(1073741824), "1.00GB");
    }

    #[test]
    fn test_is_ascii() {
        assert!(is_ascii("Hello"));
        assert!(!is_ascii("你好"));
        assert!(is_ascii(""));
    }

    #[test]
    fn test_strip_ansi_no_codes() {
        assert_eq!(strip_ansi("plain text"), "plain text");
    }

    #[test]
    fn test_strip_ansi_single() {
        assert_eq!(strip_ansi("\x1b[31mred"), "red");
    }

    #[test]
    fn test_strip_ansi_multiple() {
        assert_eq!(strip_ansi("\x1b[1m\x1b[32mgreen\x1b[0m"), "green");
    }
}
