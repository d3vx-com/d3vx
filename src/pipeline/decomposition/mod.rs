//! Task Decomposition Module
//!
//! Provides decomposition of large tasks into smaller, parallelizable child tasks.
//! Enables:
//! - Planner-created child tasks based on task analysis
//! - Dependency graph for ordered execution
//! - Limited parallel execution of independent children
//! - Parent aggregation of child results

pub mod aggregator;
pub mod decomposer;
pub mod dependency_graph;
pub mod executor;
pub mod manager;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use aggregator::ResultAggregator;
pub use decomposer::TaskDecomposer;
pub use dependency_graph::DependencyGraph;
pub use executor::{ParallelExecutionError, ParallelExecutor};
pub use manager::{DecompositionError, DecompositionManager};
pub use types::{
    AggregationStrategy, ChildTaskDefinition, ChildTaskStatus, DecompositionId, DecompositionPlan,
    DecompositionStatus, ExecutionStrategy,
};
