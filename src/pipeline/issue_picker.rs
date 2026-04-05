//! Issue Auto-Picker
//!
//! Scores and ranks open issues so the autonomous system can pick the best
//! candidate without human intervention.
//!
//! Unlike the passive `GitHubPoller` (which blindly creates tasks for all
//! issues matching trigger labels), this module applies intelligent triage:
//!
//! - IssuePriorityScore: composite 0-100 score from size, labels, deps
//! - IssuePicker: ranks candidates and selects the top one
//! - PickDecision: why an issue was chosen or deferred
//!
//! ## Scoring Model
//!
//! | Dimension              | Weight | Source           |
//! |------------------------|--------|------------------|
//! | Size / complexity      | 25%    | title/body heuristics |
//! | Label priority         | 20%    | label keywords   |
//! | Dependencies blocked   | -15%   | blocked-by labels|
//! | Recency                | 10%    | updated_at       |
//! | Assignee availability  | 10%    | assignee field   |
//! | PR-linked              | 20%    | issue body regex|
//!
//! A score >= 60 means the picker recommends working on it now.

use serde::{Deserialize, Serialize};

use super::issue_sync::types::{ExternalIssue, IssueState};

/// Priority score for a candidate issue (0-100).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuePriorityScore {
    /// Total composite score (0-100)
    pub total: u8,
    /// Size/complexity dimension score (0-25)
    pub size_score: u8,
    /// Label priority dimension score (0-20)
    pub label_score: u8,
    /// Dependency adjustment (-15 to 0)
    pub dependency_adjustment: i8,
    /// Recency dimension score (0-10)
    pub recency_score: u8,
    /// Assignee availability dimension score (0-10)
    pub assignee_score: u8,
    /// PR-linked bonus (0-20)
    pub pr_link_score: u8,
    /// Human-readable reasoning
    pub reasoning: Vec<String>,
}

impl IssuePriorityScore {
    /// Whether this issue should be picked for autonomous work.
    pub fn is_pickable(&self) -> bool {
        self.total >= 60
    }
}

/// Decision from the picker about which issue to work on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickDecision {
    /// Issue selected (if any)
    pub selected_issue: Option<ExternalIssue>,
    /// Score for the selected issue
    pub selected_score: Option<IssuePriorityScore>,
    /// All candidates that were scored
    pub all_scores: Vec<(ExternalIssue, IssuePriorityScore)>,
    /// Why nothing was selected
    pub deferral_reason: Option<String>,
}

impl PickDecision {
    /// Whether the picker selected an issue to work on.
    pub fn has_selection(&self) -> bool {
        self.selected_issue.is_some()
    }

    /// Returns all scored candidates.
    pub fn scored(&self) -> &Vec<(ExternalIssue, IssuePriorityScore)> {
        &self.all_scores
    }
}

/// Configuration for the issue picker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuePickerConfig {
    /// Minimum total score to pick an issue (0-100)
    pub min_pick_score: u8,
    /// Maximum issues to consider per poll cycle
    pub max_candidates: usize,
    /// Labels that boost priority
    pub priority_labels: Vec<String>,
    /// Labels that deprioritize (blocked-by, depends-on, etc.)
    pub blocking_labels: Vec<String>,
}

impl Default for IssuePickerConfig {
    fn default() -> Self {
        Self {
            min_pick_score: 60,
            max_candidates: 20,
            priority_labels: vec![
                "bug".to_string(),
                "high-priority".to_string(),
                "critical".to_string(),
                "good-first-issue".to_string(),
                "help-wanted".to_string(),
            ],
            blocking_labels: vec![
                "blocked".to_string(),
                "blocked-by".to_string(),
                "depends-on".to_string(),
                "needs-info".to_string(),
            ],
        }
    }
}

/// Scores, ranks, and selects issues for autonomous work.
pub struct IssuePicker {
    config: IssuePickerConfig,
}

impl IssuePicker {
    pub fn new(config: IssuePickerConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(IssuePickerConfig::default())
    }

    /// Score and rank a set of issues. Returns a pick decision.
    pub fn pick(&self, issues: &[ExternalIssue]) -> PickDecision {
        let mut scored: Vec<_> = issues
            .iter()
            .filter(|i| i.state == IssueState::Open)
            .filter(|i| i.assignee.is_none())
            .map(|issue| {
                let score = self.score_issue(issue);
                (issue.clone(), score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.total.cmp(&a.1.total));
        scored.truncate(self.config.max_candidates);

        let pick_threshold = self.config.min_pick_score;
        let first = scored.first();

        if let Some((issue, score)) = first {
            if score.total >= pick_threshold {
                return PickDecision {
                    selected_issue: Some(issue.clone()),
                    selected_score: Some(score.clone()),
                    all_scores: scored,
                    deferral_reason: None,
                };
            }
        }

        let deferral = if scored.is_empty() {
            Some("No open, unassigned issues found".to_string())
        } else {
            let top = &scored[0];
            Some(format!(
                "Top candidate '{}' scored {} (threshold: {})",
                top.0.title, top.1.total, pick_threshold
            ))
        };

        PickDecision {
            selected_issue: None,
            selected_score: None,
            all_scores: scored,
            deferral_reason: deferral,
        }
    }

    /// Score a single issue across all dimensions.
    fn score_issue(&self, issue: &ExternalIssue) -> IssuePriorityScore {
        let mut reasoning = Vec::new();

        let size_score = self.size_dimension(&issue.title, &issue.body);
        if size_score >= 15 {
            reasoning.push("moderate complexity well-scoped".to_string());
        } else if size_score <= 5 {
            reasoning.push("very small, likely quick fix".to_string());
        }

        let label_score = self.label_dimension(&issue.labels);
        if label_score >= 10 {
            reasoning.push("priority labels match".to_string());
        }

        let dep_adj = self.dependency_dimension(&issue.labels);
        if dep_adj < 0 {
            reasoning.push(format!("blocked by {} label(s)", issue.labels.len()));
        }

        let recency_score = 5; // TODO: use issue.updated_at when available
        let assignee_score = 10; // Already filtered to unassigned

        let pr_link_score = self.pr_link_dimension(&issue.body);
        if pr_link_score > 0 {
            reasoning.push("appears PR-related, actionable".to_string());
        }

        let total = (size_score as i16
            + label_score as i16
            + dep_adj as i16
            + recency_score as i16
            + assignee_score as i16
            + pr_link_score as i16)
            .clamp(0, 100) as u8;

        IssuePriorityScore {
            total,
            size_score,
            label_score,
            dependency_adjustment: dep_adj,
            recency_score,
            assignee_score,
            pr_link_score,
            reasoning,
        }
    }

    /// Size/complexity dimension (0-25).
    fn size_dimension(&self, title: &str, body: &Option<String>) -> u8 {
        let text = format!("{} {}", title, body.as_deref().unwrap_or(""));
        let words = text.split_whitespace().count();

        // Sweet spot: 20-200 words = well-scoped, not trivial, not epic
        if words < 10 {
            3
        } else if words < 50 {
            15
        } else if words < 200 {
            25
        } else {
            18 // Too large, less certain of scope
        }
    }

    /// Label priority dimension (0-20).
    fn label_dimension(&self, labels: &[String]) -> u8 {
        let lower: Vec<_> = labels.iter().map(|l| l.to_lowercase()).collect();

        // High priority labels (10 points first match, 5 more per additional)
        let mut score = 0u8;
        let mut found_any = false;

        for label in &self.config.priority_labels {
            if lower.iter().any(|l| l.contains(label)) {
                if !found_any {
                    score += 10;
                    found_any = true;
                } else {
                    score += 5;
                }
            }
        }

        score.min(20)
    }

    /// Dependency adjustment (-15 to 0). Negative reduces score.
    fn dependency_dimension(&self, labels: &[String]) -> i8 {
        let lower: Vec<_> = labels.iter().map(|l| l.to_lowercase()).collect();

        let blocking_count = self
            .config
            .blocking_labels
            .iter()
            .filter(|bl| lower.iter().any(|l| l.contains(*bl)))
            .count();

        if blocking_count > 0 {
            (-15).min(blocking_count as i8 * -8)
        } else {
            0
        }
    }

    /// Recency dimension (0-10). TODO: wire to issue.updated_at.
    fn _recency_dimension(&self, _updated_at: Option<&str>) -> u8 {
        5
    }

    /// PR-linked bonus (0-20). Issues that reference PRs are often actionable.
    fn pr_link_dimension(&self, body: &Option<String>) -> u8 {
        let Some(body) = body else { return 0 };

        // Check for common PR reference patterns
        if body.contains("PR #")
            || body.contains("pull request")
            || body.contains("linked PR")
            || body.contains("refs #")
        {
            20
        } else {
            0
        }
    }
}

/// Build a batch of external issues from JSON (for testing).
#[cfg(test)]
fn make_issue(
    id: &str,
    number: Option<u64>,
    title: &str,
    body: Option<&str>,
    labels: &[&str],
    assignee: Option<&str>,
) -> ExternalIssue {
    use super::issue_sync::types::TrackerKind;
    ExternalIssue {
        id: id.to_string(),
        number,
        title: title.to_string(),
        body: body.map(String::from),
        state: IssueState::Open,
        labels: labels.iter().map(|s| s.to_string()).collect(),
        assignee: assignee.map(String::from),
        url: None,
        tracker: TrackerKind::Github,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_dimension() {
        let picker = IssuePicker::with_defaults();
        // Very short title/body
        let score = picker.size_dimension("fix typo", &None);
        assert!(score <= 5);

        // Well-scoped description
        let score = picker.size_dimension(
            "add rate limiting to API endpoint",
            &Some(
                "The /api/v1/users endpoint needs rate limiting. \
                   Implement a token bucket algorithm with 100 req/min. \
                   Return 429 when exceeded."
                    .to_string(),
            ),
        );
        assert!(score >= 15);
    }

    #[test]
    fn test_label_dimension() {
        let picker = IssuePicker::with_defaults();
        let score = picker.label_dimension(&["bug".to_string()]);
        assert!(score >= 10);

        let score = picker.label_dimension(&["documentation".to_string()]);
        assert_eq!(score, 0);

        let score = picker.label_dimension(&["bug".to_string(), "high-priority".to_string()]);
        assert!(score >= 15);
    }

    #[test]
    fn test_dependency_dimension() {
        let picker = IssuePicker::with_defaults();
        let adj = picker.dependency_dimension(&[]);
        assert_eq!(adj, 0);

        let adj = picker.dependency_dimension(&["blocked".to_string()]);
        assert!(adj < 0);
    }

    #[test]
    fn test_pr_link_dimension() {
        let picker = IssuePicker::with_defaults();
        let bonus =
            picker.pr_link_dimension(&Some("This is blocked on PR #42 being merged".to_string()));
        assert_eq!(bonus, 20);

        let bonus = picker.pr_link_dimension(&None);
        assert_eq!(bonus, 0);
    }

    #[test]
    fn test_picker_selects_best() {
        let mut picker = IssuePicker::with_defaults();
        picker.config.min_pick_score = 30;
        let issues = vec![
            make_issue("1", Some(1), "small fix", Some("typo"), &[], None),
            make_issue(
                "2",
                Some(2),
                "implement rate limiting",
                Some(
                    "Add token bucket rate limiting to the API. \
                     Return 429 on exceeded. Use 100 req/min limit.",
                ),
                &["bug"],
                None,
            ),
        ];

        let decision = picker.pick(&issues);
        assert!(decision.has_selection());
        assert_eq!(decision.selected_issue.as_ref().unwrap().id, "2");
    }

    #[test]
    fn test_picker_defers_below_threshold() {
        let picker = IssuePicker {
            config: IssuePickerConfig {
                min_pick_score: 95,
                ..IssuePickerConfig::default()
            },
        };
        let issues = vec![make_issue("1", Some(1), "small fix", None, &[], None)];

        let decision = picker.pick(&issues);
        assert!(!decision.has_selection());
        assert!(decision.deferral_reason.is_some());
    }

    #[test]
    fn test_picker_skips_assigned() {
        let picker = IssuePicker::with_defaults();
        let issues = vec![make_issue(
            "1",
            Some(1),
            "bug",
            None,
            &["bug"],
            Some("someone"),
        )];

        let decision = picker.pick(&issues);
        assert!(!decision.has_selection());
        assert!(decision.scored().is_empty());
    }

    #[test]
    fn test_picker_skips_blocked_labels() {
        let picker = IssuePicker::with_defaults();
        let issues = vec![make_issue(
            "1",
            Some(1),
            "blocked feature",
            Some("blocked by PR #10"),
            &["blocked", "bug"],
            None,
        )];

        let decision = picker.pick(&issues);
        let score = decision
            .scored()
            .first()
            .map(|(_, s)| s.dependency_adjustment);
        assert!(score.unwrap_or(0) < 0);
    }

    #[test]
    fn test_issue_priority_score_properties() {
        let score = IssuePriorityScore {
            total: 75,
            size_score: 20,
            label_score: 15,
            dependency_adjustment: 0,
            recency_score: 10,
            assignee_score: 10,
            pr_link_score: 20,
            reasoning: vec!["labels match".to_string()],
        };
        assert!(score.is_pickable());

        let low = IssuePriorityScore {
            total: 30,
            ..score.clone()
        };
        assert!(!low.is_pickable());
    }
}
