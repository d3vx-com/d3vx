//! Prompt Composer
//!
//! Builds structured agent prompts from issue context by layering
//! instructions in a deterministic order.

use super::types::{IssueContext, LaunchConfig};

/// Composes a complete agent prompt from issue context and configuration.
pub struct PromptComposer;

impl PromptComposer {
    /// Build the full agent prompt by layering sections.
    pub fn compose(context: &IssueContext, config: &LaunchConfig) -> String {
        let mut layers = Vec::new();

        layers.push(Self::base_instructions().to_string());
        layers.push(Self::layer_issue_context(context));
        layers.push(Self::layer_project_rules(None));

        let branch = format!("{}-{}", config.branch_prefix, slugify(&context.id));
        layers.push(Self::layer_workspace_hints(&branch, "/workspace"));

        layers.push(Self::layer_done_criteria(&context.labels));

        // Filter empty layers and join with double newlines.
        layers
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Base agent instructions common to all launches.
    pub fn base_instructions() -> &'static str {
        "You are an autonomous agent executing a task in an isolated git worktree.\n\
         Read the issue description carefully and implement a complete solution.\n\
         Follow these rules:\n\
         - Read relevant files before making changes.\n\
         - Write clear, idiomatic code with proper error handling.\n\
         - Run tests after making changes to verify correctness.\n\
         - Commit your work with a descriptive message when done."
    }

    /// Format issue details as a structured context section.
    pub fn layer_issue_context(issue: &IssueContext) -> String {
        let labels = if issue.labels.is_empty() {
            "none".to_string()
        } else {
            issue.labels.join(", ")
        };

        format!(
            "## Issue Details\n\n\
             **ID**: {}\n\
             **Tracker**: {}\n\
             **Title**: {}\n\
             **Labels**: {}\n\n\
             ### Description\n\n{}",
            issue.id, issue.tracker, issue.title, labels, issue.body,
        )
    }

    /// Add project rules if available.
    pub fn layer_project_rules(rules: Option<&str>) -> String {
        match rules {
            Some(r) if !r.trim().is_empty() => {
                format!("## Project Rules\n\n{}", r.trim())
            }
            _ => String::new(),
        }
    }

    /// Workspace-specific context such as branch and workspace path.
    pub fn layer_workspace_hints(branch: &str, workspace: &str) -> String {
        format!(
            "## Workspace\n\n\
             - Branch: `{}`\n\
             - Working directory: `{}`",
            branch, workspace
        )
    }

    /// Define what "done" looks like based on issue labels.
    pub fn layer_done_criteria(labels: &[String]) -> String {
        let mut criteria = vec![
            "All existing tests pass.".to_string(),
            "Changes are committed.".to_string(),
        ];

        let labels_lower: Vec<String> = labels.iter().map(|l| l.to_lowercase()).collect();

        if labels_lower.iter().any(|l| l.contains("bug")) {
            criteria.push("Bug is reproducible before fix and resolved after.".to_string());
        }

        if labels_lower
            .iter()
            .any(|l| l.contains("feature") || l.contains("enhancement"))
        {
            criteria.push("New functionality has corresponding tests.".to_string());
        }

        if labels_lower.iter().any(|l| l.contains("security")) {
            criteria.push("No hardcoded secrets or credentials introduced.".to_string());
            criteria.push("Input validation is present on new entry points.".to_string());
        }

        if labels_lower.iter().any(|l| l.contains("performance")) {
            criteria.push(
                "No obvious performance regressions (e.g., N+1 queries, unbounded allocation)."
                    .to_string(),
            );
        }

        format!(
            "## Done Criteria\n\n{}",
            criteria
                .iter()
                .map(|c| format!("- {}", c))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// Convert a string to a filesystem/branch-safe slug.
fn slugify(input: &str) -> String {
    input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::spawner::types::TrackerKind;

    fn sample_issue() -> IssueContext {
        IssueContext {
            id: "42".to_string(),
            title: "Fix login timeout".to_string(),
            body: "Users report a 30s timeout on login.".to_string(),
            labels: vec!["bug".to_string()],
            tracker: TrackerKind::GitHub,
        }
    }

    #[test]
    fn test_prompt_composer_layers() {
        let config = LaunchConfig::default();
        let prompt = PromptComposer::compose(&sample_issue(), &config);

        // All layers should be present.
        assert!(
            prompt.contains("autonomous agent"),
            "Missing base instructions"
        );
        assert!(prompt.contains("Issue Details"), "Missing issue context");
        assert!(prompt.contains("**ID**: 42"), "Missing issue ID");
        assert!(prompt.contains("Fix login timeout"), "Missing issue title");
        assert!(prompt.contains("Workspace"), "Missing workspace hints");
        assert!(prompt.contains("Done Criteria"), "Missing done criteria");
        assert!(
            prompt.contains("Bug is reproducible"),
            "Missing bug-specific criteria"
        );
    }

    #[test]
    fn test_layer_project_rules_with_content() {
        let rules = "# My Project\n\nAlways use `anyhow::Result`.";
        let layer = PromptComposer::layer_project_rules(Some(rules));
        assert!(layer.contains("Project Rules"));
        assert!(layer.contains("anyhow::Result"));
    }

    #[test]
    fn test_layer_project_rules_empty() {
        let layer = PromptComposer::layer_project_rules(None);
        assert!(layer.is_empty());
    }

    #[test]
    fn test_done_criteria_security() {
        let criteria = PromptComposer::layer_done_criteria(&["security".to_string()]);
        assert!(criteria.contains("hardcoded secrets"));
        assert!(criteria.contains("Input validation"));
    }

    #[test]
    fn test_done_criteria_performance() {
        let criteria = PromptComposer::layer_done_criteria(&["performance".to_string()]);
        assert!(criteria.contains("performance regressions"));
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("  foo---bar  "), "foo-bar");
        assert_eq!(slugify("Fix #123: bug!"), "fix-123-bug");
    }
}
