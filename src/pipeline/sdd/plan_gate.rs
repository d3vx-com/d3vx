//! Plan gate — hard gate between "plan" and "execute" phases
//!
//! Wraps the existing `Planner` + `ApprovalFlow` so that no execution
//! can proceed without an approved `ExecutionPlan`.

use anyhow::{Context, Result};
use tracing::{info, warn};

use super::types::{SddConfig, SddError, SddState, TaskSpec};
use crate::pipeline::approval::{ApprovalConfig, ApprovalState, ExecutionPlan, Planner};
use crate::pipeline::phases::{PhaseContext, Task};

/// Hard gate: no execution without an approved plan.
///
/// For low-complexity tasks this auto-approves. For anything above the
/// threshold the user must explicitly approve or it times out.
pub struct PlanGate {
    planner: Planner,
    config: SddConfig,
}

impl PlanGate {
    pub fn new(sdd_config: SddConfig, approval_config: ApprovalConfig) -> Self {
        Self {
            planner: Planner::new(approval_config),
            config: sdd_config,
        }
    }

    /// With default approval config (auto-approve low risk, 10 min timeout)
    pub fn with_defaults() -> Self {
        Self::new(SddConfig::default(), ApprovalConfig::default())
    }

    /// Execute the plan gate: build plan, check thresholds, submit for approval.
    ///
    /// Returns the approved `ExecutionPlan` if the gate passes.
    pub async fn execute(
        &self,
        spec: &TaskSpec,
        task: &Task,
        context: &PhaseContext,
        plan_text: &str,
        session: &mut super::types::SddSession,
    ) -> Result<ExecutionPlan, SddError> {
        // 1. Build execution plan from plan phase output
        let mut plan = self.planner.build_plan(task, context, plan_text);

        // 2. Update plan with spec complexity (spec_extractor is more nuanced than planner heuristics)
        plan.complexity = spec.estimated_complexity;

        // Re-estimate risk from spec scope
        plan.risk_level = risk_from_scope(spec.scope, plan.steps.len(), plan.files_to_modify.len());

        // 3. Check SDD decomposition threshold
        if plan.complexity > self.config.decomposition_threshold {
            info!(
                complexity = plan.complexity,
                "Plan exceeds decomposition threshold, subagent decomposition will be required"
            );
        }

        // 4. Submit through approval flow
        info!(plan_id = %plan.id, "Submitting plan through approval gate");
        let approval_state = self
            .planner
            .submit_for_approval(plan.clone())
            .await
            .map_err(|e| {
                warn!(plan_id = %plan.id, error = %e, "Plan gate failed");
                SddError::PlanRejected(format!("approval flow error: {e}"))
            })?;

        // 5. Validate result
        match approval_state {
            ApprovalState::Approved | ApprovalState::ApprovedWithChanges => {
                info!(plan_id = %plan.id, state = %approval_state, "Plan gate passed");
                session.plan_id = Some(plan.id.clone());
                session
                    .transition(SddState::PlanApproved)
                    .map_err(|e| SddError::PlanRejected(format!("state transition error: {e}")))?;
                Ok(plan)
            }
            ApprovalState::Rejected => {
                return Err(SddError::PlanRejected(
                    "plan was rejected by user".to_string(),
                ));
            }
            ApprovalState::Expired => {
                return Err(SddError::PlanRejected(
                    "plan approval timed out".to_string(),
                ));
            }
            ApprovalState::Pending => {
                return Err(SddError::PlanRejected(
                    "plan approval still pending".to_string(),
                ));
            }
        }
    }

    /// Non-blocking submit — returns immediately with pending state
    pub async fn submit_non_blocking(
        &self,
        task: &Task,
        context: &PhaseContext,
        plan_text: &str,
    ) -> Result<crate::pipeline::approval::SubmitResult, SddError> {
        let plan = self.planner.build_plan(task, context, plan_text);
        self.planner
            .submit_async(plan)
            .await
            .context("failed to submit plan")
            .map_err(|e| SddError::PlanRejected(format!("submit error: {e}")))
    }

    /// Access the planner for manual decisions
    pub fn planner(&self) -> &Planner {
        &self.planner
    }
}

fn risk_from_scope(
    scope: crate::pipeline::sdd::types::Scope,
    steps: usize,
    files: usize,
) -> crate::pipeline::approval::RiskLevel {
    use crate::pipeline::approval::RiskLevel;
    use crate::pipeline::sdd::types::Scope;

    match scope {
        Scope::SingleFile if steps <= 2 => RiskLevel::Low,
        Scope::SingleFile => RiskLevel::Medium,
        Scope::NewFile if steps <= 3 => RiskLevel::Low,
        Scope::NewFile => RiskLevel::Medium,
        Scope::MultiFile if files <= 5 => RiskLevel::Medium,
        Scope::MultiFile => RiskLevel::High,
        Scope::Refactor => RiskLevel::High,
        Scope::Architecture => RiskLevel::High,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::phases::{Phase, Priority};

    fn make_task() -> Task {
        Task::new("TASK-TEST-001", "Test", "Do the thing")
            .with_phase(Phase::Research)
            .with_priority(Priority::Normal)
    }

    fn make_context() -> PhaseContext {
        PhaseContext::new(make_task(), "/tmp", "/tmp/work")
    }

    #[test]
    fn test_risk_from_scope() {
        use crate::pipeline::sdd::types::Scope;
        let single_file = Scope::SingleFile;
        let multi = Scope::MultiFile;
        let arch = Scope::Architecture;

        let r1 = risk_from_scope(single_file, 2, 1);
        assert!(matches!(r1, crate::pipeline::approval::RiskLevel::Low));

        let r2 = risk_from_scope(multi, 6, 8);
        assert!(matches!(r2, crate::pipeline::approval::RiskLevel::High));

        let r3 = risk_from_scope(arch, 10, 20);
        assert!(matches!(r3, crate::pipeline::approval::RiskLevel::High));
    }

    #[tokio::test]
    async fn test_auto_approve_below_threshold() {
        use super::super::spec_extractor::SpecExtractor;

        let gate = PlanGate::with_defaults();
        let task = make_task();
        let context = make_context();
        let mut session = super::super::types::SddSession::new("TASK-TEST-001");
        let spec = SpecExtractor::extract("Fix validation in @src/lib.rs");
        session.transition(SddState::SpecExtracted).unwrap();

        let result = gate
            .execute(
                &spec,
                &task,
                &context,
                "Fix validation in src/lib.rs",
                &mut session,
            )
            .await;

        // Low complexity tasks should auto-approve (default config)
        assert!(result.is_ok());
    }
}
