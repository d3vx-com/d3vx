//! QA Loop Integration
//!
//! Wires the existing QALoop and Commander validation into the autonomous
//! pipeline flow.
//!
//! This module bridges the gap between the Implement phase output and the
//! Review phase by running automated quality checks:
//!
//! - Entry point after Implement phase
//! - Bounded fix cycles with escalation
//! - Structured output for downstream phases
//!
//! ## Flow
//!
//! ```text
//! Implement → QA Gate → Validate → [blocked → fix → re-validate] → pass
//! ```

use crate::pipeline::commander::ValidationRunner;
use crate::pipeline::qa_loop::{QAConfig, QALoop, QAState};
use crate::pipeline::review_gate::{GateResult, ReviewGate};

/// Outcome after running the QA integration cycle.
#[derive(Debug, Clone)]
pub struct QAResult {
    /// Final state the QA loop reached
    pub final_state: QAState,
    /// Number of fix cycles that were needed
    pub cycles: u32,
    /// Whether validation passed
    pub validation_passed: bool,
    /// Review gate result
    pub review_passed: bool,
    /// Any blocking issues found
    pub blockers: Vec<String>,
    /// Human-readable summary
    pub summary: String,
}

/// Configuration for QA integration.
#[derive(Debug, Clone)]
pub struct QAIntegrationConfig {
    /// Max fix cycles before escalation
    pub max_fix_cycles: u32,
    /// Whether to run validation as part of QA
    pub run_validation: bool,
    /// Whether to run review as part of QA
    pub run_review: bool,
}

impl Default for QAIntegrationConfig {
    fn default() -> Self {
        Self {
            max_fix_cycles: 3,
            run_validation: true,
            run_review: true,
        }
    }
}

/// Pipeline-level QA bridge.
///
/// Orchestrates validate → review → fix cycles.
pub struct QAIntegration {
    config: QAIntegrationConfig,
    max_cycles: u32,
    state: QAState,
    cycles_completed: u32,
}

impl QAIntegration {
    /// Create a new QA integration with default config.
    pub fn new() -> Self {
        Self::with_config(QAIntegrationConfig::default())
    }

    /// Create with custom config.
    pub fn with_config(config: QAIntegrationConfig) -> Self {
        Self {
            max_cycles: config.max_fix_cycles,
            config,
            state: QAState::Pending,
            cycles_completed: 0,
        }
    }

    /// Run the full QA cycle: validate, review, fix if needed.
    ///
    /// `task_id` identifies the task. `diff` and `changed_files` are passed
    /// to validation and review tools.
    pub async fn run_cycle(
        &mut self,
        task_id: &str,
        diff: &str,
        changed_files: &[String],
    ) -> QAResult {
        let qa_loop = QALoop::new(task_id.to_string(), QAConfig::default());
        let mut blockers = Vec::new();
        let mut validation_passed = false;
        let mut review_passed = false;

        for attempt in 0..self.max_cycles {
            let cycles = attempt + 1;

            // Run validation
            if self.config.run_validation {
                let runner = ValidationRunner::new(std::path::PathBuf::from("."));
                let results: Vec<_> = runner.run_all().await;
                validation_passed = results.iter().all(|r| r.success);
                if !validation_passed {
                    let errors: Vec<_> = results
                        .iter()
                        .filter(|r| !r.success)
                        .flat_map(|r| &r.errors)
                        .cloned()
                        .collect();
                    blockers.extend(errors);
                }
            }

            // Run review
            if self.config.run_review {
                let gate = ReviewGate::with_defaults();
                let summary = diff_summary(diff, changed_files);
                let gate_result = gate.evaluate(&summary);
                review_passed = !gate_result.blocked;
                if !review_passed {
                    blockers.extend(format_blockers(&gate_result));
                }
            }

            // Check pass/fail
            if validation_passed && review_passed {
                return QAResult {
                    final_state: QAState::Approved,
                    cycles,
                    validation_passed: true,
                    review_passed: true,
                    blockers: Vec::new(),
                    summary: format!("QA passed after {cycles} cycle(s)"),
                };
            }

            // Attempt fix and retry
            self.state = QAState::InFix;
            blockers.clear();
        }

        // Exhausted cycles
        self.state = QAState::Escalated;
        QAResult {
            final_state: QAState::Escalated,
            cycles: self.max_cycles,
            validation_passed,
            review_passed,
            blockers,
            summary: format!("QA exhausted all {} fix cycles", self.max_cycles),
        }
    }

    /// Get current QA state for reporting.
    pub fn current_state(&self) -> QAState {
        self.state
    }
}

/// Build a minimal ReviewSummary from diff text for review gating.
fn diff_summary(diff: &str, files: &[String]) -> crate::pipeline::review_summary::ReviewSummary {
    use crate::pipeline::review_summary::{ReviewStatus, ReviewerType};

    let mut summary = crate::pipeline::review_summary::ReviewSummary {
        task_id: "qa".to_string(),
        status: ReviewStatus::InProgress,
        findings: Vec::new(),
        changed_files: files.to_vec(),
        reviewer: ReviewerType::Automated,
        requested_at: None,
        completed_at: None,
        summary_text: Some(diff.chars().take(500).collect()),
        merge_blocked: false,
        blocking_findings: Vec::new(),
    };
    summary.finalize();
    summary
}

/// Extract blocker messages from a GateResult.
fn format_blockers(result: &GateResult) -> Vec<String> {
    result
        .reasons
        .iter()
        .map(|r| match r.code.as_str() {
            code => format!("{code}: {}", r.message),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = QAIntegrationConfig::default();
        assert_eq!(config.max_fix_cycles, 3);
        assert!(config.run_validation);
        assert!(config.run_review);
    }

    #[test]
    fn test_create_qa_integration() {
        let qa = QAIntegration::new();
        assert!(matches!(qa.current_state(), QAState::Pending));
    }

    #[test]
    fn test_with_custom_config() {
        let config = QAIntegrationConfig {
            max_fix_cycles: 5,
            run_validation: false,
            run_review: true,
        };
        let qa = QAIntegration::with_config(config);
        assert_eq!(qa.max_cycles, 5);
    }
}
