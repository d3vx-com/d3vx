//! Diff Utility
//!
//! Generates unified diffs between two strings using the `similar` crate.

use similar::TextDiff;

/// Generate a unified diff between two strings
pub fn generate_unified_diff(file_path: &str, old_content: &str, new_content: &str) -> String {
    let diff = TextDiff::from_lines(old_content, new_content);

    diff.unified_diff()
        .context_radius(3)
        .header(file_path, file_path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_unified_diff() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline2 edited\nline3\nline4\n";
        let diff = generate_unified_diff("test.txt", old, new);

        assert!(diff.contains("--- test.txt"));
        assert!(diff.contains("+++ test.txt"));
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+line2 edited"));
        assert!(diff.contains("+line4"));
    }
}
