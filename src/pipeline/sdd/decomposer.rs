//! SDD decomposer — creates a DecompositionPlan from an approved ExecutionPlan
//!
//! Analyzes plan steps and groups them into child tasks with dependency links,
//! so each child agent gets a constrained scope instead of the full plan.

use std::collections::{HashMap, HashSet};

use super::types::TaskSpec;
use crate::pipeline::approval::ExecutionPlan;
use crate::pipeline::decomposition::{
    AggregationStrategy, ChildTaskDefinition, DecompositionPlan, ExecutionStrategy,
};
use crate::pipeline::phases::Phase;

/// Creates decomposition plans from approved execution plans
pub struct SddDecomposer {
    /// Maximum children per decomposition
    max_children: usize,
}

impl SddDecomposer {
    pub fn new(max_children: usize) -> Self {
        Self { max_children }
    }

    pub fn with_defaults() -> Self {
        Self::new(5)
    }

    /// Decompose an approved plan into child tasks.
    ///
    /// Each child gets a constrained instruction (its own steps + relevant files),
    /// not the full plan. Dependencies are tracked so children run in order.
    pub fn decompose(&self, plan: &ExecutionPlan, _spec: &TaskSpec) -> DecompositionPlan {
        let mut decomp = DecompositionPlan::new(&plan.id);

        // Group steps into logical child tasks
        let groups = self.group_steps(plan);

        if groups.len() <= 1 {
            // Single group — no decomposition needed
            decomp.execution_strategy = ExecutionStrategy::Sequential;
            return decomp;
        }

        // Detect dependencies between groups (file sharing)
        let deps = self.detect_dependencies(&groups);

        // Build child definitions
        for (i, (group_key, steps)) in groups.iter().enumerate() {
            let instruction = steps
                .iter()
                .map(|s| {
                    format!(
                        "Step {}: {} (files: {})",
                        s.step_number,
                        s.description,
                        s.files.join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            let files: Vec<String> = steps
                .iter()
                .flat_map(|s| s.files.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();

            let is_new_file = steps.iter().any(|s| {
                s.description.to_lowercase().contains("create")
                    || s.description.to_lowercase().contains("new")
            });

            let initial_phase = if is_new_file {
                Some(Phase::Research)
            } else {
                None
            };

            let parent_deps = deps.get(group_key).cloned().unwrap_or_default();
            let depends_on: Vec<String> = parent_deps
                .into_iter()
                .map(|k| format!("child-{}", k))
                .collect();

            let child = ChildTaskDefinition {
                key: format!("child-{i}"),
                title: format!("Part {i}: {group_key}"),
                instruction,
                complexity: self.child_complexity(&steps),
                depends_on,
                estimated_duration: Some(self.estimate_duration(&steps)),
                tags: self.derive_tags(&files),
                priority: None, // inherit from parent
                initial_phase,
            };

            decomp.add_child(child);
        }

        // Decide execution strategy
        let has_deps = decomp.children.iter().any(|c| !c.depends_on.is_empty());
        decomp.execution_strategy = if has_deps {
            ExecutionStrategy::DependencyOrder
        } else {
            ExecutionStrategy::Parallel
        };

        // Aggregation: all children must succeed
        decomp.aggregation_strategy = AggregationStrategy::AllSuccess;
        decomp.max_parallelism = self.max_children.min(3);

        decomp
    }

    /// Group plan steps into logical child tasks.
    ///
    /// Strategy: group by primary file scope (e.g. all steps touching
    /// "src/auth/" go together, all "tests/" go together).
    fn group_steps(
        &self,
        plan: &ExecutionPlan,
    ) -> Vec<(String, Vec<crate::pipeline::approval::PlanStep>)> {
        self.group_steps_impl(&plan.steps)
    }

    fn group_steps_impl(
        &self,
        steps: &[crate::pipeline::approval::PlanStep],
    ) -> Vec<(String, Vec<crate::pipeline::approval::PlanStep>)> {
        if steps.is_empty() {
            return Vec::new();
        }
        if steps.len() <= 2 {
            return vec![("all".into(), steps.to_vec())];
        }

        // Group steps by their directory prefix
        let mut groups: HashMap<String, Vec<crate::pipeline::approval::PlanStep>> = HashMap::new();
        for step in steps {
            let prefix = if step.files.is_empty() {
                "misc".into()
            } else {
                dir_prefix(&step.files[0])
            };
            groups.entry(prefix).or_default().push(step.clone());
        }

        // Cap to max_children — merge overflow into "misc"
        if groups.len() > self.max_children {
            let mut sorted: Vec<_> = groups.into_iter().collect();
            let overflow: Vec<_> = sorted
                .drain(self.max_children - 1..)
                .flat_map(|(_, v)| v)
                .collect();
            groups = sorted.into_iter().collect();
            if !overflow.is_empty() {
                groups.insert("misc".into(), overflow);
            }
        }

        groups.into_iter().collect()
    }

    /// Detect file-sharing dependencies between groups.
    fn detect_dependencies(
        &self,
        groups: &[(String, Vec<crate::pipeline::approval::PlanStep>)],
    ) -> HashMap<String, Vec<usize>> {
        let mut deps: HashMap<String, Vec<usize>> = HashMap::new();

        // Build a map: file → group index that creates it
        let file_creators: HashMap<String, usize> = groups
            .iter()
            .enumerate()
            .flat_map(|(gi, (_, steps))| {
                steps
                    .iter()
                    .flat_map(move |s| {
                        s.description
                            .to_lowercase()
                            .contains("create")
                            .then_some(s.files.clone())
                            .unwrap_or_default()
                    })
                    .map(move |f| (f, gi))
            })
            .collect();

        // Check each group: does it modify files that another group creates?
        for (gi, (_, steps)) in groups.iter().enumerate() {
            for step in steps {
                for file in &step.files {
                    if let Some(&creator_gi) = file_creators.get(file) {
                        if creator_gi != gi && step.description.to_lowercase().contains("update") {
                            deps.entry(groups[gi].0.clone())
                                .or_default()
                                .push(creator_gi);
                        }
                    }
                }
            }
        }

        deps
    }

    /// Estimate complexity for a single child task
    fn child_complexity(&self, steps: &[crate::pipeline::approval::PlanStep]) -> f64 {
        let mut score = 0.1;
        score += (steps.len() as f64) * 0.05;
        let file_count: HashSet<_> = steps.iter().flat_map(|s| &s.files).collect();
        score += (file_count.len() as f64) * 0.03;

        let kw = ["refactor", "restructure", "migrate", "async"];
        for step in steps {
            let lower = step.description.to_lowercase();
            if kw.iter().any(|k| lower.contains(k)) {
                score += 0.05;
            }
        }

        score.clamp(0.0, 1.0)
    }

    fn estimate_duration(&self, steps: &[crate::pipeline::approval::PlanStep]) -> u32 {
        // Rough heuristic: 2 min per step, 30s per file
        (steps.len() as u32 * 2
            + steps
                .iter()
                .map(|s| s.files.len() as u32)
                .sum::<u32>()
                .saturating_div(2)
                .max(1))
        .min(60) // cap at 1 hour
        .max(1)
    }

    fn derive_tags(&self, files: &[String]) -> Vec<String> {
        let mut tags = Vec::new();
        let exts: Vec<&str> = files.iter().filter_map(|f| f.rsplit('.').next()).collect();

        if exts.iter().any(|e| *e == "rs") {
            tags.push("rust".into());
        }
        if exts.iter().any(|e| *e == "ts" || *e == "tsx" || *e == "js") {
            tags.push("typescript".into());
        }
        if exts.iter().any(|e| *e == "py") {
            tags.push("python".into());
        }
        if exts.iter().any(|e| *e == "go") {
            tags.push("go".into());
        }
        if files
            .iter()
            .any(|f| f.contains("test") || f.contains("spec"))
        {
            tags.push("testing".into());
        }
        if files.iter().any(|f| f.contains("migrat")) {
            tags.push("database".into());
        }

        tags
    }
}

fn dir_prefix(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 2 {
        return path.to_string();
    }
    // Return first two directory components
    format!("{}/{}", parts[0], parts[1])
}

#[cfg(test)]
mod tests {
    use super::super::spec_extractor::SpecExtractor;
    use super::*;
    use crate::pipeline::approval::PlanStep;

    fn make_plan() -> ExecutionPlan {
        ExecutionPlan::new("T-1", "Test plan")
            .with_step("Create src/auth/mod.rs", vec!["src/auth/mod.rs".into()])
            .with_step(
                "Add handler in src/auth/handler.rs",
                vec!["src/auth/handler.rs".into()],
            )
            .with_step(
                "Update tests in tests/auth.rs",
                vec!["tests/auth.rs".into()],
            )
    }

    #[test]
    fn test_directory_prefix_grouping() {
        let plan = make_plan();
        let spec = SpecExtractor::extract("Add authentication to the API");
        let decomp = SddDecomposer::new(5).decompose(&plan, &spec);

        assert!(!decomp.children.is_empty());
        assert!(decomp.children.len() <= 5);
    }

    #[test]
    fn test_single_step_no_decomposition() {
        let mut plan = ExecutionPlan::new("T-1", "Simple");
        plan.steps.push(PlanStep {
            step_number: 1,
            description: "Fix one thing".into(),
            files: vec!["src/main.rs".into()],
            parallelizable: false,
        });
        let spec = SpecExtractor::extract("Fix bug in main");
        let decomp = SddDecomposer::new(5).decompose(&plan, &spec);

        assert_eq!(decomp.execution_strategy, ExecutionStrategy::Sequential);
    }
}
