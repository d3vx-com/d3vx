//! Syntax and quality checks for the auto-review hook.

use std::path::Path;

use super::types::{ReviewFinding, Severity};

// ---------------------------------------------------------------------------
// Supported extensions
// ---------------------------------------------------------------------------

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "h", "rb",
];

pub(super) fn is_supported_source_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Syntax checks
// ---------------------------------------------------------------------------

pub(super) fn check_syntax(file: &str, content: &str, findings: &mut Vec<ReviewFinding>) {
    let ext = Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs" | "go" | "c" | "cpp" | "h" | "java" => {
            check_brace_balance(file, content, findings, &['{', '}']);
            check_bracket_balance(file, content, findings);
        }
        "ts" | "tsx" | "js" | "jsx" | "rb" => {
            check_brace_balance(file, content, findings, &['{', '}']);
            check_bracket_balance(file, content, findings);
            check_paren_balance(file, content, findings);
        }
        "py" => {
            check_indentation_consistency(file, content, findings);
        }
        _ => {}
    }
}

fn check_brace_balance(
    file: &str,
    content: &str,
    findings: &mut Vec<ReviewFinding>,
    pair: &[char; 2],
) {
    let mut depth = 0usize;
    let mut last_bad_line = 0usize;

    for (i, line) in content.lines().enumerate() {
        let in_string = false; // simplified: does not track string state
        if in_string {
            continue;
        }
        for ch in line.chars() {
            if ch == pair[0] {
                depth += 1;
            } else if ch == pair[1] {
                depth = depth.saturating_sub(1);
            }
        }
        if depth == 0 {
            last_bad_line = 0;
        } else if last_bad_line == 0 {
            last_bad_line = i + 1;
        }
    }

    if depth > 0 {
        findings.push(ReviewFinding {
            severity: Severity::Error,
            file: file.to_string(),
            line: Some(last_bad_line),
            message: format!("Unmatched '{}' ({} unclosed)", pair[0], depth),
            source: "syntax".to_string(),
        });
    }
}

fn check_bracket_balance(file: &str, content: &str, findings: &mut Vec<ReviewFinding>) {
    let mut depth = 0usize;
    for line in content.lines() {
        for ch in line.chars() {
            if ch == '[' {
                depth += 1;
            } else if ch == ']' {
                depth = depth.saturating_sub(1);
            }
        }
    }
    if depth > 0 {
        findings.push(ReviewFinding {
            severity: Severity::Error,
            file: file.to_string(),
            line: None,
            message: format!("Unmatched '[' ({} unclosed)", depth),
            source: "syntax".to_string(),
        });
    }
}

fn check_paren_balance(file: &str, content: &str, findings: &mut Vec<ReviewFinding>) {
    let mut depth = 0usize;
    for line in content.lines() {
        for ch in line.chars() {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth = depth.saturating_sub(1);
            }
        }
    }
    if depth > 0 {
        findings.push(ReviewFinding {
            severity: Severity::Error,
            file: file.to_string(),
            line: None,
            message: format!("Unmatched '(' ({} unclosed)", depth),
            source: "syntax".to_string(),
        });
    }
}

fn check_indentation_consistency(file: &str, content: &str, findings: &mut Vec<ReviewFinding>) {
    let mut uses_spaces = false;
    let mut uses_tabs = false;

    for line in content.lines() {
        if line.starts_with("  ") {
            uses_spaces = true;
        }
        if line.starts_with('\t') {
            uses_tabs = true;
        }
    }

    if uses_spaces && uses_tabs {
        findings.push(ReviewFinding {
            severity: Severity::Warning,
            file: file.to_string(),
            line: None,
            message: "Mixed indentation (tabs and spaces)".to_string(),
            source: "syntax".to_string(),
        });
    }
}

// ---------------------------------------------------------------------------
// Style checks
// ---------------------------------------------------------------------------

const MAX_LINE_LENGTH: usize = 300;

pub(super) fn check_style(file: &str, content: &str, findings: &mut Vec<ReviewFinding>) {
    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;

        if line.len() > MAX_LINE_LENGTH {
            findings.push(ReviewFinding {
                severity: Severity::Warning,
                file: file.to_string(),
                line: Some(line_num),
                message: format!("Line too long ({} chars)", line.len()),
                source: "style".to_string(),
            });
        }

        if line.ends_with(' ') || line.ends_with('\t') {
            findings.push(ReviewFinding {
                severity: Severity::Info,
                file: file.to_string(),
                line: Some(line_num),
                message: "Trailing whitespace".to_string(),
                source: "style".to_string(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Marker checks (TODO / FIXME)
// ---------------------------------------------------------------------------

pub(super) fn check_markers(file: &str, content: &str, findings: &mut Vec<ReviewFinding>) {
    for (i, line) in content.lines().enumerate() {
        let line_upper = line.to_uppercase();
        if line_upper.contains("FIXME") {
            findings.push(ReviewFinding {
                severity: Severity::Info,
                file: file.to_string(),
                line: Some(i + 1),
                message: "FIXME marker found".to_string(),
                source: "markers".to_string(),
            });
        } else if line_upper.contains("TODO") {
            findings.push(ReviewFinding {
                severity: Severity::Info,
                file: file.to_string(),
                line: Some(i + 1),
                message: "TODO marker found".to_string(),
                source: "markers".to_string(),
            });
        }
    }
}
