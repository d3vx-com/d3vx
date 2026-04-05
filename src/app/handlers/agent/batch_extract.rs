//! Batch Result Extraction Methods
//!
//! App methods for extracting structured information from parallel
//! child task results: files, decisions, issues, code blocks, narrative.

use crate::app::App;

impl App {
    pub(super) fn extract_files_changed(result: &str) -> Vec<String> {
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

    pub(super) fn extract_decisions(result: &str) -> Vec<String> {
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

    pub(super) fn extract_issues(result: &str) -> Vec<String> {
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

    pub(super) fn extract_code_blocks(result: &str, max_tokens: usize) -> Vec<String> {
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
                        total_chars += current_block.len();
                        blocks.push(std::mem::take(&mut current_block));
                    } else {
                        current_block.clear();
                    }
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

    pub(super) fn extract_narrative(result: &str, max_tokens: usize) -> String {
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
            narrative.push(line.trim());
            char_count += line.len();
        }

        let result = narrative.join(" ");
        Self::truncate_at_sentence_boundary(&result, max_chars)
    }

    pub(super) fn truncate_at_sentence_boundary(text: &str, max_chars: usize) -> String {
        if text.len() <= max_chars {
            return text.to_string();
        }

        let truncated = &text[..max_chars];

        if let Some(last_period) = truncated.rfind('.') {
            if last_period > max_chars - 100 {
                return format!("{}.", &truncated[..last_period]);
            }
        }

        if let Some(last_newline) = truncated.rfind('\n') {
            if last_newline > max_chars - 100 {
                return truncated[..last_newline].to_string();
            }
        }

        format!(
            "{}...",
            truncated.trim_end_matches(|c: char| !c.is_alphanumeric() && c != ' ')
        )
    }
}
