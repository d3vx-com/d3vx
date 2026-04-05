//! Spec extractor — derives a structured TaskSpec from user input
//!
//! Uses deterministic heuristics (no LLM call) so it's fast, testable,
//! and never drifts beyond the user's stated intent.

use super::types::{Scope, TaskSpec};

/// Extracts a structured specification from raw user input
pub struct SpecExtractor;

impl SpecExtractor {
    /// Analyze user input and produce a TaskSpec
    pub fn extract(user_input: &str) -> TaskSpec {
        let goal = extract_goal(user_input);
        let constraints = extract_constraints(user_input);
        let scope = classify_scope(user_input);
        let acceptance_criteria = derive_acceptance_criteria(user_input, scope);
        let complexity = estimate_complexity(user_input, scope, &constraints);
        let benefits_from_decomposition = complexity > 0.5
            || scope == Scope::Architecture
            || (scope == Scope::Refactor && complexity > 0.3);

        TaskSpec {
            goal,
            constraints,
            scope,
            acceptance_criteria,
            estimated_complexity: complexity,
            benefits_from_decomposition,
        }
    }
}

fn extract_goal(input: &str) -> String {
    // Take the first sentence as the goal
    input.split('.').next().unwrap_or(input).trim().to_string()
}

fn extract_constraints(input: &str) -> Vec<String> {
    let mut constraints = Vec::new();
    let lower = input.to_lowercase();

    // Pattern: "without X", "avoid X", "don't use X"
    for keyword in &["without ", "avoid ", "no ", "don't ", "do not ", "never "] {
        for segment in lower.split(keyword).skip(1) {
            if let Some(end) = segment.find(|c: char| c == '.' || c == ',' || c == ';') {
                let fragment = segment[..end].trim().to_string();
                if !fragment.is_empty() && fragment.len() < 120 {
                    constraints.push(format!("{keyword}{fragment}"));
                }
            }
        }
    }

    // Pattern: "using X", "with X", "via X"
    for keyword in &["using ", "with ", "via "] {
        for segment in lower.split(keyword).skip(1) {
            if let Some(end) = segment.find(|c: char| c == '.' || c == ',') {
                let fragment = segment[..end].trim().to_string();
                if !fragment.is_empty() && fragment.len() < 120 {
                    constraints.push(format!("use {fragment}"));
                }
            }
        }
    }

    constraints
}

fn classify_scope(input: &str) -> Scope {
    let lower = input.to_lowercase();
    let has_at_file = input.contains('@');

    let is_create = lower.contains("create")
        || lower.contains("add")
        || lower.contains("implement")
        || lower.contains("write");

    let is_refactor = lower.contains("refactor")
        || lower.contains("restructure")
        || lower.contains("reorganize")
        || lower.contains("migrate");

    let is_arch = lower.contains("design")
        || lower.contains("architecture")
        || lower.contains("system")
        || lower.contains("from scratch");

    let file_mention_count = input.split(|c: char| c == '.' || c == '/').count() - 1;

    if is_arch && (is_create || file_mention_count >= 2) {
        Scope::Architecture
    } else if is_refactor && file_mention_count >= 2 {
        Scope::Refactor
    } else if is_refactor {
        Scope::MultiFile
    } else if is_create && has_at_file {
        Scope::SingleFile
    } else if is_create && file_mention_count >= 2 {
        Scope::MultiFile
    } else if is_create {
        Scope::NewFile
    } else if has_at_file {
        Scope::SingleFile
    } else if file_mention_count >= 2 {
        Scope::MultiFile
    } else {
        Scope::SingleFile
    }
}

fn derive_acceptance_criteria(input: &str, scope: Scope) -> Vec<String> {
    let mut criteria = Vec::new();
    let lower = input.to_lowercase();

    match scope {
        Scope::SingleFile => criteria.push("Single file change compiles cleanly".into()),
        Scope::MultiFile => {
            criteria.push("All referenced files compile".into());
            criteria.push("Integration between changed files works".into());
        }
        Scope::NewFile => criteria.push("New file is syntactically valid and testable".into()),
        Scope::Refactor => {
            criteria.push("Behavior is unchanged after refactor".into());
            criteria.push("Existing tests pass".into());
        }
        Scope::Architecture => {
            criteria.push("Architecture decision is documented".into());
            criteria.push("All integration points are covered".into());
        }
    }

    if lower.contains("test") || lower.contains("spec") {
        criteria.push("Tests are written and passing".into());
    }
    if lower.contains("fix") || lower.contains("bug") {
        criteria.push("Original issue is resolved".into());
    }
    if lower.contains("lint") || lower.contains("clippy") {
        criteria.push("Linter passes with zero warnings".into());
    }

    criteria
}

fn estimate_complexity(input: &str, scope: Scope, constraints: &[String]) -> f64 {
    let mut score = 0.0;

    // Scope base
    match scope {
        Scope::SingleFile => score += 0.15,
        Scope::MultiFile => score += 0.35,
        Scope::NewFile => score += 0.25,
        Scope::Refactor => score += 0.5,
        Scope::Architecture => score += 0.7,
    }

    // Keyword bonuses
    let complex_kw = [
        "refactor",
        "migrate",
        "rewrite",
        "restructure",
        "async",
        "concurrent",
        "database",
        "migration",
        "authentication",
        "authorization",
        "oauth",
        "cache",
        "websocket",
        "real-time",
        "distributed",
    ];
    for kw in complex_kw {
        if input.to_lowercase().contains(kw) {
            score += 0.05;
        }
    }

    // Constraint bonus
    score += (constraints.len() as f64) * 0.03;

    // File mention bonus
    let file_count = input.split("/").filter(|s| s.contains('.')).count();
    score += (file_count as f64) * 0.02;

    score.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_file_scope() {
        let spec = SpecExtractor::extract("Fix the validation bug in @src/auth/mod.rs");
        assert_eq!(spec.scope, Scope::SingleFile);
        assert!(spec.estimated_complexity < 0.3);
    }

    #[test]
    fn test_multi_file_scope() {
        let spec = SpecExtractor::extract(
            "Add rate limiting to src/api/handler.rs and src/api/middleware.rs",
        );
        assert_eq!(spec.scope, Scope::MultiFile);
    }

    #[test]
    fn test_refactor_scope() {
        let spec = SpecExtractor::extract("Refactor the database layer in src/db/");
        assert_eq!(spec.scope, Scope::Refactor);
        assert!(spec.benefits_from_decomposition);
    }

    #[test]
    fn test_constraint_extraction() {
        let spec = SpecExtractor::extract("Implement auth without using sessions, via JWT only");
        assert!(!spec.constraints.is_empty());
        let lower_joined = spec.constraints.join(" ").to_lowercase();
        assert!(lower_joined.contains("session") || lower_joined.contains("jwt"));
    }

    #[test]
    fn test_empty_input() {
        let spec = SpecExtractor::extract("");
        assert!(!spec.goal.is_empty()); // defaults to first line
        assert_eq!(spec.scope, Scope::SingleFile);
    }

    #[test]
    fn test_complexity_architecture() {
        let spec = SpecExtractor::extract(
            "Design a new authentication architecture for the API using OAuth2",
        );
        assert_eq!(spec.scope, Scope::Architecture);
        assert!(spec.estimated_complexity > 0.5);
        assert!(spec.benefits_from_decomposition);
    }
}
