//! Task-related enum types
//!
//! ExecutionMode and AgentRole enums with their Display and FromStr impls.

use serde::{Deserialize, Serialize};

/// Execution mode for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionMode {
    /// Execute directly in the main workspace
    Direct,
    /// Execute in a vex worktree
    Vex,
    /// Automatically choose based on task characteristics
    Auto,
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Direct => write!(f, "DIRECT"),
            ExecutionMode::Vex => write!(f, "VEX"),
            ExecutionMode::Auto => write!(f, "AUTO"),
        }
    }
}

impl std::str::FromStr for ExecutionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "DIRECT" => Ok(ExecutionMode::Direct),
            "VEX" => Ok(ExecutionMode::Vex),
            "AUTO" => Ok(ExecutionMode::Auto),
            _ => Err(format!("Invalid execution mode: {}", s)),
        }
    }
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::Auto
    }
}

/// Agent role for task assignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentRole {
    TechLead,
    Executor,
    Coder,
    Documenter,
    SpecReviewer,
    QualityReviewer,
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRole::TechLead => write!(f, "TECH_LEAD"),
            AgentRole::Executor => write!(f, "EXECUTOR"),
            AgentRole::Coder => write!(f, "CODER"),
            AgentRole::Documenter => write!(f, "DOCUMENTER"),
            AgentRole::SpecReviewer => write!(f, "SPEC_REVIEWER"),
            AgentRole::QualityReviewer => write!(f, "QUALITY_REVIEWER"),
        }
    }
}

impl std::str::FromStr for AgentRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TECH_LEAD" => Ok(AgentRole::TechLead),
            "EXECUTOR" => Ok(AgentRole::Executor),
            "CODER" => Ok(AgentRole::Coder),
            "DOCUMENTER" => Ok(AgentRole::Documenter),
            "SPEC_REVIEWER" => Ok(AgentRole::SpecReviewer),
            "QUALITY_REVIEWER" => Ok(AgentRole::QualityReviewer),
            _ => Err(format!("Invalid agent role: {}", s)),
        }
    }
}
