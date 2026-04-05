//! Execution Classifier
//!
//! Analyzes task complexity and determines the appropriate execution mode.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use super::phases::{Phase, Task};

/// Execution mode determined by the classifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Tiny task - execute directly without creating a formal task record
    /// (e.g., simple queries, status checks)
    TinyDirect,
    /// Small task - single agent execution, no decomposition needed
    SmallDirect,
    /// Medium task with risk - use Vex pattern (two agents for review)
    MediumVex,
    /// Large task - decompose into subtasks
    LargeDecompose,
    /// Complex task requiring multi-phase planning
    ComplexMultiPhase,
}

impl ExecutionMode {
    /// Check if this mode requires task decomposition
    pub fn requires_decomposition(&self) -> bool {
        matches!(
            self,
            ExecutionMode::LargeDecompose | ExecutionMode::ComplexMultiPhase
        )
    }

    /// Check if this mode uses Vex pattern (two-agent review)
    pub fn uses_vex_pattern(&self) -> bool {
        matches!(self, ExecutionMode::MediumVex)
    }

    /// Check if this mode should create a formal task record
    pub fn requires_task_record(&self) -> bool {
        !matches!(self, ExecutionMode::TinyDirect)
    }

    /// Get recommended initial phase for this mode
    pub fn recommended_initial_phase(&self) -> Phase {
        match self {
            ExecutionMode::TinyDirect => Phase::Implement,
            ExecutionMode::SmallDirect => Phase::Research,
            ExecutionMode::MediumVex => Phase::Research,
            ExecutionMode::LargeDecompose => Phase::Plan,
            ExecutionMode::ComplexMultiPhase => Phase::Research,
        }
    }
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::TinyDirect => write!(f, "Tiny (Direct)"),
            ExecutionMode::SmallDirect => write!(f, "Small (Direct)"),
            ExecutionMode::MediumVex => write!(f, "Medium (Vex)"),
            ExecutionMode::LargeDecompose => write!(f, "Large (Decompose)"),
            ExecutionMode::ComplexMultiPhase => write!(f, "Complex (Multi-Phase)"),
        }
    }
}

/// Risk indicators that affect classification
#[derive(Debug, Clone, Default)]
pub struct RiskIndicators {
    /// Involves production changes
    pub production_impact: bool,
    /// Involves database schema changes
    pub schema_changes: bool,
    /// Involves authentication/security
    pub security_related: bool,
    /// Involves external API calls
    pub external_dependencies: bool,
    /// Involves file system operations
    pub file_operations: bool,
    /// Involves git operations
    pub git_operations: bool,
    /// Has time constraints
    pub time_constraints: bool,
    /// Affects multiple components
    pub cross_cutting: bool,
}

impl RiskIndicators {
    /// Calculate overall risk score (0-10)
    pub fn risk_score(&self) -> u8 {
        let mut score = 0u8;
        if self.production_impact {
            score += 2;
        }
        if self.schema_changes {
            score += 2;
        }
        if self.security_related {
            score += 3;
        }
        if self.external_dependencies {
            score += 1;
        }
        if self.file_operations {
            score += 1;
        }
        if self.git_operations {
            score += 1;
        }
        if self.time_constraints {
            score += 1;
        }
        if self.cross_cutting {
            score += 2;
        }
        score.min(10)
    }

    /// Check if task is high risk
    pub fn is_high_risk(&self) -> bool {
        self.risk_score() >= 5
    }

    /// Analyze text for risk indicators
    pub fn from_text(text: &str) -> Self {
        let lower = text.to_lowercase();

        Self {
            production_impact: lower.contains("production")
                || lower.contains("deploy")
                || lower.contains("release")
                || lower.contains("live"),
            schema_changes: lower.contains("migration")
                || lower.contains("schema")
                || lower.contains("database change")
                || lower.contains("alter table"),
            security_related: lower.contains("auth")
                || lower.contains("security")
                || lower.contains("password")
                || lower.contains("token")
                || lower.contains("credential")
                || lower.contains("permission"),
            external_dependencies: lower.contains("api")
                || lower.contains("external")
                || lower.contains("third-party")
                || lower.contains("integration"),
            file_operations: lower.contains("file")
                || lower.contains("write")
                || lower.contains("delete")
                || lower.contains("create"),
            git_operations: lower.contains("git")
                || lower.contains("branch")
                || lower.contains("merge")
                || lower.contains("commit")
                || lower.contains("push"),
            time_constraints: lower.contains("urgent")
                || lower.contains("asap")
                || lower.contains("deadline")
                || lower.contains("blocker"),
            cross_cutting: lower.contains("refactor")
                || lower.contains("multiple")
                || lower.contains("across")
                || lower.contains("system-wide"),
        }
    }
}

/// Complexity metrics for classification
#[derive(Debug, Clone)]
pub struct ComplexityMetrics {
    /// Estimated lines of code to change
    pub estimated_loc: usize,
    /// Number of files likely affected
    pub files_affected: usize,
    /// Number of distinct operations needed
    pub operations_count: usize,
    /// Conceptual complexity (1-10)
    pub conceptual_complexity: u8,
    /// Whether tests are needed
    pub requires_tests: bool,
    /// Whether documentation is needed
    pub requires_docs: bool,
}

impl ComplexityMetrics {
    /// Calculate overall complexity score
    pub fn complexity_score(&self) -> u8 {
        let mut score = 0u8;

        // Lines of code contribution
        if self.estimated_loc > 500 {
            score += 3;
        } else if self.estimated_loc > 200 {
            score += 2;
        } else if self.estimated_loc > 50 {
            score += 1;
        }

        // Files affected contribution
        if self.files_affected > 10 {
            score += 2;
        } else if self.files_affected > 5 {
            score += 1;
        }

        // Operations contribution
        if self.operations_count > 5 {
            score += 2;
        } else if self.operations_count > 2 {
            score += 1;
        }

        // Conceptual complexity
        score += self.conceptual_complexity.min(3);

        // Tests and docs
        if self.requires_tests {
            score += 1;
        }
        if self.requires_docs {
            score += 1;
        }

        score.min(10)
    }

    /// Estimate metrics from task instruction
    pub fn from_text(text: &str) -> Self {
        let lower = text.to_lowercase();
        let word_count = text.split_whitespace().count();

        // Estimate LOC based on word count (rough heuristic)
        let estimated_loc = word_count.saturating_mul(2);

        // Count file references
        let files_affected = text
            .matches(".rs")
            .chain(text.matches(".ts"))
            .chain(text.matches(".js"))
            .chain(text.matches("file"))
            .count()
            .max(1);

        // Count operations (look for action verbs)
        let operations_count = [
            "add",
            "create",
            "update",
            "delete",
            "modify",
            "refactor",
            "implement",
            "fix",
            "change",
            "remove",
        ]
        .iter()
        .filter(|verb| lower.contains(*verb))
        .count()
        .max(1);

        // Conceptual complexity based on keywords
        let conceptual_complexity = if lower.contains("algorithm")
            || lower.contains("architecture")
            || lower.contains("design")
            || lower.contains("pattern")
        {
            3
        } else if lower.contains("logic") || lower.contains("business") {
            2
        } else {
            1
        };

        // Check for test requirements
        let requires_tests = !lower.contains("skip test")
            && (lower.contains("test") || lower.contains("spec") || estimated_loc > 100);

        // Check for docs requirements
        let requires_docs =
            lower.contains("document") || lower.contains("readme") || lower.contains("api doc");

        Self {
            estimated_loc,
            files_affected,
            operations_count,
            conceptual_complexity,
            requires_tests,
            requires_docs,
        }
    }
}

impl Default for ComplexityMetrics {
    fn default() -> Self {
        Self {
            estimated_loc: 10,
            files_affected: 1,
            operations_count: 1,
            conceptual_complexity: 1,
            requires_tests: false,
            requires_docs: false,
        }
    }
}

/// Configuration for the classifier
#[derive(Debug, Clone)]
pub struct ClassifierConfig {
    /// Complexity threshold for small vs medium (0-10 scale)
    pub small_medium_threshold: u8,
    /// Complexity threshold for medium vs large
    pub medium_large_threshold: u8,
    /// Risk threshold for requiring Vex pattern
    pub vex_risk_threshold: u8,
    /// Enable automatic complexity estimation
    pub auto_estimate: bool,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            small_medium_threshold: 3,
            medium_large_threshold: 6,
            vex_risk_threshold: 5,
            auto_estimate: true,
        }
    }
}

/// Classifier that determines execution mode for tasks
pub struct ExecutionClassifier {
    config: ClassifierConfig,
}

impl ExecutionClassifier {
    /// Create a new classifier with default configuration
    pub fn new() -> Self {
        Self {
            config: ClassifierConfig::default(),
        }
    }

    /// Create a classifier with custom configuration
    pub fn with_config(config: ClassifierConfig) -> Self {
        Self { config }
    }

    /// Classify a task and determine its execution mode
    pub fn classify(&self, task: &Task) -> Result<ClassificationResult> {
        info!("Classifying task: {}", task.id);

        // Gather metrics
        let complexity = if self.config.auto_estimate {
            ComplexityMetrics::from_text(&task.instruction)
        } else {
            ComplexityMetrics::default()
        };

        let risk = RiskIndicators::from_text(&format!("{} {}", task.title, task.instruction));

        // Determine execution mode
        let mode = self.determine_mode(&complexity, &risk);

        debug!(
            "Task {} classified as {} (complexity: {}, risk: {})",
            task.id,
            mode,
            complexity.complexity_score(),
            risk.risk_score()
        );

        Ok(ClassificationResult {
            mode,
            complexity_score: complexity.complexity_score(),
            risk_score: risk.risk_score(),
            reasoning: self.generate_reasoning(&mode, &complexity, &risk),
        })
    }

    /// Determine execution mode from metrics
    fn determine_mode(
        &self,
        complexity: &ComplexityMetrics,
        risk: &RiskIndicators,
    ) -> ExecutionMode {
        let complexity_score = complexity.complexity_score();
        let risk_score = risk.risk_score();

        // Tiny tasks: very low complexity, no risk
        if complexity_score <= 1 && risk_score <= 1 {
            return ExecutionMode::TinyDirect;
        }

        // Large or complex tasks
        if complexity_score >= self.config.medium_large_threshold {
            // If cross-cutting or high complexity, go multi-phase
            if risk.cross_cutting || complexity.conceptual_complexity >= 3 {
                return ExecutionMode::ComplexMultiPhase;
            }
            return ExecutionMode::LargeDecompose;
        }

        // Medium tasks with high risk
        if complexity_score >= self.config.small_medium_threshold {
            if risk_score >= self.config.vex_risk_threshold {
                return ExecutionMode::MediumVex;
            }
            return ExecutionMode::SmallDirect;
        }

        // Small tasks
        ExecutionMode::SmallDirect
    }

    /// Generate human-readable reasoning for the classification
    fn generate_reasoning(
        &self,
        mode: &ExecutionMode,
        complexity: &ComplexityMetrics,
        risk: &RiskIndicators,
    ) -> String {
        let mut reasons = Vec::new();

        // Complexity factors
        if complexity.estimated_loc > 200 {
            reasons.push(format!("~{} LOC", complexity.estimated_loc));
        }
        if complexity.files_affected > 3 {
            reasons.push(format!("{} files", complexity.files_affected));
        }
        if complexity.conceptual_complexity >= 2 {
            reasons.push("high conceptual complexity".to_string());
        }

        // Risk factors
        if risk.production_impact {
            reasons.push("production impact".to_string());
        }
        if risk.security_related {
            reasons.push("security-related".to_string());
        }
        if risk.schema_changes {
            reasons.push("schema changes".to_string());
        }

        if reasons.is_empty() {
            format!("Classified as {} (simple task)", mode)
        } else {
            format!("Classified as {} due to: {}", mode, reasons.join(", "))
        }
    }

    /// Quick classification without full analysis
    pub fn quick_classify(instruction: &str) -> ExecutionMode {
        let complexity = ComplexityMetrics::from_text(instruction);
        let risk = RiskIndicators::from_text(instruction);

        let complexity_score = complexity.complexity_score();
        let risk_score = risk.risk_score();

        if complexity_score <= 1 && risk_score <= 1 {
            ExecutionMode::TinyDirect
        } else if complexity_score >= 6 {
            if risk.cross_cutting {
                ExecutionMode::ComplexMultiPhase
            } else {
                ExecutionMode::LargeDecompose
            }
        } else if complexity_score >= 3 && risk_score >= 5 {
            ExecutionMode::MediumVex
        } else {
            ExecutionMode::SmallDirect
        }
    }
}

impl Default for ExecutionClassifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of task classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// Determined execution mode
    pub mode: ExecutionMode,
    /// Complexity score (0-10)
    pub complexity_score: u8,
    /// Risk score (0-10)
    pub risk_score: u8,
    /// Human-readable reasoning
    pub reasoning: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_task(instruction: &str) -> Task {
        Task::new("TEST-001", "Test Task", instruction)
    }

    #[test]
    fn test_execution_mode_properties() {
        assert!(!ExecutionMode::TinyDirect.requires_task_record());
        assert!(ExecutionMode::SmallDirect.requires_task_record());
        assert!(ExecutionMode::MediumVex.uses_vex_pattern());
        assert!(ExecutionMode::LargeDecompose.requires_decomposition());
    }

    #[test]
    fn test_risk_indicators() {
        let indicators =
            RiskIndicators::from_text("This is a production security fix for authentication");
        assert!(indicators.production_impact);
        assert!(indicators.security_related);
        assert!(indicators.is_high_risk());
    }

    #[test]
    fn test_complexity_metrics() {
        let metrics =
            ComplexityMetrics::from_text("Implement a new algorithm that refactors multiple files");
        assert!(metrics.conceptual_complexity >= 2);
    }

    #[test]
    fn test_classify_tiny_task() {
        let classifier = ExecutionClassifier::new();
        let task = create_test_task("What is the status of the project?");

        let result = classifier.classify(&task).unwrap();
        assert_eq!(result.mode, ExecutionMode::TinyDirect);
    }

    #[test]
    fn test_classify_small_task() {
        let classifier = ExecutionClassifier::new();
        let task = create_test_task("Add a simple utility function");

        let result = classifier.classify(&task).unwrap();
        assert!(matches!(
            result.mode,
            ExecutionMode::SmallDirect | ExecutionMode::TinyDirect
        ));
    }

    #[test]
    fn test_classify_medium_vex() {
        let classifier = ExecutionClassifier::new();
        let task = create_test_task(
            "Update the production authentication system to support new security tokens. \
            This requires a full architectural redesign of the core identity provider logic.",
        );

        let result = classifier.classify(&task).unwrap();
        // Should be at least MediumVex due to production + security
        assert!(matches!(
            result.mode,
            ExecutionMode::MediumVex
                | ExecutionMode::LargeDecompose
                | ExecutionMode::ComplexMultiPhase
        ));
    }

    #[test]
    fn test_classify_large_task() {
        let classifier = ExecutionClassifier::new();
        // Long instruction with many operations
        let instruction = "Refactor the entire codebase to implement a new architecture pattern. \
            Add, update, delete, modify multiple files. Create new tests and documentation. \
            This affects multiple components across the system.";
        let task = create_test_task(instruction);

        let result = classifier.classify(&task).unwrap();
        assert!(result.mode.requires_decomposition());
    }

    #[test]
    fn test_quick_classify() {
        let mode = ExecutionClassifier::quick_classify("Fix a typo");
        assert!(matches!(
            mode,
            ExecutionMode::TinyDirect | ExecutionMode::SmallDirect
        ));
    }
}
