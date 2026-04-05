//! Core types for task decomposition

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::pipeline::phases::{Phase, Priority, TaskStatus};

/// Unique identifier for a decomposition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecompositionId(pub u64);

impl std::fmt::Display for DecompositionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "decomp-{}", self.0)
    }
}

/// Status of a decomposition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecompositionStatus {
    /// Plan created but not yet approved
    Planned,
    /// Plan approved, awaiting execution
    Approved,
    /// Currently executing child tasks
    Executing,
    /// All children completed successfully
    Completed,
    /// One or more children failed
    Failed,
    /// Partially completed (some succeeded, some failed)
    Partial,
    /// Cancelled by user or system
    Cancelled,
}

/// Definition of a child task within a decomposition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildTaskDefinition {
    /// Unique key for this child (e.g., "backend", "frontend", "tests")
    pub key: String,
    /// Task title
    pub title: String,
    /// Task instruction/description
    pub instruction: String,
    /// Estimated complexity (0.0 - 1.0)
    pub complexity: f64,
    /// Keys of children this depends on
    pub depends_on: Vec<String>,
    /// Estimated duration in minutes
    pub estimated_duration: Option<u32>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Priority override (None means inherit from parent)
    pub priority: Option<Priority>,
    /// Initial phase override
    pub initial_phase: Option<Phase>,
}

/// Status of a child task in execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildTaskStatus {
    /// Child key
    pub key: String,
    /// Assigned task ID (if created)
    pub task_id: Option<String>,
    /// Current status
    pub status: TaskStatus,
    /// Result summary (if completed)
    pub result: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Execution start time
    pub started_at: Option<String>,
    /// Execution end time
    pub completed_at: Option<String>,
}

/// Strategy for how children should be executed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStrategy {
    /// Execute all children in parallel (max concurrency)
    Parallel,
    /// Execute children one at a time
    Sequential,
    /// Execute based on dependency graph (respect dependencies)
    DependencyOrder,
    /// Execute up to N children in parallel
    LimitedParallel(usize),
}

impl Default for ExecutionStrategy {
    fn default() -> Self {
        ExecutionStrategy::DependencyOrder
    }
}

/// Strategy for aggregating child results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregationStrategy {
    /// All children must succeed
    AllSuccess,
    /// Majority must succeed (>50%)
    MajoritySuccess,
    /// Any child success is sufficient
    AnySuccess,
    /// Custom logic (specified per decomposition)
    Custom,
}

impl Default for AggregationStrategy {
    fn default() -> Self {
        AggregationStrategy::AllSuccess
    }
}

/// A decomposition plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompositionPlan {
    /// Unique ID for this decomposition
    pub id: DecompositionId,
    /// Parent task ID
    pub parent_task_id: String,
    /// Child task definitions
    pub children: Vec<ChildTaskDefinition>,
    /// Execution strategy
    pub execution_strategy: ExecutionStrategy,
    /// Aggregation strategy
    pub aggregation_strategy: AggregationStrategy,
    /// Current status
    pub status: DecompositionStatus,
    /// Maximum parallelism (for LimitedParallel)
    pub max_parallelism: usize,
    /// Creation timestamp
    pub created_at: String,
    /// Approval timestamp
    pub approved_at: Option<String>,
    /// Execution start timestamp
    pub execution_started_at: Option<String>,
    /// Completion timestamp
    pub completed_at: Option<String>,
    /// Final aggregated result
    pub final_result: Option<String>,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl DecompositionPlan {
    /// Create a new decomposition plan
    pub fn new(parent_task_id: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = DecompositionId(COUNTER.fetch_add(1, Ordering::SeqCst));
        let now = chrono::Utc::now().to_rfc3339();

        Self {
            id,
            parent_task_id: parent_task_id.to_string(),
            children: Vec::new(),
            execution_strategy: ExecutionStrategy::default(),
            aggregation_strategy: AggregationStrategy::default(),
            status: DecompositionStatus::Planned,
            max_parallelism: 3,
            created_at: now,
            approved_at: None,
            execution_started_at: None,
            completed_at: None,
            final_result: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a child task definition (mutating version)
    pub fn add_child(&mut self, child: ChildTaskDefinition) {
        self.children.push(child);
    }

    /// Add a child task definition (builder pattern)
    pub fn with_child(mut self, child: ChildTaskDefinition) -> Self {
        self.children.push(child);
        self
    }

    /// Set execution strategy
    pub fn with_execution_strategy(mut self, strategy: ExecutionStrategy) -> Self {
        self.execution_strategy = strategy;
        self
    }

    /// Set aggregation strategy
    pub fn with_aggregation_strategy(mut self, strategy: AggregationStrategy) -> Self {
        self.aggregation_strategy = strategy;
        self
    }

    /// Set max parallelism
    pub fn with_max_parallelism(mut self, max: usize) -> Self {
        self.max_parallelism = max;
        self
    }

    /// Approve the plan
    pub fn approve(&mut self) {
        self.status = DecompositionStatus::Approved;
        self.approved_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Start execution
    pub fn start_execution(&mut self) {
        self.status = DecompositionStatus::Executing;
        self.execution_started_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark as completed
    pub fn complete(&mut self, result: String) {
        self.status = DecompositionStatus::Completed;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.final_result = Some(result);
    }

    /// Mark as failed
    pub fn fail(&mut self, error: &str) {
        self.status = DecompositionStatus::Failed;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.final_result = Some(format!("Failed: {}", error));
    }

    /// Mark as partial
    pub fn partial(&mut self, result: String) {
        self.status = DecompositionStatus::Partial;
        self.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.final_result = Some(result);
    }

    /// Get children with no dependencies (can start immediately)
    pub fn get_root_children(&self) -> Vec<&ChildTaskDefinition> {
        self.children
            .iter()
            .filter(|c| c.depends_on.is_empty())
            .collect()
    }

    /// Get children that depend on a specific key
    pub fn get_dependent_children(&self, key: &str) -> Vec<&ChildTaskDefinition> {
        self.children
            .iter()
            .filter(|c| c.depends_on.contains(&key.to_string()))
            .collect()
    }

    /// Check if all dependencies are satisfied
    pub fn are_dependencies_satisfied(
        &self,
        child: &ChildTaskDefinition,
        completed_keys: &HashSet<String>,
    ) -> bool {
        child
            .depends_on
            .iter()
            .all(|dep| completed_keys.contains(dep))
    }
}
