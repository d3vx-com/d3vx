//! Tests for GitHub Event Types
//!
//! Covers event parsing, serialization, and normalization.

#[cfg(test)]
mod tests {
    use crate::pipeline::github::{
        GitHubEvent, CIStatus, CheckStatus, CheckOutput,
    };
    use serde_json::json;

    // =========================================================================
    // GitHub Event Serialization Tests
    // =========================================================================

    #[test]
    fn test_issue_opened_event() {
        let event = GitHubEvent::IssueOpened {
            number: 42,
            repository: "owner/repo".to_string(),
            author: "developer".to_string(),
            title: "Bug report".to_string(),
            body: Some("Something is broken".to_string()),
            labels: vec!["bug".to_string()],
        };

        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("issue_opened"));
        assert!(json.contains("42"));
        assert!(json.contains("owner/repo"));
    }

    #[test]
    fn test_issue_labeled_event() {
        let event = GitHubEvent::IssueLabeled {
            number: 10,
            repository: "owner/repo".to_string(),
            label: "d3vx-auto".to_string(),
            actor: "bot".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("issue_labeled"));
        assert!(json.contains("d3vx-auto"));
    }

    #[test]
    fn test_issue_closed_event() {
        let event = GitHubEvent::IssueClosed {
            number: 15,
            repository: "owner/repo".to_string(),
            actor: "maintainer".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("issue_closed"));
    }

    #[test]
    fn test_pr_review_requested_event() {
        let event = GitHubEvent::PRReviewRequested {
            number: 100,
            repository: "owner/repo".to_string(),
            author: "contributor".to_string(),
            title: "New feature".to_string(),
            requested_reviewer: "reviewer".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("pr_review_requested"));
        assert!(json.contains("reviewer"));
    }

    #[test]
    fn test_pr_comment_event() {
        let event = GitHubEvent::PRComment {
            number: 50,
            comment_id: 12345,
            repository: "owner/repo".to_string(),
            author: "commenter".to_string(),
            body: "Looks good to me!".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("pr_comment"));
        assert!(json.contains("12345"));
    }

    #[test]
    fn test_pr_changes_requested_event() {
        let event = GitHubEvent::PRChangesRequested {
            number: 75,
            repository: "owner/repo".to_string(),
            reviewer: "senior-dev".to_string(),
            comment: Some("Please fix the error handling".to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("pr_changes_requested"));
    }

    #[test]
    fn test_ci_status_changed_event() {
        let event = GitHubEvent::CIStatusChanged {
            repository: "owner/repo".to_string(),
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
            status: CIStatus::Success,
            context: "ci/tests".to_string(),
            description: Some("All tests passed".to_string()),
            target_url: Some("https://ci.example.com/build/123".to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("ci_status_changed"));
        assert!(json.contains("success"));
    }

    #[test]
    fn test_check_run_completed_event() {
        let event = GitHubEvent::CheckRunCompleted {
            repository: "owner/repo".to_string(),
            branch: "feature".to_string(),
            commit_sha: "def456".to_string(),
            check_name: "security-scan".to_string(),
            status: CheckStatus::Completed,
            conclusion: Some("success".to_string()),
            output: Some(CheckOutput {
                title: Some("Security Scan Results".to_string()),
                summary: Some("No vulnerabilities found".to_string()),
                text: Some("Detailed report...".to_string()),
            }),
        };

        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("check_run_completed"));
    }

    // =========================================================================
    // CI Status Tests
    // =========================================================================

    #[test]
    fn test_ci_status_variants() {
        let statuses = vec![
            (CIStatus::Pending, "pending"),
            (CIStatus::Success, "success"),
            (CIStatus::Failure, "failure"),
            (CIStatus::Error, "error"),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert!(json.contains(expected));
        }
    }

    #[test]
    fn test_ci_status_deserialization() {
        let pending: CIStatus = serde_json::from_str("\"pending\"").unwrap();
        assert_eq!(pending, CIStatus::Pending);

        let success: CIStatus = serde_json::from_str("\"success\"").unwrap();
        assert_eq!(success, CIStatus::Success);
    }

    // =========================================================================
    // Check Status Tests
    // =========================================================================

    #[test]
    fn test_check_status_variants() {
        let statuses = vec![
            (CheckStatus::Queued, "queued"),
            (CheckStatus::InProgress, "in_progress"),
            (CheckStatus::Completed, "completed"),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert!(json.contains(expected));
        }
    }

    // =========================================================================
    // Round-Trip Tests
    // =========================================================================

    #[test]
    fn test_event_round_trip() {
        let original = GitHubEvent::IssueOpened {
            number: 1,
            repository: "test/repo".to_string(),
            author: "user".to_string(),
            title: "Test Issue".to_string(),
            body: Some("Body".to_string()),
            labels: vec!["label1".to_string(), "label2".to_string()],
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: GitHubEvent = serde_json::from_str(&json).unwrap();

        if let GitHubEvent::IssueOpened { number, repository, .. } = parsed {
            assert_eq!(number, 1);
            assert_eq!(repository, "test/repo");
        } else {
            panic!("Wrong event type");
        }
    }
}
