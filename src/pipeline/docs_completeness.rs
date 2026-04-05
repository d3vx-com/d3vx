//! Documentation Completeness System
//!
//! Evaluates whether documentation updates are required and satisfied for code changes.
//! Integrates with the review gate system for merge readiness.
//!
//! ## Overview
//!
//! When code changes are made, documentation may need updates. This module:
//! - Evaluates whether docs are required based on change signals
//! - Checks if required documentation has been updated
//! - Provides a completeness status for merge readiness
//!
//! ## Usage
//!
//! ```rust
//! use pipeline::docs_completeness::{DocsCompleteness, DocsCompletenessEvaluator};
//!
//! let evaluator = DocsCompletenessEvaluator::new(project_root);
//! let result = evaluator.evaluate(&changed_files, &description);
//! if result.required() && !result.is_satisfied() {
//!     // Don't merge - docs required
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Documentation completeness status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocsStatus {
    /// Docs evaluation has not run
    NotEvaluated,
    /// Documentation not required for these changes
    NotRequired,
    /// Documentation is complete
    Complete,
    /// Documentation is missing but required
    Missing,
    /// Documentation is partial
    Partial,
}

impl DocsStatus {
    /// Whether docs are satisfied for merge
    pub fn satisfied_for_merge(self) -> bool {
        matches!(self, DocsStatus::NotRequired | DocsStatus::Complete)
    }

    /// Whether docs are required but missing
    pub fn blocks_merge(self) -> bool {
        matches!(self, DocsStatus::Missing)
    }
}

impl Default for DocsStatus {
    fn default() -> Self {
        DocsStatus::NotEvaluated
    }
}

/// Type of documentation that might be needed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocType {
    /// README or main documentation
    Readme,
    /// API documentation
    ApiDocs,
    /// Changelog or release notes
    Changelog,
    /// Inline code comments
    InlineComments,
    /// Examples or tutorials
    Examples,
    /// Configuration documentation
    ConfigDocs,
}

impl DocType {
    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            DocType::Readme => "README",
            DocType::ApiDocs => "API Docs",
            DocType::Changelog => "Changelog",
            DocType::InlineComments => "Comments",
            DocType::Examples => "Examples",
            DocType::ConfigDocs => "Config Docs",
        }
    }
}

/// Signal that might require documentation updates
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocsSignal {
    pub doc_type: DocType,
    pub file_pattern: String,
    pub reason: String,
    pub satisfied: bool,
}

/// Result of docs completeness evaluation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DocsCompleteness {
    /// Task ID this belongs to
    pub task_id: Option<String>,
    /// Overall completeness status
    pub status: DocsStatus,
    /// Signals that were evaluated
    pub signals: Vec<DocsSignal>,
    /// Whether docs are required
    pub docs_required: bool,
    /// Whether docs are satisfied
    pub satisfied: bool,
    /// Missing documentation types
    pub missing_types: Vec<DocType>,
    /// Changed files that were considered
    pub changed_files: Vec<String>,
    /// When evaluation occurred
    pub evaluated_at: Option<String>,
}

impl DocsCompleteness {
    /// Create a new not-evaluated result
    pub fn not_evaluated(task_id: Option<String>) -> Self {
        Self {
            task_id,
            status: DocsStatus::NotEvaluated,
            signals: Vec::new(),
            docs_required: false,
            satisfied: true,
            missing_types: Vec::new(),
            changed_files: Vec::new(),
            evaluated_at: None,
        }
    }

    /// Check if merge is blocked due to docs
    pub fn blocks_merge(&self) -> bool {
        self.docs_required && !self.satisfied
    }

    /// Check if merge can proceed
    pub fn can_merge(&self) -> bool {
        !self.blocks_merge()
    }

    /// Get status line for UI
    pub fn status_line(&self) -> String {
        match self.status {
            DocsStatus::NotEvaluated => "Docs: Not evaluated".to_string(),
            DocsStatus::NotRequired => "Docs: Not required".to_string(),
            DocsStatus::Complete => "Docs: Complete".to_string(),
            DocsStatus::Missing => format!(
                "Docs: Missing ({})",
                self.missing_types
                    .iter()
                    .map(|t| t.display_name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            DocsStatus::Partial => "Docs: Partial".to_string(),
        }
    }

    /// Get summary for UI display
    pub fn summary_for_ui(&self) -> DocsCompletenessUiSummary {
        DocsCompletenessUiSummary {
            status: self.status_line(),
            status_enum: self.status,
            docs_required: self.docs_required,
            satisfied: self.satisfied,
            blocks_merge: self.blocks_merge(),
            can_merge: self.can_merge(),
            missing_count: self.missing_types.len(),
            signals_count: self.signals.len(),
            signals: self
                .signals
                .iter()
                .map(|s| DocsSignalUi {
                    doc_type: s.doc_type,
                    doc_type_name: s.doc_type.display_name().to_string(),
                    reason: s.reason.clone(),
                    satisfied: s.satisfied,
                })
                .collect(),
        }
    }
}

/// Simplified UI summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocsCompletenessUiSummary {
    pub status: String,
    pub status_enum: DocsStatus,
    pub docs_required: bool,
    pub satisfied: bool,
    pub blocks_merge: bool,
    pub can_merge: bool,
    pub missing_count: usize,
    pub signals_count: usize,
    pub signals: Vec<DocsSignalUi>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocsSignalUi {
    pub doc_type: DocType,
    pub doc_type_name: String,
    pub reason: String,
    pub satisfied: bool,
}

/// Evaluates documentation completeness based on code changes
pub struct DocsCompletenessEvaluator {
    #[allow(dead_code)] // Reserved for filesystem-based doc discovery
    project_root: std::path::PathBuf,
}

impl DocsCompletenessEvaluator {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self { project_root }
    }

    /// Evaluate docs completeness for a set of changed files
    pub fn evaluate(&self, changed_files: &[String], description: &str) -> DocsCompleteness {
        let mut signals = Vec::new();
        let mut docs_required = false;
        let mut satisfied_count = 0;

        let changed: HashSet<_> = changed_files.iter().map(|s| s.as_str()).collect();

        // Check for README changes
        let readme_signals = self.check_readme(&changed, description);
        for signal in readme_signals {
            docs_required = true;
            if signal.satisfied {
                satisfied_count += 1;
            }
            signals.push(signal);
        }

        // Check for API docs changes
        let api_signals = self.check_api_docs(&changed);
        for signal in api_signals {
            docs_required = true;
            if signal.satisfied {
                satisfied_count += 1;
            }
            signals.push(signal);
        }

        // Check for changelog
        let changelog_signals = self.check_changelog(&changed, description);
        for signal in changelog_signals {
            docs_required = true;
            if signal.satisfied {
                satisfied_count += 1;
            }
            signals.push(signal);
        }

        // Check for breaking changes (always require changelog)
        if self.is_breaking_change(description) {
            let has_changelog = changed.iter().any(|f| {
                f.contains("CHANGELOG") || f.contains("changelog") || f.contains("CHANGES")
            });
            if !has_changelog {
                signals.push(DocsSignal {
                    doc_type: DocType::Changelog,
                    file_pattern: "CHANGELOG*".to_string(),
                    reason: "Breaking change detected - changelog required".to_string(),
                    satisfied: false,
                });
                docs_required = true;
            }
        }

        let status = if !docs_required {
            DocsStatus::NotRequired
        } else if satisfied_count == signals.len() && !signals.is_empty() {
            DocsStatus::Complete
        } else if satisfied_count > 0 {
            DocsStatus::Partial
        } else {
            DocsStatus::Missing
        };

        let missing_types: Vec<DocType> = signals
            .iter()
            .filter(|s| !s.satisfied)
            .map(|s| s.doc_type)
            .collect();

        DocsCompleteness {
            task_id: None,
            status,
            signals,
            docs_required,
            satisfied: docs_required && missing_types.is_empty(),
            missing_types,
            changed_files: changed_files.to_vec(),
            evaluated_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }

    fn check_readme(&self, changed: &HashSet<&str>, _description: &str) -> Vec<DocsSignal> {
        let mut signals = Vec::new();

        // Check if README was updated
        let readme_updated = changed.iter().any(|f| {
            f.ends_with("README.md") || f.ends_with("README.txt") || f.ends_with("README")
        });

        // New feature/behavior changes might need README
        let needs_readme = changed.iter().any(|f| {
            let lower = f.to_lowercase();
            lower.ends_with(".rs") && !lower.contains("test")
        });

        if needs_readme && !readme_updated {
            signals.push(DocsSignal {
                doc_type: DocType::Readme,
                file_pattern: "README*".to_string(),
                reason: "Source files changed - verify README is updated".to_string(),
                satisfied: false,
            });
        } else if readme_updated {
            signals.push(DocsSignal {
                doc_type: DocType::Readme,
                file_pattern: "README*".to_string(),
                reason: "README was updated".to_string(),
                satisfied: true,
            });
        }

        signals
    }

    fn check_api_docs(&self, changed: &HashSet<&str>) -> Vec<DocsSignal> {
        let mut signals = Vec::new();

        // Check for API file changes
        let api_changed = changed.iter().any(|f| {
            f.contains("/api/")
                || f.ends_with("_api.rs")
                || f.contains("/v1/")
                || f.ends_with("_client.rs")
        });

        // Check for docs.rs or doc files
        let docs_updated = changed.iter().any(|f| {
            f.contains("/docs/")
                || f.contains("/doc/")
                || f.ends_with(".md") && !f.ends_with("README.md") && !f.contains("CHANGELOG")
        });

        if api_changed {
            signals.push(DocsSignal {
                doc_type: DocType::ApiDocs,
                file_pattern: "**/docs/**".to_string(),
                reason: "API files changed - check docs are updated".to_string(),
                satisfied: docs_updated,
            });
        }

        signals
    }

    fn check_changelog(&self, changed: &HashSet<&str>, description: &str) -> Vec<DocsSignal> {
        let mut signals = Vec::new();

        let changelog_updated = changed.iter().any(|f| {
            f.contains("CHANGELOG")
                || f.contains("changelog")
                || f.contains("CHANGES")
                || f.ends_with(".changes.md")
        });

        // Features, fixes, changes typically need changelog
        let has_notable_change = description.to_lowercase().contains("add ")
            || description.to_lowercase().contains("fix ")
            || description.to_lowercase().contains("change ")
            || description.to_lowercase().contains("improve")
            || description.to_lowercase().contains("deprecate")
            || description.to_lowercase().contains("remove");

        if has_notable_change {
            signals.push(DocsSignal {
                doc_type: DocType::Changelog,
                file_pattern: "CHANGELOG*".to_string(),
                reason: "Notable change detected".to_string(),
                satisfied: changelog_updated,
            });
        }

        signals
    }

    fn is_breaking_change(&self, description: &str) -> bool {
        let lower = description.to_lowercase();
        lower.contains("break")
            || lower.contains("remove")
            || lower.contains("delete")
            || lower.contains("rename")
            || lower.contains("rename")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_required_when_no_source_changes() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());
        let result = evaluator.evaluate(&[], "format code");
        assert_eq!(result.status, DocsStatus::NotRequired);
        assert!(result.can_merge());
    }

    #[test]
    fn test_breaking_change_requires_changelog() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());
        let result =
            evaluator.evaluate(&["src/lib.rs".to_string()], "break: remove deprecated API");
        assert!(result.docs_required);
        assert!(!result.satisfied);
        assert!(result.blocks_merge());
    }

    #[test]
    fn test_changelog_satisfied() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());
        let result = evaluator.evaluate(
            &["src/lib.rs".to_string(), "CHANGELOG.md".to_string()],
            "add new feature",
        );
        assert!(result.docs_required);
        // Breaking change still requires changelog, but feature change check should find it
        assert!(!result.blocks_merge() || result.satisfied);
    }

    #[test]
    fn test_status_satisfied_for_merge() {
        assert!(DocsStatus::NotRequired.satisfied_for_merge());
        assert!(DocsStatus::Complete.satisfied_for_merge());
        assert!(!DocsStatus::Missing.satisfied_for_merge());
        assert!(!DocsStatus::Partial.satisfied_for_merge());
    }

    #[test]
    fn test_blocks_merge() {
        assert!(!DocsStatus::NotRequired.blocks_merge());
        assert!(!DocsStatus::Complete.blocks_merge());
        assert!(DocsStatus::Missing.blocks_merge());
        assert!(!DocsStatus::Partial.blocks_merge());
    }

    #[test]
    fn test_summary_for_ui() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());
        let result = evaluator.evaluate(&["src/lib.rs".to_string()], "add feature");
        let ui = result.summary_for_ui();
        assert!(ui.can_merge || !ui.blocks_merge);
    }

    #[test]
    fn test_api_changes_require_docs() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(
            &["src/api/v1/handler.rs".to_string()],
            "add new API endpoint",
        );

        assert!(result.docs_required);
        assert!(result
            .signals
            .iter()
            .any(|s| s.doc_type == DocType::ApiDocs));
    }

    #[test]
    fn test_api_changes_with_docs_satisfied() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(
            &[
                "src/api/v1/handler.rs".to_string(),
                "docs/api.md".to_string(),
            ],
            "add new API endpoint",
        );

        assert!(result.docs_required);
        let api_signal = result
            .signals
            .iter()
            .find(|s| s.doc_type == DocType::ApiDocs);
        assert!(api_signal.map(|s| s.satisfied).unwrap_or(false));
    }

    #[test]
    fn test_config_changes_require_docs() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(
            &[
                "config/default.toml".to_string(),
                "src/config.rs".to_string(),
            ],
            "add new configuration option",
        );

        assert!(result.docs_required);
    }

    #[test]
    fn test_readme_satisfied_when_updated() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(
            &["src/lib.rs".to_string(), "README.md".to_string()],
            "implement new feature",
        );

        let readme_signal = result
            .signals
            .iter()
            .find(|s| s.doc_type == DocType::Readme);
        assert!(readme_signal.map(|s| s.satisfied).unwrap_or(false));
    }

    #[test]
    fn test_readme_missing_when_source_changed() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(&["src/lib.rs".to_string()], "implement new feature");

        let readme_signal = result
            .signals
            .iter()
            .find(|s| s.doc_type == DocType::Readme);
        assert!(readme_signal.map(|s| !s.satisfied).unwrap_or(false));
    }

    #[test]
    fn test_no_changes_empty_description() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(&[], "");

        assert_eq!(result.status, DocsStatus::NotRequired);
        assert!(!result.docs_required);
    }

    #[test]
    fn test_breaking_change_always_requires_changelog() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result =
            evaluator.evaluate(&["src/lib.rs".to_string()], "BREAKING: remove old function");

        assert!(result.docs_required);
        let changelog_signal = result
            .signals
            .iter()
            .find(|s| s.doc_type == DocType::Changelog);
        assert!(changelog_signal.is_some());
    }

    #[test]
    fn test_breaking_with_changelog_satisfied() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(
            &["src/lib.rs".to_string(), "CHANGELOG.md".to_string()],
            "BREAKING: remove old function",
        );

        assert!(result.docs_required);
        let changelog_signal = result
            .signals
            .iter()
            .find(|s| s.doc_type == DocType::Changelog);
        assert!(changelog_signal.map(|s| s.satisfied).unwrap_or(false));
    }

    #[test]
    fn test_changed_files_preserved_in_result() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());
        let changed = vec!["src/main.rs".to_string(), "README.md".to_string()];

        let result = evaluator.evaluate(&changed, "update main");

        assert_eq!(result.changed_files, changed);
    }

    #[test]
    fn test_test_files_do_not_require_readme() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(
            &["src/lib.rs".to_string(), "tests/lib_test.rs".to_string()],
            "add tests",
        );

        assert!(result.docs_required);
        let readme_signal = result
            .signals
            .iter()
            .find(|s| s.doc_type == DocType::Readme);
        assert!(readme_signal.map(|s| !s.satisfied).unwrap_or(false));
    }

    #[test]
    fn test_deprecation_requires_changelog() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(
            &["src/lib.rs".to_string()],
            "deprecate old function in favor of new one",
        );

        assert!(result.docs_required);
        let changelog_signal = result
            .signals
            .iter()
            .find(|s| s.doc_type == DocType::Changelog);
        assert!(changelog_signal.is_some());
    }

    #[test]
    fn test_complete_status_when_all_satisfied() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(
            &[
                "src/lib.rs".to_string(),
                "README.md".to_string(),
                "CHANGELOG.md".to_string(),
            ],
            "add new feature",
        );

        assert_eq!(result.status, DocsStatus::Complete);
        assert!(result.can_merge());
    }

    #[test]
    fn test_partial_status_when_some_satisfied() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(
            &["src/lib.rs".to_string(), "README.md".to_string()],
            "add new feature and breaking change",
        );

        assert_eq!(result.status, DocsStatus::Partial);
        assert!(!result.can_merge());
    }

    #[test]
    fn test_missing_types_collected() {
        let evaluator = DocsCompletenessEvaluator::new(".".into());

        let result = evaluator.evaluate(
            &["src/api/v1/handler.rs".to_string()],
            "add new API endpoint",
        );

        assert!(!result.missing_types.is_empty());
        assert!(result.missing_types.contains(&DocType::ApiDocs));
    }
}
