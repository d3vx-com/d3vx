//! Planner agent specialization
//!
//! Provides the planner role that generates structured execution plans
//! from task descriptions. The plan is then submitted through the
//! approval flow before being handed off to executor agents.

use tracing::{debug, info};

use super::flow::ApprovalFlow;
use super::types::*;
use crate::pipeline::phases::{PhaseContext, Task};

/// Planner agent that produces structured execution plans
pub struct Planner {
    /// Approval flow for plan gating
    approval: ApprovalFlow,
}

impl Planner {
    /// Create a new planner with the given approval configuration
    pub fn new(config: ApprovalConfig) -> Self {
        Self {
            approval: ApprovalFlow::new(config),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(ApprovalConfig::default())
    }

    /// Create a planner that doesn't require approval (auto-approve everything)
    pub fn no_approval() -> Self {
        Self::new(ApprovalConfig {
            require_approval: false,
            ..Default::default()
        })
    }

    /// Access the underlying approval flow
    pub fn approval_flow(&self) -> &ApprovalFlow {
        &self.approval
    }

    /// Build a structured plan from phase context and agent output
    ///
    /// This parses the free-form agent output from the Plan phase into
    /// a structured `ExecutionPlan` that can be submitted for approval.
    pub fn build_plan(
        &self,
        task: &Task,
        _context: &PhaseContext,
        agent_output: &str,
    ) -> ExecutionPlan {
        let mut plan = ExecutionPlan::new(&task.id, extract_summary(agent_output));

        // Extract steps from the agent output
        let steps = extract_steps(agent_output);
        for step in steps {
            plan = plan.with_step(step.description, step.files);
        }

        // Detect files mentioned in the output
        let (modify, create) = extract_file_mentions(agent_output);
        plan.files_to_modify = modify;
        plan.files_to_create = create;

        // Estimate risk and complexity from the plan
        plan.risk_level = estimate_risk(&plan);
        plan.complexity = estimate_complexity(&plan);

        debug!(
            plan_id = %plan.id,
            steps = plan.steps.len(),
            risk = %plan.risk_level,
            complexity = plan.complexity,
            "Built execution plan"
        );

        plan
    }

    /// Submit a plan through the approval flow and wait for a decision
    pub async fn submit_for_approval(
        &self,
        plan: ExecutionPlan,
    ) -> Result<ApprovalState, ApprovalError> {
        info!(plan_id = %plan.id, "Submitting plan for approval");
        self.approval.submit(plan).await
    }

    /// Submit without blocking
    pub async fn submit_async(
        &self,
        plan: ExecutionPlan,
    ) -> Result<super::flow::SubmitResult, ApprovalError> {
        self.approval.submit_async(plan).await
    }

    /// Record a user decision
    pub async fn record_decision(&self, decision: ApprovalDecision) -> Result<(), ApprovalError> {
        self.approval.decide(decision).await
    }
}

/// Extract a summary line from agent output
fn extract_summary(output: &str) -> String {
    output
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("Implementation plan")
        .trim()
        .to_string()
}

/// A raw step extracted from text
struct RawStep {
    description: String,
    files: Vec<String>,
}

/// Extract numbered steps from agent output
fn extract_steps(output: &str) -> Vec<RawStep> {
    let mut steps = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        // Match patterns like "1. Step description" or "- Step description"
        let step_text = if let Some(rest) = trimmed.strip_prefix(|c: char| c.is_ascii_digit()) {
            let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
            rest.strip_prefix('.')
                .or_else(|| rest.strip_prefix(')'))
                .map(|s| s.trim())
        } else if let Some(rest) = trimmed.strip_prefix("- ") {
            Some(rest)
        } else {
            None
        };

        if let Some(text) = step_text {
            if !text.is_empty() {
                let files = extract_file_paths(text);
                steps.push(RawStep {
                    description: text.to_string(),
                    files,
                });
            }
        }
    }

    steps
}

/// Extract file paths from text (simple heuristic)
fn extract_file_paths(text: &str) -> Vec<String> {
    let mut files = Vec::new();
    for word in text.split_whitespace() {
        // Look for paths with extensions or known prefixes
        if word.contains('/') && word.contains('.') {
            let clean = word.trim_matches(|c: char| c == '`' || c == '\'' || c == '"' || c == ',');
            files.push(clean.to_string());
        } else if word.starts_with("src/") || word.starts_with("lib/") || word.starts_with("test") {
            let clean = word.trim_matches(|c: char| c == '`' || c == '\'' || c == '"' || c == ',');
            files.push(clean.to_string());
        }
    }
    files
}

/// Extract file modification/creation mentions
fn extract_file_mentions(output: &str) -> (Vec<String>, Vec<String>) {
    let mut modify = Vec::new();
    let mut create = Vec::new();

    for line in output.lines() {
        let lower = line.to_lowercase();
        let is_create =
            lower.contains("create") || lower.contains("new file") || lower.contains("add file");
        let is_modify = lower.contains("modify")
            || lower.contains("edit")
            || lower.contains("update")
            || lower.contains("change");

        for file in extract_file_paths(line) {
            if is_create && !is_modify {
                if !create.contains(&file) {
                    create.push(file);
                }
            } else if is_modify {
                if !modify.contains(&file) {
                    modify.push(file);
                }
            }
        }
    }

    (modify, create)
}

/// Estimate risk level from plan characteristics
fn estimate_risk(plan: &ExecutionPlan) -> RiskLevel {
    let file_count = plan.files_to_modify.len() + plan.files_to_create.len();
    let step_count = plan.steps.len();

    if file_count > 10 || step_count > 8 {
        RiskLevel::High
    } else if file_count > 4 || step_count > 4 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
}

/// Estimate complexity score (0.0 - 1.0) from plan characteristics
fn estimate_complexity(plan: &ExecutionPlan) -> f64 {
    let mut score = 0.0;

    // Factor in number of steps
    score += (plan.steps.len() as f64 * 0.08).min(0.4);

    // Factor in number of files
    let file_count = plan.files_to_modify.len() + plan.files_to_create.len();
    score += (file_count as f64 * 0.05).min(0.3);

    // Factor in step descriptions containing complex keywords
    let complex_keywords = ["refactor", "restructure", "migrate", "rewrite", "architect"];
    for step in &plan.steps {
        let lower = step.description.to_lowercase();
        if complex_keywords.iter().any(|k| lower.contains(k)) {
            score += 0.1;
        }
    }

    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::phases::task::PhaseContext;

    fn make_task() -> Task {
        Task::new("TASK-001", "Test task", "Implement feature X")
    }

    #[test]
    fn test_extract_summary() {
        let output = "This plan implements feature X\n\nStep 1: Do thing";
        assert_eq!(extract_summary(output), "This plan implements feature X");
    }

    #[test]
    fn test_extract_steps_numbered() {
        let output = "Plan:\n1. Create src/foo.rs\n2. Update src/bar.rs\n3. Add tests";
        let steps = extract_steps(output);
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].description, "Create src/foo.rs");
        assert!(steps[0].files.contains(&"src/foo.rs".to_string()));
    }

    #[test]
    fn test_extract_steps_dashed() {
        let output = "- Step one\n- Step two\n- Step three";
        let steps = extract_steps(output);
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn test_estimate_risk() {
        let mut plan = ExecutionPlan::new("T-1", "test");
        assert_eq!(estimate_risk(&plan), RiskLevel::Low);

        for i in 0..6 {
            plan.files_to_modify.push(format!("file{}.rs", i));
        }
        assert_eq!(estimate_risk(&plan), RiskLevel::Medium);

        for i in 0..10 {
            plan.files_to_modify.push(format!("more{}.rs", i));
        }
        assert_eq!(estimate_risk(&plan), RiskLevel::High);
    }

    #[test]
    fn test_estimate_complexity() {
        let mut plan = ExecutionPlan::new("T-1", "test");
        let c = estimate_complexity(&plan);
        assert!(c < 0.1);

        plan = plan.with_step("Refactor the entire module", vec![]);
        plan = plan.with_step("Migrate data layer", vec![]);
        let c2 = estimate_complexity(&plan);
        assert!(c2 > c, "complex keywords should increase score");
    }

    #[test]
    fn test_build_plan() {
        let task = make_task();
        let context = PhaseContext::new(task.clone(), "/tmp", "/tmp/work");
        let planner = Planner::no_approval();

        let output = "Implement feature X\n\n1. Create src/new_module.rs\n2. Update src/main.rs\n3. Add tests in src/new_module.rs";
        let plan = planner.build_plan(&task, &context, output);

        assert_eq!(plan.steps.len(), 3);
        assert!(!plan.summary.is_empty());
    }

    #[tokio::test]
    async fn test_auto_approval() {
        let planner = Planner::new(ApprovalConfig::default());
        let plan = ExecutionPlan::new("T-1", "Simple task")
            .with_risk(RiskLevel::Low)
            .with_complexity(0.1);

        let state = planner.submit_for_approval(plan).await.unwrap();
        assert_eq!(state, ApprovalState::Approved);
    }

    #[tokio::test]
    async fn test_no_approval_mode() {
        let planner = Planner::no_approval();
        let plan = ExecutionPlan::new("T-1", "Any task").with_risk(RiskLevel::High);
        let state = planner.submit_for_approval(plan).await.unwrap();
        assert_eq!(state, ApprovalState::Approved);
    }
}
