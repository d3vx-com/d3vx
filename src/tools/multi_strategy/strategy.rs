//! Strategy enum and parsing utilities for multi-strategy execution.

use serde_json::Value;

/// Available implementation strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    Concise,
    Thorough,
    Creative,
}

impl Strategy {
    /// All available strategies in canonical order
    pub fn all() -> &'static [Strategy] {
        &[Strategy::Concise, Strategy::Thorough, Strategy::Creative]
    }

    /// Parse a strategy from its string name (case-insensitive)
    pub fn from_name(name: &str) -> Option<Strategy> {
        match name.to_lowercase().as_str() {
            "concise" => Some(Strategy::Concise),
            "thorough" => Some(Strategy::Thorough),
            "creative" => Some(Strategy::Creative),
            _ => None,
        }
    }

    /// Human-readable name for this strategy
    pub fn name(&self) -> &'static str {
        match self {
            Strategy::Concise => "concise",
            Strategy::Thorough => "thorough",
            Strategy::Creative => "creative",
        }
    }

    /// Short description of what this strategy emphasizes
    pub fn description(&self) -> &'static str {
        match self {
            Strategy::Concise => "Minimal, direct approach — least code possible",
            Strategy::Thorough => "Comprehensive with error handling, tests, and documentation",
            Strategy::Creative => "Innovative and unconventional approaches",
        }
    }

    /// Generate a modified prompt emphasizing this strategy's approach
    pub fn generate_prompt(&self, task: &str) -> String {
        match self {
            Strategy::Concise => format!(
                "{task}\n\n\
                Approach: Be minimal and direct. Write the least code possible \
                to solve the problem. Avoid over-engineering, skip optional features, \
                and prefer simple data structures. Every line must earn its place."
            ),
            Strategy::Thorough => format!(
                "{task}\n\n\
                Approach: Be comprehensive and robust. Add proper error handling \
                for every edge case, write tests for critical paths, include \
                documentation for public APIs, validate all inputs, and consider \
                backwards compatibility. Prefer explicit over implicit."
            ),
            Strategy::Creative => format!(
                "{task}\n\n\
                Approach: Be innovative and think outside the box. Consider \
                unconventional solutions, explore alternative algorithms or data \
                structures, and look for elegant abstractions. Challenge assumptions \
                about the 'obvious' way to solve this. Prefer cleverness that \
                simplifies, not complicates."
            ),
        }
    }
}

/// Parse strategy names from a JSON value, falling back to defaults on failure.
pub fn parse_strategies(input: &Value) -> Vec<Strategy> {
    let names = match input.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
            .collect::<Vec<_>>(),
        None => return Strategy::all().to_vec(),
    };

    if names.is_empty() {
        return Strategy::all().to_vec();
    }

    let parsed: Vec<Strategy> = names
        .iter()
        .filter_map(|n| Strategy::from_name(n))
        .collect();

    if parsed.is_empty() {
        return Strategy::all().to_vec();
    }

    parsed
}

/// Clamp the max_agents value to the allowed range [2, 3].
pub fn clamp_max_agents(value: Option<&Value>) -> usize {
    let raw = value.and_then(|v| v.as_u64()).unwrap_or(2);
    raw.clamp(2, 3) as usize
}
