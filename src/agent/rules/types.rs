//! Core types for per-project agent rules.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Loaded project rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRules {
    /// Project description from rules file
    pub description: Option<String>,
    /// Architecture constraints
    pub constraints: Vec<String>,
    /// Preferred patterns and conventions
    pub conventions: Vec<String>,
    /// Files/directories the agent should NOT modify
    pub protected_paths: Vec<String>,
    /// Additional system prompt additions
    pub system_prompt_additions: Vec<String>,
    /// Per-role rules keyed by role name (e.g. "backend", "frontend")
    pub role_rules: HashMap<String, Vec<String>>,
    /// Architecture doc content (if found)
    pub architecture_doc: Option<String>,
}

/// A single rule entry from the rules YAML file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RulesFile {
    pub(crate) description: Option<String>,
    pub(crate) constraints: Option<Vec<String>>,
    pub(crate) conventions: Option<Vec<String>>,
    pub(crate) protected_paths: Option<Vec<String>>,
    pub(crate) system_prompt: Option<Vec<String>>,
    pub(crate) roles: Option<HashMap<String, Vec<String>>>,
}

impl Default for ProjectRules {
    fn default() -> Self {
        Self {
            description: None,
            constraints: Vec::new(),
            conventions: Vec::new(),
            protected_paths: Vec::new(),
            system_prompt_additions: Vec::new(),
            role_rules: HashMap::new(),
            architecture_doc: None,
        }
    }
}

impl ProjectRules {
    /// Merge overlay rules on top of base rules. Overlay values take precedence.
    pub fn merge(base: Self, overlay: Self) -> Self {
        Self {
            description: overlay.description.or(base.description),
            constraints: merge_vec(base.constraints, overlay.constraints),
            conventions: merge_vec(base.conventions, overlay.conventions),
            protected_paths: merge_vec(base.protected_paths, overlay.protected_paths),
            system_prompt_additions: merge_vec(
                base.system_prompt_additions,
                overlay.system_prompt_additions,
            ),
            role_rules: merge_map(base.role_rules, overlay.role_rules),
            architecture_doc: overlay.architecture_doc.or(base.architecture_doc),
        }
    }

    /// Format rules as a system prompt section suitable for injection.
    pub fn to_prompt_section(&self) -> String {
        let mut sections = Vec::new();

        sections.push("# Project Rules".to_string());

        if let Some(ref desc) = self.description {
            sections.push(format!("## Description\n{}", desc));
        }

        if !self.constraints.is_empty() {
            let items = self
                .constraints
                .iter()
                .map(|c| format!("- {}", c))
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("## Constraints\n{}", items));
        }

        if !self.conventions.is_empty() {
            let items = self
                .conventions
                .iter()
                .map(|c| format!("- {}", c))
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("## Conventions\n{}", items));
        }

        if !self.protected_paths.is_empty() {
            let items = self
                .protected_paths
                .iter()
                .map(|p| format!("- {}", p))
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("## Protected Paths\n{}", items));
        }

        if let Some(ref arch_doc) = self.architecture_doc {
            sections.push(format!("## Architecture Notes\n{}", arch_doc));
        }

        for (prompt_addition_idx, addition) in self.system_prompt_additions.iter().enumerate() {
            sections.push(format!(
                "## Additional Context {}\n{}",
                prompt_addition_idx + 1,
                addition
            ));
        }

        sections.join("\n\n")
    }

    /// Get rules for a specific agent role.
    pub fn rules_for_role(&self, role: &str) -> Vec<String> {
        let normalized = role.to_lowercase();
        self.role_rules
            .get(&normalized)
            .cloned()
            .unwrap_or_default()
    }

    /// Format role-specific rules as a prompt section.
    pub fn role_prompt_section(&self, role: &str) -> Option<String> {
        let rules = self.rules_for_role(role);
        if rules.is_empty() {
            return None;
        }
        let items = rules
            .iter()
            .map(|r| format!("- {}", r))
            .collect::<Vec<_>>()
            .join("\n");
        Some(format!("## Role-specific: {}\n{}", role, items))
    }

    /// Check if a path matches any protected path pattern.
    pub fn is_protected(&self, path: &str) -> bool {
        self.protected_paths.iter().any(|pattern| {
            match glob::Pattern::new(pattern) {
                Ok(glob_pattern) => glob_pattern.matches(path),
                Err(_) => {
                    // Fall back to simple prefix/contains match for invalid patterns
                    path.starts_with(pattern) || path.contains(pattern)
                }
            }
        })
    }

    /// Returns true if there are any rules defined.
    pub fn has_rules(&self) -> bool {
        self.description.is_some()
            || !self.constraints.is_empty()
            || !self.conventions.is_empty()
            || !self.protected_paths.is_empty()
            || !self.system_prompt_additions.is_empty()
            || !self.role_rules.is_empty()
            || self.architecture_doc.is_some()
    }
}

/// Merge two vecs: base first, then overlay items that aren't already present.
pub(crate) fn merge_vec(base: Vec<String>, overlay: Vec<String>) -> Vec<String> {
    let mut result = base;
    for item in overlay {
        if !result.contains(&item) {
            result.push(item);
        }
    }
    result
}

/// Merge two HashMaps: overlay entries extend existing keys.
pub(crate) fn merge_map(
    base: HashMap<String, Vec<String>>,
    overlay: HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>> {
    let mut result = base;
    for (key, values) in overlay {
        result
            .entry(key)
            .and_modify(|existing| {
                for v in &values {
                    if !existing.contains(v) {
                        existing.push(v.clone());
                    }
                }
            })
            .or_insert(values);
    }
    result
}
