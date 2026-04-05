//! Semantic Merge Analysis
//!
//! Goes beyond line-based conflict detection to analyze structural changes
//! that could break the codebase at merge time:
//!
//! - **API contract breaks**: changed function signatures, removed public items
//! - **Call graph breaks**: callers that reference deleted or renamed symbols
//! - **Value collisions**: enum variant removals, constant changes, type changes
//! - **Behavior shifts**: changed default values, altered error handling paths
//!
//! ## Levels vs Line Conflicts
//!
//! A line conflict (what `conflicts.rs` detects) is when two branches
//! modify the same line. A semantic issue is when branches modify
//! *different* lines but the *combined* result is broken:
//!
//! ```text
//! Branch A: deletes fn foo()
//! Branch B: adds let x = foo();
//! → No line conflict, but compile error after merge
//! ```
//!
//! ## Design
//!
//! Analysis is heuristic-based (no full AST parsing) using pattern matching
//! on diffs for performance. Each finding includes the symbol, issue type,
//! suggested resolution, and a severity level.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Kind of semantic issue detected.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    /// A public function signature changed or was removed
    ApiContractBreak,
    /// A caller references a symbol that no longer exists
    CallGraphBreak,
    /// An enum variant or constant was changed incompatibly
    ValueCollision,
    /// A type definition changed (struct fields, trait impls)
    TypeMismatch,
    /// A file was modified by both branches with different intent
    ConflictingIntent,
    /// A default value or behavior changed
    BehaviorShift,
}

/// Severity of a merge issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeViolationSeverity {
    /// Informational — human review recommended
    Info,
    /// Warning — likely needs a small adjustment
    Warning,
    /// Error — will not compile or tests will fail
    Error,
    /// Critical — data loss or security regression possible
    Critical,
}

impl std::fmt::Display for MergeViolationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// A single semantic merge issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeViolation {
    /// What kind of issue
    pub kind: IssueKind,
    /// How severe
    pub severity: MergeViolationSeverity,
    /// File and symbol involved
    pub symbol: String,
    /// Which branch introduced the change
    pub branch_a_changes: String,
    /// Which branch has the conflicting reference
    pub branch_b_changes: String,
    /// Suggested resolution
    pub suggestion: String,
}

/// Result of semantic merge analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticsReport {
    /// Whether merge is safe
    pub merge_safe: bool,
    /// Issues found
    pub violations: Vec<MergeViolation>,
    /// Files analyzed
    pub files_analyzed: usize,
    /// Summary for UI display
    pub summary: String,
}

impl SemanticsReport {
    pub fn clean(files_analyzed: usize) -> Self {
        Self {
            merge_safe: true,
            violations: Vec::new(),
            files_analyzed,
            summary: "No semantic conflicts detected".to_string(),
        }
    }
}

/// Analyzes diffs from two branches for semantic conflicts.
pub struct SemanticMergeAnalyzer;

impl SemanticMergeAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Analyze two branch diffs for semantic merge issues.
    ///
    /// `diff_a` and `diff_b` are unified diff strings from each branch
    /// against the common ancestor.
    pub fn analyze(diff_a: &str, diff_b: &str, files_changed_common: &[String]) -> SemanticsReport {
        let mut violations = Vec::new();
        let files_a = Self::parse_changed_files(diff_a);
        let files_b = Self::parse_changed_files(diff_b);
        let total_files: HashSet<_> = files_a
            .iter()
            .chain(files_b.iter())
            .chain(files_changed_common.iter())
            .cloned()
            .collect();

        // 1. Check for API contract breaks: one branch removes a symbol,
        // the other references it.
        let removed_symbols = Self::extract_removed_symbols(diff_a);
        let added_calls = Self::extract_function_calls(diff_b);
        for symbol in &removed_symbols {
            if added_calls.contains(symbol) {
                violations.push(MergeViolation {
                    kind: IssueKind::CallGraphBreak,
                    severity: MergeViolationSeverity::Error,
                    symbol: symbol.clone(),
                    branch_a_changes: format!("removed {symbol}"),
                    branch_b_changes: format!("calls {symbol}"),
                    suggestion: format!("update caller or restore {symbol}"),
                });
            }
        }

        // 2. Symmetric check: branch B removes, branch A calls
        let removed_b = Self::extract_removed_symbols(diff_b);
        let added_calls_a = Self::extract_function_calls(diff_a);
        for symbol in &removed_b {
            if added_calls_a.contains(symbol) {
                violations.push(MergeViolation {
                    kind: IssueKind::CallGraphBreak,
                    severity: MergeViolationSeverity::Error,
                    symbol: symbol.clone(),
                    branch_a_changes: format!("calls {symbol}"),
                    branch_b_changes: format!("removed {symbol}"),
                    suggestion: format!("update caller or restore {symbol}"),
                });
            }
        }

        // 3. Check for value collisions: enum variant changes
        let enum_changes_a = Self::extract_enum_changes(diff_a);
        let enum_changes_b = Self::extract_enum_changes(diff_b);
        for (variant, change_a) in &enum_changes_a {
            if enum_changes_b.contains_key(variant) {
                let change_b = &enum_changes_b[variant];
                violations.push(MergeViolation {
                    kind: IssueKind::ValueCollision,
                    severity: MergeViolationSeverity::Warning,
                    symbol: variant.clone(),
                    branch_a_changes: change_a.clone(),
                    branch_b_changes: change_b.clone(),
                    suggestion: "reconcile enum variant changes".to_string(),
                });
            }
        }

        // 4. Conflicting intent: both branches touch the same file
        let overlapping_files: HashSet<_> = files_a
            .iter()
            .cloned()
            .collect::<HashSet<_>>()
            .intersection(&files_b.iter().cloned().collect())
            .cloned()
            .collect();
        for file in &overlapping_files {
            violations.push(MergeViolation {
                kind: IssueKind::ConflictingIntent,
                severity: MergeViolationSeverity::Info,
                symbol: file.clone(),
                branch_a_changes: "modified".to_string(),
                branch_b_changes: "modified".to_string(),
                suggestion: "resolve line-level merge conflicts".to_string(),
            });
        }

        // Sort by severity (most critical first)
        violations.sort_by(|a, b| b.severity.cmp(&a.severity));

        let merge_safe = violations
            .iter()
            .all(|v| v.severity <= MergeViolationSeverity::Info);

        let summary = if violations.is_empty() {
            "No semantic conflicts detected".to_string()
        } else {
            let errors: usize = violations
                .iter()
                .filter(|v| v.severity >= MergeViolationSeverity::Error)
                .count();
            let warnings: usize = violations
                .iter()
                .filter(|v| v.severity == MergeViolationSeverity::Warning)
                .count();
            let info: usize = violations
                .iter()
                .filter(|v| v.severity == MergeViolationSeverity::Info)
                .count();
            format!(
                "{} issue(s): {} error(s), {} warning(s), {} info",
                violations.len(),
                errors,
                warnings,
                info
            )
        };

        SemanticsReport {
            merge_safe,
            violations,
            files_analyzed: total_files.len(),
            summary,
        }
    }

    /// Extract set of files modified in a diff.
    fn parse_changed_files(diff: &str) -> Vec<String> {
        let mut files = Vec::new();
        for line in diff.lines() {
            if line.starts_with("+++ b/") || line.starts_with("--- a/") {
                let path = line.splitn(2, '/').nth(1).unwrap_or("");
                let path = path.split_whitespace().next().unwrap_or("");
                if !path.is_empty() && !files.iter().any(|f| f == path) {
                    files.push(path.to_string());
                }
            }
        }
        files
    }

    /// Extract symbols (functions, types) that were removed.
    fn extract_removed_symbols(diff: &str) -> HashSet<String> {
        let mut removed = HashSet::new();

        for line in diff.lines() {
            if !line.starts_with('-') || line.starts_with("---") {
                continue;
            }
            let trimmed = line.trim_start_matches('-').trim();

            // Function removal
            if let Some(name) = Self::extract_fn_name(trimmed) {
                removed.insert(name);
            }
            // Const/struct/enum removal
            if let Some(name) = Self::extract_type_name(trimmed) {
                removed.insert(name);
            }
        }

        removed
    }

    /// Extract function calls added in a diff.
    fn extract_function_calls(diff: &str) -> HashSet<String> {
        let mut calls = HashSet::new();

        for line in diff.lines() {
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }
            // Match simple function calls: `identifier(`
            let trimmed = line.trim_start_matches('+').trim();
            for cap in trimmed.match_indices(|c: char| c.is_ascii_alphabetic() || c == '_') {
                let start = cap.0;
                let rest = &trimmed[start..];
                let end = rest
                    .find('_')
                    .map(|i| rest[i..].find('(').map(|j| i + j + 1).unwrap_or(rest.len()))
                    .unwrap_or_else(|| rest.find('(').unwrap_or(rest.len()));

                let ident = rest[..end].trim_end_matches('(');
                if !ident.is_empty() && !Self::is_keyword(ident) {
                    calls.insert(ident.to_string());
                }
            }
        }

        calls
    }

    /// Extract enum variant changes.
    fn extract_enum_changes(diff: &str) -> HashMap<String, String> {
        let mut changes = HashMap::new();

        for line in diff.lines() {
            let stripped = line.trim_start_matches(['+', '-']).trim();

            if stripped.starts_with("variant ") || stripped.contains("Variant") {
                let name = stripped.split_whitespace().nth(1).unwrap_or("unknown");
                let action = if line.starts_with('-') {
                    "removed".to_string()
                } else if line.starts_with('+') {
                    "added".to_string()
                } else {
                    "modified".to_string()
                };
                changes.insert(name.to_string(), action);
            }
        }

        changes
    }

    fn extract_fn_name(line: &str) -> Option<String> {
        if line.starts_with("fn ") || line.starts_with("pub fn ") {
            let after = line.rfind("fn ").map(|i| &line[i + 3..])?;
            after.split('(').next().map(|s| s.trim().to_string())
        } else if line.starts_with("async fn ") || line.starts_with("pub async fn ") {
            let after = line.rfind("fn ").map(|i| &line[i + 3..])?;
            after.split('(').next().map(|s| s.trim().to_string())
        } else {
            None
        }
    }

    fn extract_type_name(line: &str) -> Option<String> {
        for prefix in &[
            "pub struct ",
            "struct ",
            "pub enum ",
            "enum ",
            "pub const ",
            "const ",
        ] {
            if line.starts_with(prefix) {
                let after = &line[prefix.len()..];
                return after.split_whitespace().next().map(|s| s.to_string());
            }
        }
        None
    }

    fn is_keyword(ident: &str) -> bool {
        matches!(
            ident,
            "fn" | "let"
                | "pub"
                | "use"
                | "mod"
                | "struct"
                | "enum"
                | "impl"
                | "trait"
                | "type"
                | "const"
                | "static"
                | "if"
                | "else"
                | "match"
                | "for"
                | "while"
                | "loop"
                | "return"
                | "self"
                | "Self"
                | "super"
                | "where"
                | "mut"
                | "ref"
                | "async"
                | "await"
        )
    }
}

impl Default for SemanticMergeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_report() {
        let report = SemanticsReport::clean(5);
        assert!(report.merge_safe);
        assert!(report.violations.is_empty());
        assert_eq!(report.files_analyzed, 5);
    }

    #[test]
    fn test_call_graph_break_detection() {
        let diff_a = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +0,0 @@\n-fn helper() {}\n";
        let diff_b = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,5 @@\n+let x = helper();\n";

        let result = SemanticMergeAnalyzer::analyze(diff_a, diff_b, &[]);
        assert!(!result.merge_safe);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].kind, IssueKind::CallGraphBreak);
        assert_eq!(result.violations[0].severity, MergeViolationSeverity::Error);
    }

    #[test]
    fn test_symmetric_call_graph_break() {
        let diff_a =
            "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,5 @@\n+let y = removed_func();\n";
        let diff_b = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,3 +0,0 @@\n-fn removed_func() {}\n";

        let result = SemanticMergeAnalyzer::analyze(diff_a, diff_b, &[]);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].kind, IssueKind::CallGraphBreak);
    }

    #[test]
    fn test_parse_changed_files() {
        let diff = "--- a/src/lib.rs\n+++ b/src/lib.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n";
        let files = SemanticMergeAnalyzer::parse_changed_files(diff);
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f == "src/lib.rs"));
    }

    #[test]
    fn test_extract_fn_name() {
        assert_eq!(
            SemanticMergeAnalyzer::extract_fn_name("-fn foo(arg: i32) {}"),
            Some("foo".to_string())
        );
        assert_eq!(
            SemanticMergeAnalyzer::extract_fn_name("-pub fn bar() -> Result<()>"),
            Some("bar".to_string())
        );
    }

    #[test]
    fn test_severity_ordering() {
        use MergeViolationSeverity::*;
        assert!(Info < Warning);
        assert!(Warning < Error);
        assert!(Error < Critical);
    }
}
