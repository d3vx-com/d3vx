//! PostToolUse Metadata Observer
//!
//! Watches agent tool output for significant workspace events and
//! updates session metadata automatically.

use std::sync::Arc;

/// Significant events detected in tool output.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceEvent {
    /// A pull request was created via `gh pr create`.
    PrCreated { url: String, number: u64 },
    /// A new branch was created via git checkout/switch.
    BranchCreated { name: String },
    /// A pull request was merged via `gh pr merge`.
    PrMerged { number: u64 },
    /// Test failures detected from cargo test / npm test output.
    TestsFailed { test_names: Vec<String> },
    /// A CI run was triggered.
    CiTriggered { run_id: Option<String> },
    /// Files were changed by a tool.
    FilesChanged { paths: Vec<String> },
    /// A git commit was made.
    CommitMade { hash: String, message: String },
}

impl std::fmt::Display for WorkspaceEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrCreated { url, number } => write!(f, "PrCreated({url}, #{number})"),
            Self::BranchCreated { name } => write!(f, "BranchCreated({name})"),
            Self::PrMerged { number } => write!(f, "PrMerged(#{number})"),
            Self::TestsFailed { test_names } => write!(f, "TestsFailed({})", test_names.join(", ")),
            Self::CiTriggered { run_id } => {
                write!(f, "CiTriggered({})", run_id.as_deref().unwrap_or("unknown"))
            }
            Self::FilesChanged { paths } => write!(f, "FilesChanged({})", paths.join(", ")),
            Self::CommitMade { hash, message } => write!(f, "CommitMade({hash}, {message})"),
        }
    }
}

/// A matched event with confidence score and the raw output that produced it.
#[derive(Debug, Clone)]
pub struct EventMatch {
    /// The detected workspace event.
    pub event: WorkspaceEvent,
    /// Confidence score between 0.0 and 1.0.
    pub confidence: f64,
    /// The raw tool output line(s) that triggered the match.
    pub raw_output: String,
}

/// A function that inspects tool output and optionally extracts a workspace event.
type ExtractorFn = fn(&str) -> Option<WorkspaceEvent>;

/// A registered output pattern with optional tool filtering.
pub struct OutputPattern {
    /// Human-readable name for this pattern.
    pub name: String,
    /// If set, only scan output from these tool names.
    pub tool_filter: Option<Vec<String>>,
    /// Function that attempts to extract an event from tool output.
    pub extractor: ExtractorFn,
}

/// Observer that scans tool output for known patterns and produces events.
pub struct WorkspaceObserver {
    patterns: Vec<Arc<OutputPattern>>,
}

impl WorkspaceObserver {
    /// Create a new observer with built-in patterns.
    pub fn new() -> Self {
        let patterns = vec![
            Arc::new(OutputPattern {
                name: "pr_created".to_string(),
                tool_filter: Some(vec!["Bash".to_string()]),
                extractor: extract_pr_created,
            }),
            Arc::new(OutputPattern {
                name: "branch_created".to_string(),
                tool_filter: Some(vec!["Bash".to_string()]),
                extractor: extract_branch_created,
            }),
            Arc::new(OutputPattern {
                name: "pr_merged".to_string(),
                tool_filter: Some(vec!["Bash".to_string()]),
                extractor: extract_pr_merged,
            }),
            Arc::new(OutputPattern {
                name: "test_failure".to_string(),
                tool_filter: Some(vec!["Bash".to_string()]),
                extractor: extract_test_failure,
            }),
            Arc::new(OutputPattern {
                name: "commit_made".to_string(),
                tool_filter: Some(vec!["Bash".to_string()]),
                extractor: extract_commit,
            }),
        ];
        Self { patterns }
    }

    /// Scan tool output for all registered patterns.
    ///
    /// Returns a list of matches sorted by confidence (highest first).
    pub fn scan_output(&self, tool_name: &str, output: &str) -> Vec<EventMatch> {
        let mut matches = Vec::new();

        for pattern in &self.patterns {
            let applies = match &pattern.tool_filter {
                Some(tools) => tools.iter().any(|t| t == tool_name),
                None => true,
            };
            if !applies {
                continue;
            }

            if let Some(event) = (pattern.extractor)(output) {
                let confidence = compute_confidence(&event, output);
                matches.push(EventMatch {
                    event,
                    confidence,
                    raw_output: output.to_string(),
                });
            }
        }

        matches.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches
    }

    /// Register a custom output pattern.
    pub fn register_pattern(&mut self, pattern: OutputPattern) {
        self.patterns.push(Arc::new(pattern));
    }
}

impl Default for WorkspaceObserver {
    fn default() -> Self {
        Self::new()
    }
}

/// Assign a heuristic confidence based on event type and output content.
fn compute_confidence(event: &WorkspaceEvent, output: &str) -> f64 {
    match event {
        WorkspaceEvent::PrCreated { .. } | WorkspaceEvent::PrMerged { .. } => {
            if output.contains("github.com") {
                0.95
            } else {
                0.7
            }
        }
        WorkspaceEvent::BranchCreated { .. } => 0.85,
        WorkspaceEvent::TestsFailed { test_names } => {
            if test_names.len() > 1 {
                0.9
            } else {
                0.8
            }
        }
        WorkspaceEvent::CiTriggered { .. } => 0.75,
        WorkspaceEvent::FilesChanged { .. } => 0.7,
        WorkspaceEvent::CommitMade { hash, .. } => {
            if hash.len() >= 7 {
                0.9
            } else {
                0.6
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Built-in extractors
// ---------------------------------------------------------------------------

/// Detect `gh pr create` output containing a GitHub pull request URL.
pub fn extract_pr_created(output: &str) -> Option<WorkspaceEvent> {
    for line in output.lines() {
        let line = line.trim();
        if !line.contains("pull/") {
            continue;
        }
        // Match URL like https://github.com/owner/repo/pull/123
        if let Some(url) = extract_github_pr_url(line) {
            let number = extract_pr_number_from_url(&url)?;
            return Some(WorkspaceEvent::PrCreated { url, number });
        }
    }
    None
}

/// Detect branch creation from `git checkout -b` or `git switch -c`.
pub fn extract_branch_created(output: &str) -> Option<WorkspaceEvent> {
    for line in output.lines() {
        let line = line.trim();
        // "Switched to a new branch 'name'" or "Switched to branch 'name'"
        if let Some(name) = extract_between(line, "Switched to a new branch '", "'") {
            return Some(WorkspaceEvent::BranchCreated {
                name: name.to_string(),
            });
        }
        if let Some(name) = extract_between(line, "Switched to branch '", "'") {
            return Some(WorkspaceEvent::BranchCreated {
                name: name.to_string(),
            });
        }
    }
    None
}

/// Detect `gh pr merge` output.
pub fn extract_pr_merged(output: &str) -> Option<WorkspaceEvent> {
    for line in output.lines() {
        let line = line.trim();
        // gh pr merge outputs lines like "merged" or "Pull request #42 was merged"
        if line.contains("merged") && line.contains("pull request") {
            let number = extract_hash_number(line)?;
            return Some(WorkspaceEvent::PrMerged { number });
        }
        // Also handle "gh pr merge" confirmation lines with URL
        if let Some(url) = extract_github_pr_url(line) {
            if output.to_lowercase().contains("merge") {
                let number = extract_pr_number_from_url(&url)?;
                return Some(WorkspaceEvent::PrMerged { number });
            }
        }
    }
    None
}

/// Detect test failures from `cargo test` or `npm test` output.
pub fn extract_test_failure(output: &str) -> Option<WorkspaceEvent> {
    let mut test_names = Vec::new();
    let is_test_run =
        output.contains("test result:") || output.contains("Tests:") || output.contains("FAIL");
    if !is_test_run {
        return None;
    }

    for line in output.lines() {
        let line = line.trim();
        // cargo test: "test test_name ... FAILED"
        if line.contains("... FAILED") {
            let name = line
                .trim_start_matches("test ")
                .trim_end_matches(" ... FAILED")
                .trim();
            if !name.is_empty() {
                test_names.push(name.to_string());
            }
        }
        // npm/jest: "FAIL path/to/test.ts" or "  * test_name"
        if line.starts_with("FAIL ") {
            let name = line.trim_start_matches("FAIL ").trim();
            if !name.is_empty() {
                test_names.push(name.to_string());
            }
        }
    }

    if test_names.is_empty() {
        return None;
    }

    Some(WorkspaceEvent::TestsFailed { test_names })
}

/// Detect `git commit` output with a commit hash.
pub fn extract_commit(output: &str) -> Option<WorkspaceEvent> {
    for line in output.lines() {
        let line = line.trim();
        // Format: "[branch abc1234] commit message"
        if let Some(bracket_end) = line.find(']') {
            let before_bracket = &line[..bracket_end];
            if let Some(hash_start) = before_bracket.rfind(' ') {
                let hash = &before_bracket[hash_start + 1..];
                if hash.len() >= 7 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                    let message = line[bracket_end + 1..].trim().to_string();
                    return Some(WorkspaceEvent::CommitMade {
                        hash: hash.to_string(),
                        message,
                    });
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a GitHub PR URL from a string.
fn extract_github_pr_url(text: &str) -> Option<String> {
    let start = text.find("https://github.com/")?;
    let rest = &text[start..];
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Extract the PR number from a GitHub pull request URL.
fn extract_pr_number_from_url(url: &str) -> Option<u64> {
    let parts: Vec<&str> = url.split('/').collect();
    // .../pull/123
    let pull_idx = parts.iter().position(|p| *p == "pull")?;
    let num_str = parts.get(pull_idx + 1)?;
    let num_str = num_str.trim_end_matches(|c: char| !c.is_ascii_digit());
    num_str.parse().ok()
}

/// Extract a `#number` from text (e.g., "Pull request #42 was merged").
fn extract_hash_number(text: &str) -> Option<u64> {
    let idx = text.find('#')?;
    let after = &text[idx + 1..];
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Extract text between two delimiters.
fn extract_between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)?;
    let after_open = &text[start + open.len()..];
    let end = after_open.find(close)?;
    Some(&after_open[..end])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pr_created_from_gh_output() {
        let output = "Creating pull request for feature-x into main in owner/repo\n\
                      https://github.com/owner/repo/pull/42\n";
        let event = extract_pr_created(output).expect("should detect PR created");
        match event {
            WorkspaceEvent::PrCreated { url, number } => {
                assert_eq!(url, "https://github.com/owner/repo/pull/42");
                assert_eq!(number, 42);
            }
            other => panic!("Expected PrCreated, got {other}"),
        }
    }

    #[test]
    fn test_extract_branch_from_git_output() {
        let output = "Switched to a new branch 'feature/auth-login'\n";
        let event = extract_branch_created(output).expect("should detect branch");
        match event {
            WorkspaceEvent::BranchCreated { name } => {
                assert_eq!(name, "feature/auth-login");
            }
            other => panic!("Expected BranchCreated, got {other}"),
        }
    }

    #[test]
    fn test_extract_test_failure_from_cargo() {
        let output = "\
running 3 tests
test test_add ... ok
test test_subtract ... FAILED
test test_multiply ... FAILED

test result: FAILED. 1 passed; 2 failed;
";
        let event = extract_test_failure(output).expect("should detect failures");
        match event {
            WorkspaceEvent::TestsFailed { test_names } => {
                assert_eq!(test_names, vec!["test_subtract", "test_multiply"]);
            }
            other => panic!("Expected TestsFailed, got {other}"),
        }
    }

    #[test]
    fn test_no_match_for_unrelated_output() {
        let output = "The quick brown fox jumps over the lazy dog.\nAll good here.\n";
        assert!(extract_pr_created(output).is_none());
        assert!(extract_branch_created(output).is_none());
        assert!(extract_pr_merged(output).is_none());
        assert!(extract_test_failure(output).is_none());
        assert!(extract_commit(output).is_none());
    }

    #[test]
    fn test_multiple_events_from_single_output() {
        let observer = WorkspaceObserver::new();

        let output = "\
[main abc1234] feat: add login handler
Switched to a new branch 'feature/login'
https://github.com/owner/repo/pull/7
";

        let matches = observer.scan_output("Bash", output);
        // Should detect at least commit + branch + PR
        assert!(
            matches.len() >= 3,
            "Expected at least 3 events, got {}",
            matches.len()
        );

        let events: Vec<String> = matches.iter().map(|m| m.event.to_string()).collect();
        assert!(events.iter().any(|e| e.starts_with("CommitMade")));
        assert!(events.iter().any(|e| e.starts_with("BranchCreated")));
        assert!(events.iter().any(|e| e.starts_with("PrCreated")));
    }

    #[test]
    fn test_scan_respects_tool_filter() {
        let observer = WorkspaceObserver::new();

        // PR extraction is filtered to Bash only; Read should not match.
        let output = "https://github.com/owner/repo/pull/99\n";
        let matches = observer.scan_output("Read", output);
        assert!(
            matches.is_empty(),
            "Read tool should not trigger Bash patterns"
        );
    }

    #[test]
    fn test_register_custom_pattern() {
        let mut observer = WorkspaceObserver::new();
        observer.register_pattern(OutputPattern {
            name: "custom_deploy".to_string(),
            tool_filter: None,
            extractor: |_output: &str| None, // placeholder
        });

        let matches = observer.scan_output("Bash", "irrelevant");
        assert!(matches.is_empty());
    }
}
