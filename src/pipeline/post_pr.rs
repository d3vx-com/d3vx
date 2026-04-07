//! Post-PR Workflow
//!
//! After a PR is created by the pipeline, this module runs two autonomous loops:
//!
//! 1. **CI Fix Loop** — monitors CI status and retries on failure
//! 2. **Review Response Loop** — addresses review comments and pushes fixes
//!
//! Both use `gh` CLI under the hood and are designed for daemon-mode overnight work.

use std::sync::Arc;

use tracing::{info, warn};

use super::pr_ci_loop::{CiFixConfig, CiFixLoop};
use super::review_response::ReviewResponseLoop;
use super::reaction::{ReactionEvent, ReactionType};
use super::orchestrator::reaction_bridge::ReactionBridge;

/// Result of the full post-PR workflow.
#[derive(Debug)]
pub struct PostPrOutcome {
    /// Whether CI is green.
    pub ci_green: bool,
    /// Whether all review comments were addressed.
    pub reviews_addressed: bool,
    /// Number of CI fix attempts made.
    pub ci_fix_attempts: u32,
    /// Number of review response attempts made.
    pub review_attempts: u32,
    /// Human-readable summary.
    pub summary: String,
}

/// Runs the post-PR autonomous workflow: CI fix loop + review response loop.
pub struct PostPrWorkflow {
    repository: String,
    pr_number: u64,
    ci_config: CiFixConfig,
    max_review_attempts: u32,
}

impl PostPrWorkflow {
    /// Create a new workflow for a given PR.
    pub fn new(repository: String, pr_number: u64) -> Self {
        Self {
            repository,
            pr_number,
            ci_config: CiFixConfig::default(),
            max_review_attempts: 3,
        }
    }

    /// Override CI fix configuration.
    pub fn with_ci_config(mut self, config: CiFixConfig) -> Self {
        self.ci_config = config;
        self
    }

    /// Override max review response attempts.
    pub fn with_max_review_attempts(mut self, max: u32) -> Self {
        self.max_review_attempts = max;
        self
    }

    /// Run the full post-PR workflow.
    ///
    /// 1. Wait for CI, auto-fix on failure.
    /// 2. Check for review comments, address them.
    ///
    /// The `on_ci_fix` closure receives failing check names and should
    /// apply fixes and push to the PR branch. Returns `true` if changes
    /// were made.
    ///
    /// The `on_review_feedback` closure receives actionable feedback and
    /// should make code changes. Returns `true` if changes were made.
    ///
    /// The `push_changes` closure pushes the current branch to origin.
    pub async fn run(
        &mut self,
        on_ci_fix: impl FnMut(Vec<String>) -> bool,
        on_review_feedback: impl Fn(&[super::review_response::ActionableFeedback]) -> bool,
        push_changes: impl Fn() -> anyhow::Result<()>,
    ) -> PostPrOutcome {
        info!(
            "Starting post-PR workflow for PR #{} in {}",
            self.pr_number, self.repository
        );

        // Phase 1: CI fix loop
        let ci_result = self.run_ci_fix(on_ci_fix).await;

        // Phase 2: Review response loop (only if CI is green)
        let review_result = if ci_result.is_green {
            self.run_review_response(on_review_feedback, push_changes).await
        } else {
            info!("Skipping review response — CI not green");
            ReviewPhaseResult::skipped()
        };

        let summary = format!(
            "PR #{}: CI {} ({} fix attempts), Reviews {} ({} attempts)",
            self.pr_number,
            if ci_result.is_green { "green" } else { "failed" },
            ci_result.fix_attempts,
            if review_result.addressed { "addressed" } else { "pending" },
            review_result.attempts,
        );

        PostPrOutcome {
            ci_green: ci_result.is_green,
            reviews_addressed: review_result.addressed,
            ci_fix_attempts: ci_result.fix_attempts,
            review_attempts: review_result.attempts,
            summary,
        }
    }

    /// Run only the CI fix loop.
    async fn run_ci_fix(
        &mut self,
        mut on_fix: impl FnMut(Vec<String>) -> bool,
    ) -> super::pr_ci_loop::CiFixResult {
        let mut ci_loop = CiFixLoop::new(
            self.ci_config.clone(),
            self.repository.clone(),
            self.pr_number,
        );

        info!("Starting CI fix loop for PR #{}", self.pr_number);
        ci_loop.run(|failing| on_fix(failing)).await
    }

    /// Run only the review response loop.
    async fn run_review_response(
        &mut self,
        on_feedback: impl Fn(&[super::review_response::ActionableFeedback]) -> bool,
        push_changes: impl Fn() -> anyhow::Result<()>,
    ) -> ReviewPhaseResult {
        let mut review_loop = ReviewResponseLoop::new(
            self.repository.clone(),
            self.pr_number,
        )
        .with_max_attempts(self.max_review_attempts);

        let report = match review_loop.fetch_review_report().await {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to fetch review report for PR #{}: {}", self.pr_number, e);
                return ReviewPhaseResult::skipped();
            }
        };

        if !report.needs_changes() {
            info!("No blocking reviews on PR #{}", self.pr_number);
            return ReviewPhaseResult::addressed(0);
        }

        info!(
            "Found {} actionable review items on PR #{}",
            report.actionable.len(),
            self.pr_number,
        );

        let result = review_loop
            .address_comments(&report, on_feedback, || push_changes())
            .await;

        match result {
            Ok(r) if r.success => {
                info!("Review response: {}", r.summary);
                ReviewPhaseResult::addressed(1)
            }
            Ok(r) => {
                warn!("Review response incomplete: {}", r.summary);
                ReviewPhaseResult::failed(1, r.summary)
            }
            Err(e) => {
                warn!("Review response error: {}", e);
                ReviewPhaseResult::failed(1, e.to_string())
            }
        }
    }
}

/// Internal result for the review phase.
struct ReviewPhaseResult {
    addressed: bool,
    attempts: u32,
}

impl ReviewPhaseResult {
    fn addressed(attempts: u32) -> Self {
        Self { addressed: true, attempts }
    }
    fn skipped() -> Self {
        Self { addressed: true, attempts: 0 }
    }
    fn failed(attempts: u32, _reason: String) -> Self {
        Self { addressed: false, attempts }
    }
}

/// Emit reaction events for a completed post-PR workflow.
///
/// Called by the daemon after the workflow finishes to feed results
/// back into the reaction engine for escalation/notification.
pub async fn emit_post_pr_events(
    bridge: &Arc<ReactionBridge>,
    outcome: &PostPrOutcome,
    task_id: &str,
) {
    if !outcome.ci_green {
        bridge
            .on_ci_failure(ReactionEvent::CIFailure {
                repository: String::new(),
                branch: String::new(),
                commit_sha: String::new(),
                context: "post-pr-ci".to_string(),
                description: format!(
                    "CI still failing after {} fix attempts",
                    outcome.ci_fix_attempts
                ),
                target_url: None,
                task_id: Some(task_id.to_string()),
            })
            .await;
    }
}
