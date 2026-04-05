//! Parallel Child Evaluation
//!
//! Heuristic evaluation of parallel child task results for candidate selection.

use crate::app::{CandidateEvaluation, ParallelChildTask};

/// Evaluate a parallel child task result for quality scoring
pub fn evaluate_parallel_child(child: &ParallelChildTask) -> CandidateEvaluation {
    let result = child.result.clone().unwrap_or_default().to_lowercase();
    let mut evaluation = CandidateEvaluation::default();

    // Changed file quality assessment
    evaluation.changed_file_quality += if child.ownership.is_some() { 3 } else { 1 };
    if result.contains("```") || result.contains("diff") || result.contains("patch") {
        evaluation.changed_file_quality += 3;
        evaluation
            .notes
            .push("Output references concrete code or patch details.".to_string());
    }

    // Test/lint outcome assessment
    if result.contains("test") || result.contains("tests") || result.contains("lint") {
        evaluation.test_lint_outcome += 3;
    }
    if result.contains("pass") || result.contains("passed") || result.contains("green") {
        evaluation.test_lint_outcome += 2;
        evaluation
            .notes
            .push("Candidate reports successful validation signals.".to_string());
    }
    if result.contains("fail") || result.contains("error") {
        evaluation.test_lint_outcome -= 2;
        evaluation
            .notes
            .push("Candidate mentions failing validation or runtime errors.".to_string());
    }

    // Documentation quality
    if child.specialist_role.to_ascii_lowercase().contains("doc")
        || result.contains("readme")
        || result.contains("docs")
        || result.contains("documentation")
    {
        evaluation.docs_completeness += 3;
    }

    // Conflict risk assessment
    if result.contains("merge conflict") || result.contains("conflict") {
        evaluation.conflict_risk -= 3;
        evaluation
            .notes
            .push("Candidate reports merge conflict risks.".to_string());
    }

    // Scope adherence
    if let Some(ref_ownership) = &child.ownership {
        if result.contains(ref_ownership) {
            evaluation.scope_adherence += 2;
        }
    }

    // Calculate total score
    evaluation.total_score = evaluation.changed_file_quality
        + evaluation.test_lint_outcome
        + evaluation.docs_completeness
        + evaluation.conflict_risk
        + evaluation.scope_adherence;

    evaluation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ParallelChildStatus;

    fn create_test_child(result: &str) -> ParallelChildTask {
        ParallelChildTask {
            key: "test".to_string(),
            description: "Test task".to_string(),
            task: "Test the code".to_string(),
            agent_type: "General".to_string(),
            specialist_role: "Executor".to_string(),
            depends_on: vec![],
            ownership: Some("src/main.rs".to_string()),
            task_id: None,
            agent_id: None,
            status: ParallelChildStatus::Completed,
            result: Some(result.to_string()),
            evaluation: None,
            progress: 100,
            blocked: false,
            blocker_reason: None,
            messages_sent: 0,
            messages_received: 0,
        }
    }

    #[test]
    fn test_evaluate_code_changes() {
        let child = create_test_child("```rust\nfn main() {}\n```\nTests passed!");
        let eval = evaluate_parallel_child(&child);

        assert!(eval.changed_file_quality > 0);
        assert!(eval.test_lint_outcome > 0);
    }

    #[test]
    fn test_evaluate_documentation() {
        let child = ParallelChildTask {
            key: "test".to_string(),
            description: "Docs task".to_string(),
            task: "Write docs".to_string(),
            agent_type: "General".to_string(),
            specialist_role: "Documenter".to_string(),
            depends_on: vec![],
            ownership: None,
            task_id: None,
            agent_id: None,
            status: ParallelChildStatus::Completed,
            result: Some("Updated README.md documentation".to_string()),
            evaluation: None,
            progress: 100,
            blocked: false,
            blocker_reason: None,
            messages_sent: 0,
            messages_received: 0,
        };
        let eval = evaluate_parallel_child(&child);

        assert!(eval.docs_completeness > 0);
    }

    #[test]
    fn test_evaluate_conflicts() {
        let child = create_test_child("Warning: merge conflict detected");
        let eval = evaluate_parallel_child(&child);

        assert!(eval.conflict_risk < 0);
    }
}
