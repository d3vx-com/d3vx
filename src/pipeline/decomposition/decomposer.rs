//! Task decomposer - analyzes tasks and creates decomposition plans

use tracing::warn;

use super::types::{ChildTaskDefinition, DecompositionPlan, ExecutionStrategy};
use crate::pipeline::phases::Task;

/// Task decomposer - analyzes tasks and creates decomposition plans
pub struct TaskDecomposer {
    /// Maximum children per decomposition
    max_children: usize,
    /// Complexity threshold for decomposition (tasks above this get decomposed)
    pub complexity_threshold: f64,
}

impl TaskDecomposer {
    /// Create a new decomposer
    pub fn new() -> Self {
        Self {
            max_children: 10,
            complexity_threshold: 0.7,
        }
    }

    /// Create with custom settings
    pub fn with_settings(max_children: usize, complexity_threshold: f64) -> Self {
        Self {
            max_children,
            complexity_threshold,
        }
    }

    /// Analyze a task and determine if it should be decomposed
    pub fn should_decompose(&self, task: &Task) -> bool {
        // Check metadata for decomposition hints
        if let serde_json::Value::Object(map) = &task.metadata {
            if let Some(decompose) = map.get("should_decompose") {
                if let Some(should) = decompose.as_bool() {
                    return should;
                }
            }
            // Check complexity score
            if let Some(complexity) = map.get("complexity_score") {
                if let Some(score) = complexity.as_f64() {
                    return score > self.complexity_threshold;
                }
            }
        }

        false
    }

    /// Create a decomposition plan for a task
    pub fn create_plan(
        &self,
        task: &Task,
        mut child_definitions: Vec<ChildTaskDefinition>,
    ) -> DecompositionPlan {
        let mut plan = DecompositionPlan::new(&task.id);

        if child_definitions.len() > self.max_children {
            warn!(
                "Truncating decomposition for task {} from {} to {} children",
                task.id,
                child_definitions.len(),
                self.max_children
            );
            child_definitions.truncate(self.max_children);
        }

        // Add all children
        for child in child_definitions {
            plan.add_child(child);
        }

        // Determine execution strategy based on dependencies
        let has_dependencies = plan.children.iter().any(|c| !c.depends_on.is_empty());
        if !has_dependencies {
            plan.execution_strategy = ExecutionStrategy::Parallel;
        } else {
            plan.execution_strategy = ExecutionStrategy::DependencyOrder;
        }

        plan
    }

    /// Auto-decompose a task based on analysis
    /// This would typically be called by a planner agent
    pub fn auto_decompose(&self, task: &Task) -> Option<DecompositionPlan> {
        if !self.should_decompose(task) {
            return None;
        }

        // In a real implementation, this would use AI to analyze and decompose
        // For now, return None - the actual decomposition logic
        // would be provided by the planner phase
        None
    }
}

impl Default for TaskDecomposer {
    fn default() -> Self {
        Self::new()
    }
}
