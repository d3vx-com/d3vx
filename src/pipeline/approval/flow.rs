//! Approval flow orchestration
//!
//! Manages the plan approval lifecycle: plan creation, user review,
//! decision processing, and auto-approval logic.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Notify, RwLock};
use tracing::{debug, info, warn};

use super::types::*;

/// Manages the approval flow for execution plans
pub struct ApprovalFlow {
    /// Configuration
    config: ApprovalConfig,
    /// Pending and completed approvals keyed by plan ID
    approvals: Arc<RwLock<HashMap<String, ApprovalEntry>>>,
    /// Notifiers for plans awaiting decision
    notifiers: Arc<RwLock<HashMap<String, Arc<Notify>>>>,
}

/// Internal tracking entry for an approval request
#[derive(Debug, Clone)]
struct ApprovalEntry {
    plan: ExecutionPlan,
    state: ApprovalState,
    feedback: Option<String>,
    created_at: std::time::Instant,
}

/// Result of submitting a plan for approval
#[derive(Debug, Clone)]
pub enum SubmitResult {
    /// Plan was auto-approved
    AutoApproved { plan_id: String },
    /// Plan is awaiting user approval
    PendingReview { plan_id: String, display: String },
    /// Approval is disabled, plan proceeds
    ApprovalDisabled { plan_id: String },
}

impl ApprovalFlow {
    /// Create a new approval flow with the given configuration
    pub fn new(config: ApprovalConfig) -> Self {
        Self {
            config,
            approvals: Arc::new(RwLock::new(HashMap::new())),
            notifiers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(ApprovalConfig::default())
    }

    /// Submit a plan for approval
    ///
    /// Returns immediately if auto-approved. Otherwise waits for a user decision
    /// (with timeout from config).
    pub async fn submit(&self, plan: ExecutionPlan) -> Result<ApprovalState, ApprovalError> {
        // If approval is disabled, auto-proceed
        if !self.config.require_approval {
            info!(plan_id = %plan.id, "Approval disabled, auto-proceeding");
            return Ok(ApprovalState::Approved);
        }

        // Check auto-approval criteria
        if plan.qualifies_for_auto_approval(&self.config) {
            info!(plan_id = %plan.id, "Plan auto-approved (low risk/complexity)");
            let entry = ApprovalEntry {
                state: ApprovalState::Approved,
                plan: plan.clone(),
                feedback: None,
                created_at: std::time::Instant::now(),
            };
            self.approvals.write().await.insert(plan.id.clone(), entry);
            return Ok(ApprovalState::Approved);
        }

        // Register pending approval
        let plan_id = plan.id.clone();
        let _display = plan.format_for_display();
        let notify = Arc::new(Notify::new());

        let entry = ApprovalEntry {
            state: ApprovalState::Pending,
            plan,
            feedback: None,
            created_at: std::time::Instant::now(),
        };

        self.approvals.write().await.insert(plan_id.clone(), entry);
        self.notifiers
            .write()
            .await
            .insert(plan_id.clone(), notify.clone());

        info!(plan_id = %plan_id, "Plan awaiting approval");

        // Wait for decision with timeout
        let timeout = Duration::from_secs(self.config.timeout_secs);
        let pid_for_lookup = plan_id.clone();
        let pid_for_cleanup = plan_id.clone();
        let result = tokio::select! {
            _ = notify.notified() => {
                let approvals = self.approvals.read().await;
                match approvals.get(&pid_for_lookup) {
                    Some(entry) => Ok(entry.state),
                    None => Err(ApprovalError::NotFound { plan_id }),
                }
            }
            _ = tokio::time::sleep(timeout) => {
                warn!(plan_id = %pid_for_lookup, "Approval timed out");
                let mut approvals = self.approvals.write().await;
                if let Some(entry) = approvals.get_mut(&pid_for_lookup) {
                    entry.state = ApprovalState::Expired;
                }
                Err(ApprovalError::Timeout { timeout })
            }
        };

        // Cleanup notifier
        self.notifiers.write().await.remove(&pid_for_cleanup);

        result
    }

    /// Submit a plan without waiting (non-blocking)
    pub async fn submit_async(&self, plan: ExecutionPlan) -> Result<SubmitResult, ApprovalError> {
        if !self.config.require_approval {
            return Ok(SubmitResult::ApprovalDisabled {
                plan_id: plan.id.clone(),
            });
        }

        if plan.qualifies_for_auto_approval(&self.config) {
            let plan_id = plan.id.clone();
            let entry = ApprovalEntry {
                state: ApprovalState::Approved,
                plan,
                feedback: None,
                created_at: std::time::Instant::now(),
            };
            self.approvals.write().await.insert(plan_id.clone(), entry);
            return Ok(SubmitResult::AutoApproved { plan_id });
        }

        let plan_id = plan.id.clone();
        let display = plan.format_for_display();
        let notify = Arc::new(Notify::new());

        let entry = ApprovalEntry {
            state: ApprovalState::Pending,
            plan,
            feedback: None,
            created_at: std::time::Instant::now(),
        };

        self.approvals.write().await.insert(plan_id.clone(), entry);
        self.notifiers.write().await.insert(plan_id.clone(), notify);

        Ok(SubmitResult::PendingReview { plan_id, display })
    }

    /// Record a user decision on a pending plan
    pub async fn decide(&self, decision: ApprovalDecision) -> Result<(), ApprovalError> {
        let plan_id = &decision.plan_id;

        let mut approvals = self.approvals.write().await;
        let entry = approvals
            .get_mut(plan_id)
            .ok_or_else(|| ApprovalError::NotFound {
                plan_id: plan_id.clone(),
            })?;

        // Validate transition
        if !entry.state.valid_transitions().contains(&decision.state) {
            return Err(ApprovalError::InvalidTransition {
                from: entry.state.to_string(),
                to: decision.state.to_string(),
            });
        }

        info!(plan_id = %plan_id, state = %decision.state, "Approval decision recorded");

        entry.state = decision.state;
        entry.feedback = decision.feedback;

        // Notify anyone waiting
        drop(approvals);
        if let Some(notify) = self.notifiers.read().await.get(plan_id) {
            notify.notify_one();
        }

        Ok(())
    }

    /// Get the current state of a plan
    pub async fn get_state(&self, plan_id: &str) -> Option<ApprovalState> {
        let approvals = self.approvals.read().await;
        approvals.get(plan_id).map(|e| e.state)
    }

    /// Get feedback for a plan (if any)
    pub async fn get_feedback(&self, plan_id: &str) -> Option<String> {
        let approvals = self.approvals.read().await;
        approvals.get(plan_id).and_then(|e| e.feedback.clone())
    }

    /// Check if a plan can proceed to execution
    pub async fn can_execute(&self, plan_id: &str) -> bool {
        let approvals = self.approvals.read().await;
        approvals
            .get(plan_id)
            .map(|e| e.state.is_executable())
            .unwrap_or(false)
    }

    /// List all pending approvals
    pub async fn pending_plans(&self) -> Vec<(String, String)> {
        let approvals = self.approvals.read().await;
        approvals
            .iter()
            .filter(|(_, e)| e.state == ApprovalState::Pending)
            .map(|(id, e)| (id.clone(), e.plan.summary.clone()))
            .collect()
    }

    /// Clear expired approvals
    pub async fn cleanup_expired(&self) -> usize {
        let timeout = Duration::from_secs(self.config.timeout_secs);
        let mut approvals = self.approvals.write().await;
        let now = std::time::Instant::now();

        let expired: Vec<String> = approvals
            .iter()
            .filter(|(_, e)| {
                e.state == ApprovalState::Pending && now.duration_since(e.created_at) > timeout
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();
        for id in &expired {
            if let Some(entry) = approvals.get_mut(id) {
                entry.state = ApprovalState::Expired;
            }
        }

        debug!(expired = count, "Cleaned up expired approvals");
        count
    }
}
