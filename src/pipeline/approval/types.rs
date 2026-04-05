//! Approval flow type definitions
//!
//! Defines the data structures for the planner/executor approval gate.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Errors in the approval flow
#[derive(Debug, Error)]
pub enum ApprovalError {
    /// The plan was rejected
    #[error("Plan rejected: {reason}")]
    Rejected { reason: String },

    /// Approval request timed out
    #[error("Approval request timed out after {timeout:?}")]
    Timeout { timeout: Duration },

    /// Invalid state transition attempted
    #[error("Cannot transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    /// Plan not found
    #[error("Plan {plan_id} not found")]
    NotFound { plan_id: String },

    /// Plan already in review
    #[error("Plan {plan_id} is already under review")]
    AlreadyUnderReview { plan_id: String },
}

/// Current state of a plan approval request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    /// Plan has been generated, awaiting review
    Pending,
    /// User has approved the plan as-is
    Approved,
    /// User rejected the plan
    Rejected,
    /// User approved with modifications/feedback
    ApprovedWithChanges,
    /// Approval timed out
    Expired,
}

impl Default for ApprovalState {
    fn default() -> Self {
        ApprovalState::Pending
    }
}

impl fmt::Display for ApprovalState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApprovalState::Pending => write!(f, "pending"),
            ApprovalState::Approved => write!(f, "approved"),
            ApprovalState::Rejected => write!(f, "rejected"),
            ApprovalState::ApprovedWithChanges => write!(f, "approved_with_changes"),
            ApprovalState::Expired => write!(f, "expired"),
        }
    }
}

impl ApprovalState {
    /// Check if this state allows execution to proceed
    pub fn is_executable(&self) -> bool {
        matches!(
            self,
            ApprovalState::Approved | ApprovalState::ApprovedWithChanges
        )
    }

    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ApprovalState::Approved
                | ApprovalState::Rejected
                | ApprovalState::ApprovedWithChanges
                | ApprovalState::Expired
        )
    }

    /// Valid transitions from the current state
    pub fn valid_transitions(&self) -> &[ApprovalState] {
        match self {
            ApprovalState::Pending => &[
                ApprovalState::Approved,
                ApprovalState::Rejected,
                ApprovalState::ApprovedWithChanges,
                ApprovalState::Expired,
            ],
            ApprovalState::Approved
            | ApprovalState::Rejected
            | ApprovalState::ApprovedWithChanges
            | ApprovalState::Expired => &[],
        }
    }
}

/// A structured execution plan produced by the planner agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Unique plan identifier
    pub id: String,
    /// The task this plan is for
    pub task_id: String,
    /// High-level summary of the approach
    pub summary: String,
    /// Ordered list of implementation steps
    pub steps: Vec<PlanStep>,
    /// Files the plan expects to modify
    pub files_to_modify: Vec<String>,
    /// Files the plan expects to create
    pub files_to_create: Vec<String>,
    /// Risk assessment (low/medium/high)
    pub risk_level: RiskLevel,
    /// Estimated complexity score (0.0 - 1.0)
    pub complexity: f64,
    /// Timestamp when the plan was created
    pub created_at: u64,
}

/// Risk level for a plan
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
        }
    }
}

/// A single step in an execution plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Step number (1-indexed)
    pub step_number: usize,
    /// Short description of this step
    pub description: String,
    /// Files involved in this step
    pub files: Vec<String>,
    /// Whether this step can be parallelized
    pub parallelizable: bool,
}

/// An approval decision from the user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    /// The plan being decided on
    pub plan_id: String,
    /// The decision
    pub state: ApprovalState,
    /// Optional feedback/modification requests
    pub feedback: Option<String>,
    /// Timestamp of the decision
    pub decided_at: u64,
}

/// Configuration for the approval flow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalConfig {
    /// Whether approval is required before execution
    pub require_approval: bool,
    /// Timeout for approval requests (seconds)
    pub timeout_secs: u64,
    /// Auto-approve plans below this risk level
    pub auto_approve_below_risk: Option<RiskLevel>,
    /// Auto-approve plans below this complexity score
    pub auto_approve_below_complexity: Option<f64>,
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            require_approval: true,
            timeout_secs: 600, // 10 minutes
            auto_approve_below_risk: Some(RiskLevel::Low),
            auto_approve_below_complexity: Some(0.3),
        }
    }
}

impl ExecutionPlan {
    /// Create a new execution plan
    pub fn new(task_id: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            id: format!("plan-{}", uuid::Uuid::new_v4().as_simple()),
            task_id: task_id.into(),
            summary: summary.into(),
            steps: Vec::new(),
            files_to_modify: Vec::new(),
            files_to_create: Vec::new(),
            risk_level: RiskLevel::Medium,
            complexity: 0.5,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Add a step to the plan
    pub fn with_step(mut self, description: impl Into<String>, files: Vec<String>) -> Self {
        let step_number = self.steps.len() + 1;
        self.steps.push(PlanStep {
            step_number,
            description: description.into(),
            files,
            parallelizable: false,
        });
        self
    }

    /// Set the risk level
    pub fn with_risk(mut self, risk: RiskLevel) -> Self {
        self.risk_level = risk;
        self
    }

    /// Set the complexity score
    pub fn with_complexity(mut self, score: f64) -> Self {
        self.complexity = score.clamp(0.0, 1.0);
        self
    }

    /// Check if this plan qualifies for auto-approval under the given config
    pub fn qualifies_for_auto_approval(&self, config: &ApprovalConfig) -> bool {
        if let Some(max_risk) = &config.auto_approve_below_risk {
            match (self.risk_level, max_risk) {
                (RiskLevel::Low, _) => {}
                (RiskLevel::Medium, RiskLevel::High) => {}
                _ => return false,
            }
        }
        if let Some(max_complexity) = config.auto_approve_below_complexity {
            if self.complexity > max_complexity {
                return false;
            }
        }
        true
    }

    /// Format the plan for display to the user
    pub fn format_for_display(&self) -> String {
        let mut output = format!("Plan: {}\n", self.summary);
        output.push_str(&format!(
            "Risk: {} | Complexity: {:.0}%\n",
            self.risk_level,
            self.complexity * 100.0
        ));

        if !self.steps.is_empty() {
            output.push_str("\nSteps:\n");
            for step in &self.steps {
                output.push_str(&format!("  {}. {}\n", step.step_number, step.description));
                if !step.files.is_empty() {
                    output.push_str(&format!("     Files: {}\n", step.files.join(", ")));
                }
            }
        }

        if !self.files_to_modify.is_empty() {
            output.push_str(&format!(
                "\nFiles to modify: {}\n",
                self.files_to_modify.join(", ")
            ));
        }
        if !self.files_to_create.is_empty() {
            output.push_str(&format!(
                "Files to create: {}\n",
                self.files_to_create.join(", ")
            ));
        }

        output
    }
}
