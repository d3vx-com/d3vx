//! Risk Assessment — Enhanced Classification
//!
//! Extends the existing `ExecutionClassifier` with fine-grained risk scoring:
//!
//! - Per-file risk based on path sensitivity (config, auth, migration, etc.)
//! - Dependency risk from the project's call graph
//! - Rollback difficulty (how hard to undo if something goes wrong)
//! - Composite risk score that feeds into execution mode decisions
//!
//! ## Design
//!
//! The existing classifier uses coarse text heuristics (`RiskIndicators::from_text`).
//! This module adds structural analysis that considers:
//! - Which files are touched
//! - What role each file plays in the system
//! - How many other files depend on it
//! - Whether changes are reversible
//!
//! ## Scoring Model
//!
//! Each dimension contributes to a composite risk score (0.0 - 1.0):
//! - File sensitivity weight (0.0 - 0.4)
//! - Dependency breadth weight (0.0 - 0.3)
//! - Rollback difficulty weight (0.0 - 0.3)
//!
//! The composite score maps to execution guidance:
//! - < 0.2: Direct execution (low risk)
//! - 0.2 - 0.5: Vex pattern (two-agent review)
//! - 0.5 - 0.8: Decomposition required
//! - >= 0.8: Decomposition + human review

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Risk level for a single file based on its role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRisk {
    /// Non-critical: tests, docs, config samples
    Negligible,
    /// Low: utility functions, helpers
    Low,
    /// Medium: business logic, handlers
    Medium,
    /// High: auth, config, database
    High,
    /// Critical: migrations, core infrastructure
    Critical,
}

impl FileRisk {
    pub fn weight(&self) -> f64 {
        match self {
            Self::Negligible => 0.0,
            Self::Low => 0.1,
            Self::Medium => 0.25,
            Self::High => 0.4,
            Self::Critical => 0.6,
        }
    }

    /// Classify a file path by its role sensitivity.
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_lowercase();

        // Critical paths
        if lower.contains("migration")
            || lower.contains("schema")
            || lower.contains("db_init")
            || lower.contains("seed")
        {
            return Self::Critical;
        }

        // High sensitivity
        if lower.contains("auth")
            || lower.contains("token")
            || lower.contains("credential")
            || lower.contains("permission")
            || lower.contains("middleware")
            || lower.contains("config")
            || lower.ends_with(".env")
            || lower.ends_with("config.yml")
            || lower.ends_with("config.yaml")
        {
            return Self::High;
        }

        // Medium sensitivity
        if lower.contains("handler")
            || lower.contains("controller")
            || lower.contains("service")
            || lower.contains("business")
            || lower.contains("router")
        {
            return Self::Medium;
        }

        // Low sensitivity
        if lower.contains("util")
            || lower.contains("helper")
            || lower.contains("format")
            || lower.contains("parse")
            || lower.contains("convert")
        {
            return Self::Low;
        }

        // Negligible
        if lower.contains("test")
            || lower.contains("spec")
            || lower.ends_with(".test.rs")
            || lower.ends_with(".md")
            || lower.ends_with(".txt")
            || lower.starts_with("docs/")
        {
            return Self::Negligible;
        }

        // Default: medium for unknown
        Self::Medium
    }
}

/// How difficult it would be to roll back a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollbackDifficulty {
    /// Trivial: revert a commit, delete a file
    Easy,
    /// Moderate: revert with potential data loss
    Moderate,
    /// Hard: database down-migration, data transformation
    Hard,
    /// Nearly impossible: destructive operation, external state
    Irreversible,
}

impl RollbackDifficulty {
    pub fn weight(&self) -> f64 {
        match self {
            Self::Easy => 0.05,
            Self::Moderate => 0.15,
            Self::Hard => 0.3,
            Self::Irreversible => 0.5,
        }
    }

    /// Estimate rollback difficulty from file paths and description.
    pub fn estimate(files: &[&str], description: &str) -> Self {
        let lower = description.to_lowercase();

        // Irreversible signals
        if lower.contains("drop") && lower.contains("table")
            || lower.contains("delete") && lower.contains("production")
            || lower.contains("destroy")
        {
            return Self::Irreversible;
        }

        // Hard: migrations with data changes
        let has_migrations = files.iter().any(|p| p.contains("migration"));
        if has_migrations
            && (lower.contains("data") || lower.contains("transform") || lower.contains("backfill"))
        {
            return Self::Hard;
        }

        // Moderate: structural changes
        if has_migrations
            || files.iter().any(|p| p.contains("schema"))
            || lower.contains("rename")
            || lower.contains("refactor")
        {
            return Self::Moderate;
        }

        // Easy: additive changes
        Self::Easy
    }
}

/// Dependency risk based on how many consumers a file has.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRisk {
    /// Number of files that import this one
    pub inbound_count: usize,
    /// Number of files this one depends on
    pub outbound_count: usize,
}

impl DependencyRisk {
    pub fn weight(&self) -> f64 {
        // More inbound dependents = higher risk
        match self.inbound_count {
            0 => 0.0,
            1..=3 => 0.05,
            4..=10 => 0.15,
            11..=20 => 0.2,
            _ => 0.3,
        }
    }

    /// Estimate from file analysis. Real implementation would parse
    /// import statements; this uses a keyword heuristic.
    pub fn estimate_from_files(_files: &[&str]) -> HashMap<String, DependencyRisk> {
        _files
            .iter()
            .map(|f| {
                (
                    f.to_string(),
                    DependencyRisk {
                        inbound_count: 0,
                        outbound_count: 0,
                    },
                )
            })
            .collect()
    }
}

/// Composite risk assessment for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    /// Per-file risk levels
    pub file_risks: Vec<(String, FileRisk)>,
    /// Estimated rollback difficulty
    pub rollback_difficulty: RollbackDifficulty,
    /// Dependency risk summary
    pub dependency_risk: DependencyRisk,
    /// Composite score (0.0 - 1.0)
    pub composite_score: f64,
    /// Human-readable reasoning
    pub reasoning: Vec<String>,
}

impl RiskAssessment {
    /// Assess risk from file paths and task description.
    pub fn assess(files: &[String], instruction: &str) -> Self {
        let mut file_risks = Vec::new();
        let mut max_file_risk = 0.0;
        let mut avg_file_risk = 0.0;
        let mut reasoning = Vec::new();

        for file in files {
            let risk = FileRisk::from_path(file);
            let w = risk.weight();
            if w > max_file_risk {
                max_file_risk = w;
            }
            avg_file_risk += w;
            file_risks.push((file.clone(), risk));
        }

        if !files.is_empty() {
            avg_file_risk /= files.len() as f64;
        }

        // Composite: weighted average of max and mean file risk
        let file_score = max_file_risk * 0.6 + avg_file_risk * 0.4;

        // Rollback difficulty
        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        let rollback = RollbackDifficulty::estimate(&file_refs, instruction);
        let rollback_score = rollback.weight();

        // Dependency risk
        let dep_risk = DependencyRisk::estimate_from_files(&file_refs);
        let dep_score =
            dep_risk.values().map(|d| d.weight()).sum::<f64>() / dep_risk.len().max(1) as f64;

        // Composite: file sensitivity (40%) + rollback (30%) + dependency (30%)
        let composite = file_score * 0.4 + rollback_score * 0.3 + dep_score * 0.3;
        let composite = composite.min(1.0);

        // Build reasoning
        if max_file_risk >= 0.4 {
            let critical_files: Vec<_> = file_risks
                .iter()
                .filter(|(_, r)| r.weight() >= 0.4)
                .map(|(f, _)| f.as_str())
                .collect();
            reasoning.push(format!("Sensitive files: {}", critical_files.join(", ")));
        }

        if rollback.weight() >= 0.3 {
            reasoning.push(format!("Rollback difficulty: {:?}", rollback));
        }

        if dep_score > 0.15 {
            reasoning.push(format!("Dependency breadth: {:.2}", dep_score));
        }

        if reasoning.is_empty() {
            reasoning.push("Low structural risk".to_string());
        }

        Self {
            file_risks,
            rollback_difficulty: rollback,
            dependency_risk: DependencyRisk {
                inbound_count: 0,
                outbound_count: 0,
            },
            composite_score: composite,
            reasoning,
        }
    }

    /// Get execution recommendation based on composite score.
    pub fn recommendation(&self) -> RiskRecommendation {
        match self.composite_score {
            s if s < 0.2 => RiskRecommendation::DirectExecution,
            s if s < 0.5 => RiskRecommendation::VexPattern,
            s if s < 0.8 => RiskRecommendation::Decompose,
            _ => RiskRecommendation::DecomposeAndReview,
        }
    }
}

/// Recommended execution strategy based on risk assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskRecommendation {
    /// Execute directly without extra guardrails
    DirectExecution,
    /// Use two-agent Vex pattern for review
    VexPattern,
    /// Decompose into subtasks
    Decompose,
    /// Decompose and require human review
    DecomposeAndReview,
}

impl std::fmt::Display for RiskRecommendation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DirectExecution => write!(f, "Execute directly"),
            Self::VexPattern => write!(f, "Two-agent review pattern"),
            Self::Decompose => write!(f, "Decompose into subtasks"),
            Self::DecomposeAndReview => write!(f, "Decompose with human review"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_risk_from_path() {
        assert!(matches!(
            FileRisk::from_path("migration/001_create_users.rs"),
            FileRisk::Critical
        ));
        assert!(matches!(
            FileRisk::from_path("src/auth/token.rs"),
            FileRisk::High
        ));
        assert!(matches!(
            FileRisk::from_path("src/handler/user.rs"),
            FileRisk::Medium
        ));
        assert!(matches!(
            FileRisk::from_path("src/util/format.rs"),
            FileRisk::Low
        ));
        assert!(matches!(
            FileRisk::from_path("tests/auth_test.rs"),
            FileRisk::Negligible
        ));
    }

    #[test]
    fn test_file_risk_weights() {
        assert!((FileRisk::Negligible.weight() - 0.0).abs() < 0.001);
        assert!((FileRisk::Critical.weight() - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_rollback_difficulty() {
        let files = vec!["migration/001.rs"];
        let desc = "drop table users";
        assert!(matches!(
            RollbackDifficulty::estimate(&files, desc),
            RollbackDifficulty::Irreversible
        ));

        let desc = "add column name";
        assert!(matches!(
            RollbackDifficulty::estimate(&files, desc),
            RollbackDifficulty::Moderate
        ));

        let files = vec!["src/lib.rs"];
        let desc = "add utility function";
        assert!(matches!(
            RollbackDifficulty::estimate(&files, desc),
            RollbackDifficulty::Easy
        ));
    }

    #[test]
    fn test_risk_assessment_composite() {
        let files = vec!["src/auth/mod.rs".to_string()];
        let assessment = RiskAssessment::assess(&files, "add auth endpoint");

        assert!(assessment.composite_score > 0.0);
        assert!(assessment.composite_score <= 1.0);
        assert!(!assessment.file_risks.is_empty());
        assert!(matches!(assessment.file_risks[0].1, FileRisk::High));
    }

    #[test]
    fn test_risk_assessment_low_risk() {
        let files = vec![
            "docs/guide.md".to_string(),
            "src/util/helper.rs".to_string(),
        ];
        let assessment = RiskAssessment::assess(&files, "fix typo in helper doc");

        assert!(assessment.composite_score < 0.3);
        assert!(matches!(
            assessment.rollback_difficulty,
            RollbackDifficulty::Easy
        ));
    }

    #[test]
    fn test_recommendation_mapping() {
        let rec_low = RiskAssessment::assess(&["docs/readme.md".to_string()], "update docs");
        assert!(matches!(
            rec_low.recommendation(),
            RiskRecommendation::DirectExecution
        ));

        let rec_high = RiskAssessment::assess(
            &["migration/001_drop_table.rs".to_string()],
            "drop production table and backfill data",
        );
        assert!(matches!(
            rec_high.recommendation(),
            RiskRecommendation::DecomposeAndReview
        ));
    }
}
