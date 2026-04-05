//! Subagent-Driven Development (SDD) core types
//!
//! Defines the state machine, configuration, session, and error types
//! for the spec → plan → decompose → execute workflow.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Workflow states in the SDD lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SddState {
    /// No active SDD session
    Idle,
    /// Spec extracted from user input
    SpecExtracted,
    /// Plan approved and ready for decomposition
    PlanApproved,
    /// Decomposition plan created, ready to execute
    DecompositionCreated,
    /// Child agents are executing
    ChildrenExecuting,
    /// All children completed successfully
    ChildrenComplete,
    /// Integration and validation complete
    Integrated,
    /// One or more children failed
    Failed,
}

impl fmt::Display for SddState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SddState::Idle => write!(f, "idle"),
            SddState::SpecExtracted => write!(f, "spec_extracted"),
            SddState::PlanApproved => write!(f, "plan_approved"),
            SddState::DecompositionCreated => write!(f, "decomposition_created"),
            SddState::ChildrenExecuting => write!(f, "children_executing"),
            SddState::ChildrenComplete => write!(f, "children_complete"),
            SddState::Integrated => write!(f, "integrated"),
            SddState::Failed => write!(f, "failed"),
        }
    }
}

/// What scope of work this task requires
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Touches a single existing file
    SingleFile,
    /// Touches multiple existing files
    MultiFile,
    /// Creates new files
    NewFile,
    /// Structural refactoring across modules
    Refactor,
    /// New architecture/component design
    Architecture,
}

/// Structured specification extracted from user input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// What the user wants to achieve
    pub goal: String,
    /// Constraints (tech rules, patterns, forbidden actions)
    pub constraints: Vec<String>,
    /// Scope classification
    pub scope: Scope,
    /// Acceptance criteria derived from the request
    pub acceptance_criteria: Vec<String>,
    /// Estimated complexity 0.0–1.0
    pub estimated_complexity: f64,
    /// Whether subagent decomposition is warranted
    pub benefits_from_decomposition: bool,
}

/// An SDD session tracks the full lifecycle of one SDD workflow
#[derive(Debug, Clone)]
pub struct SddSession {
    /// Unique session ID
    pub session_id: String,
    /// Associated task ID
    pub task_id: String,
    /// Current workflow state
    pub state: SddState,
    /// Extracted specification
    pub spec: Option<TaskSpec>,
    /// Approved execution plan
    pub plan_id: Option<String>,
    /// Decomposition plan ID (if decomposition was triggered)
    pub decomposition_id: Option<String>,
    /// Child task results
    pub child_results: Vec<ChildResult>,
}

/// Result from one child agent in a decomposition
#[derive(Debug, Clone)]
pub struct ChildResult {
    /// Child key (e.g. "backend", "frontend")
    pub key: String,
    /// Whether the child completed successfully
    pub success: bool,
    /// Summary of what the child produced
    pub summary: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Files the child modified
    pub files_changed: Vec<String>,
}

/// Configuration for the SDD workflow
#[derive(Debug, Clone)]
pub struct SddConfig {
    /// Complexity threshold above which decomposition is required
    pub decomposition_threshold: f64,
    /// Maximum child agents per decomposition
    pub max_children: usize,
    /// Timeout in seconds for the full SDD workflow
    pub workflow_timeout_secs: u64,
}

impl Default for SddConfig {
    fn default() -> Self {
        Self {
            decomposition_threshold: 0.5,
            max_children: 5,
            workflow_timeout_secs: 3600, // 1 hour
        }
    }
}

/// Errors that can occur during SDD
#[derive(Debug, Error)]
pub enum SddError {
    #[error("Spec extraction failed: {0}")]
    SpecExtraction(String),

    #[error("Plan gate rejected: {0}")]
    PlanRejected(String),

    #[error("Decomposition failed: {0}")]
    Decomposition(String),

    #[error("Child execution failed: {0}")]
    ChildExecution(String),

    #[error("Integration failed: {0}")]
    Integration(String),

    #[error("Workflow timed out")]
    Timeout,

    #[error("Invalid state transition: {from} → {to}")]
    InvalidTransition { from: String, to: String },
}

impl SddSession {
    /// Create a new session
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            session_id: format!("sdd-{}", uuid::Uuid::new_v4().as_simple()),
            task_id: task_id.into(),
            state: SddState::Idle,
            spec: None,
            plan_id: None,
            decomposition_id: None,
            child_results: Vec::new(),
        }
    }

    /// Transition to a new state if valid
    pub fn transition(&mut self, new_state: SddState) -> Result<(), SddError> {
        if !self.valid_transition(new_state) {
            return Err(SddError::InvalidTransition {
                from: self.state.to_string(),
                to: new_state.to_string(),
            });
        }
        self.state = new_state;
        Ok(())
    }

    fn valid_transition(&self, to: SddState) -> bool {
        match (self.state, to) {
            (SddState::Idle, SddState::SpecExtracted) => true,
            (SddState::SpecExtracted, SddState::PlanApproved) => true,
            (SddState::PlanApproved, SddState::DecompositionCreated) => true,
            (SddState::PlanApproved, SddState::ChildrenExecuting) => true, // no decomp needed
            (SddState::DecompositionCreated, SddState::ChildrenExecuting) => true,
            (SddState::ChildrenExecuting, SddState::ChildrenComplete) => true,
            (SddState::ChildrenComplete, SddState::Integrated) => true,
            (_, SddState::Failed) => true,
            _ => false,
        }
    }
}
