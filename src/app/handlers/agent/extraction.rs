//! Text Extraction Utilities
//!
//! Helper functions for extracting structured information from agent outputs.

/// Get status icon for a given status string
pub fn status_icon(status: &str) -> &'static str {
    match status {
        "Completed" => "✅",
        "Failed" => "❌",
        "Cancelled" => "⏭️",
        "Pending" => "⏳",
        "Running" => "🔄",
        _ => "❓",
    }
}

/// Extract file paths from result text
pub fn extract_files_changed(result: &str) -> Vec<String> {
    let mut files = Vec::new();

    let patterns = [
        "file:",
        "files:",
        "changed:",
        "modified:",
        "created:",
        "updated:",
        "src/",
        "lib/",
        "bin/",
        "tests/",
        "docs/",
    ];

    for line in result.lines() {
        let line_lower = line.to_lowercase();
        if patterns.iter().any(|p| line_lower.contains(p)) {
            let parts: Vec<&str> = line
                .split(|c: char| c == ',' || c == '\n' || c == ' ')
                .filter(|s| {
                    let s_lower = s.to_lowercase();
                    (s_lower.starts_with("src/")
                        || s_lower.starts_with("lib/")
                        || s_lower.starts_with("bin/")
                        || s_lower.starts_with("tests/")
                        || s_lower.starts_with("docs/")
                        || s_lower.contains(".rs")
                        || s_lower.contains(".ts")
                        || s_lower.contains(".js")
                        || s_lower.contains(".py")
                        || s_lower.contains(".go"))
                        && s.len() > 3
                })
                .collect();

            for part in parts {
                let trimmed = part.trim_matches(|c| "`,* \n\t".contains(c));
                if !trimmed.is_empty() && !files.iter().any(|f: &String| f.contains(trimmed)) {
                    files.push(trimmed.to_string());
                }
            }
        }
    }

    files.truncate(10);
    files
}

/// Extract decisions from result text
pub fn extract_decisions(result: &str) -> Vec<String> {
    let mut decisions = Vec::new();
    let decision_indicators = [
        "decision:",
        "decided:",
        "chose:",
        "chosen:",
        "selected:",
        "implemented:",
        "used:",
        "adopted:",
        "approach:",
    ];

    for line in result.lines() {
        let line_lower = line.to_lowercase();
        if decision_indicators.iter().any(|d| line_lower.contains(d)) {
            let trimmed = line.trim();
            if !trimmed.is_empty() && trimmed.len() < 300 {
                let clean = trimmed
                    .trim_start_matches(|c: char| !c.is_alphanumeric())
                    .to_string();
                if !clean.is_empty()
                    && !decisions
                        .iter()
                        .any(|d: &String| d.contains(&clean[..clean.len().min(50)]))
                {
                    decisions.push(clean);
                }
            }
        }
    }

    decisions.truncate(5);
    decisions
}

/// Extract issues/warnings from result text
pub fn extract_issues(result: &str) -> Vec<String> {
    let mut issues = Vec::new();
    let issue_indicators = ["warning:", "error:", "issue:", "failed:", "warning"];

    for line in result.lines() {
        let line_lower = line.to_lowercase();
        if issue_indicators.iter().any(|i| line_lower.contains(i)) {
            let trimmed = line.trim();
            if !trimmed.is_empty() && trimmed.len() < 200 {
                issues.push(trimmed.to_string());
            }
        }
    }

    issues.truncate(5);
    issues
}

/// Extract code blocks from result text with token limit
pub fn extract_code_blocks(result: &str, max_tokens: usize) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut current_block = String::new();

    let avg_chars_per_token = 4;
    let max_chars = max_tokens * avg_chars_per_token;
    let mut total_chars = 0;

    for line in result.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            if in_block {
                if !current_block.is_empty() && current_block.len() < 3000 {
                    blocks.push(current_block.clone());
                    total_chars += current_block.len();
                }
                current_block.clear();
                in_block = false;
            } else {
                in_block = true;
            }
        } else if in_block {
            current_block.push_str(line);
            current_block.push('\n');
        } else if trimmed.len() > 50
            && (trimmed.contains("fn ")
                || trimmed.contains("struct ")
                || trimmed.contains("impl ")
                || trimmed.contains("class ")
                || trimmed.contains("func ")
                || trimmed.contains("def ")
                || trimmed.contains("pub ")
                || trimmed.contains("async "))
            && !trimmed.contains("...")
            && !blocks.iter().any(|b| b.contains(trimmed))
        {
            if total_chars + trimmed.len() < max_chars {
                blocks.push(trimmed.to_string());
                total_chars += trimmed.len();
            }
        }

        if total_chars >= max_chars && blocks.len() >= 2 {
            break;
        }
    }

    if in_block && !current_block.is_empty() && current_block.len() < 3000 {
        blocks.push(current_block);
    }

    blocks.truncate(3);
    blocks
}

/// Extract narrative text from result with token limit
pub fn extract_narrative(result: &str, max_tokens: usize) -> String {
    let avg_chars_per_token = 4;
    let max_chars = max_tokens * avg_chars_per_token;

    let lines: Vec<&str> = result
        .lines()
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("```")
                && !trimmed.starts_with("# ")
                && trimmed.len() > 20
                && !trimmed
                    .chars()
                    .all(|c| c == '-' || c == '*' || c == ' ' || c == '|')
        })
        .collect();

    let mut narrative = Vec::new();
    let mut char_count = 0;

    for line in lines {
        if char_count + line.len() > max_chars {
            break;
        }
        narrative.push(line);
        char_count += line.len();
    }

    narrative.join("\n")
}

/// Truncate text at sentence boundary
pub fn truncate_at_sentence_boundary(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }

    let truncated = &text[..max_chars];

    // Find last sentence boundary
    if let Some(pos) = truncated.rfind(|c| c == '.' || c == '!' || c == '?') {
        truncated[..=pos].to_string()
    } else if let Some(pos) = truncated.rfind(|c: char| c.is_whitespace()) {
        truncated[..pos].to_string()
    } else {
        truncated.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_icon() {
        assert_eq!(status_icon("Completed"), "✅");
        assert_eq!(status_icon("Failed"), "❌");
        assert_eq!(status_icon("Unknown"), "❓");
    }

    #[test]
    fn test_extract_files_changed() {
        let result = "Changed files:\nsrc/main.rs\nlib/utils.rs";
        let files = extract_files_changed(result);
        assert!(!files.is_empty());
    }

    #[test]
    fn test_extract_decisions() {
        let result = "Decision: Use async/await pattern\nImplemented: Fast processing";
        let decisions = extract_decisions(result);
        assert!(!decisions.is_empty());
    }

    #[test]
    fn test_extract_issues() {
        let result = "Warning: deprecated API\nError: file not found";
        let issues = extract_issues(result);
        assert!(!issues.is_empty());
    }

    #[test]
    fn test_truncate_at_sentence_boundary() {
        let text = "This is a sentence. And another one here.";
        let truncated = truncate_at_sentence_boundary(text, 25);
        assert!(truncated.ends_with('.'));
    }
}
