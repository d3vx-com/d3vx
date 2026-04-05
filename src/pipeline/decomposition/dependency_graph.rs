//! Dependency graph for execution ordering

use std::collections::{HashMap, HashSet};
use tracing::warn;

use super::types::DecompositionPlan;

/// Dependency graph for execution ordering
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// All nodes (child keys)
    nodes: HashSet<String>,
    /// Edges: key -> keys that depend on it
    dependents: HashMap<String, Vec<String>>,
    /// Reverse edges: key -> keys it depends on
    dependencies: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
            dependents: HashMap::new(),
            dependencies: HashMap::new(),
        }
    }

    /// Build from a decomposition plan
    pub fn from_plan(plan: &DecompositionPlan) -> Self {
        let mut graph = Self::new();

        for child in &plan.children {
            graph.add_node(&child.key);
            for dep in &child.depends_on {
                graph.add_dependency(&child.key, dep);
            }
        }

        graph
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, key: &str) {
        self.nodes.insert(key.to_string());
    }

    /// Add a dependency (child depends on dependency)
    pub fn add_dependency(&mut self, child: &str, depends_on: &str) {
        self.nodes.insert(child.to_string());
        self.nodes.insert(depends_on.to_string());

        self.dependents
            .entry(depends_on.to_string())
            .or_insert_with(Vec::new)
            .push(child.to_string());

        self.dependencies
            .entry(child.to_string())
            .or_insert_with(Vec::new)
            .push(depends_on.to_string());
    }

    /// Get nodes with no dependencies (can start immediately)
    pub fn get_roots(&self) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|n| !self.dependencies.contains_key(*n) || self.dependencies[*n].is_empty())
            .cloned()
            .collect()
    }

    /// Get nodes that depend on a specific node
    pub fn get_dependents(&self, key: &str) -> Vec<String> {
        self.dependents.get(key).cloned().unwrap_or_default()
    }

    /// Check if all dependencies are satisfied for a node
    pub fn are_dependencies_satisfied(&self, key: &str, completed: &HashSet<String>) -> bool {
        match self.dependencies.get(key) {
            Some(deps) => deps.iter().all(|d| completed.contains(d)),
            None => true,
        }
    }

    /// Get execution levels (topological sort)
    pub fn get_execution_levels(&self) -> Vec<Vec<String>> {
        let mut levels = Vec::new();
        let mut completed = HashSet::new();
        let mut remaining: HashSet<String> = self.nodes.clone();

        while !remaining.is_empty() {
            // Find all nodes whose dependencies are satisfied
            let ready: Vec<String> = remaining
                .iter()
                .filter(|n| self.are_dependencies_satisfied(n, &completed))
                .cloned()
                .collect();

            if ready.is_empty() {
                // Circular dependency - break by taking remaining nodes
                warn!("Circular dependency detected in decomposition graph");
                levels.push(remaining.iter().cloned().collect());
                break;
            }

            // Mark as completed
            for node in &ready {
                completed.insert(node.clone());
                remaining.remove(node);
            }

            levels.push(ready);
        }

        levels
    }

    /// Validate graph has no cycles
    pub fn validate(&self) -> Result<(), String> {
        let levels = self.get_execution_levels();
        let total_nodes: usize = levels.iter().map(|l| l.len()).sum();
        if total_nodes != self.nodes.len() {
            return Err("Graph validation failed: not all nodes reachable".to_string());
        }
        Ok(())
    }
}
