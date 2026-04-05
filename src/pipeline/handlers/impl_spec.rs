//! Implementation specification — structured context from Plan to Implement.
//!
//! Rather than shoving the full Plan-phase conversation into the Implement
//! agent's context (which dilutes signal with noise), the Plan handler emits
//! a compact, typed spec that the Implement handler injects directly.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// A structured implementation specification produced by the Plan phase.
///
/// This is the primary context for the Implement phase agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationSpec {
    /// Short summary of what is being implemented
    pub summary: String,
    /// Subtasks to complete (carried over from the plan)
    #[serde(default)]
    pub subtasks: Vec<Subtask>,
    /// Files that need to be created with their purpose
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_to_create: Vec<FileTarget>,
    /// Files that need to be modified and what to change
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_to_modify: Vec<FileTarget>,
    /// New or modified public API signatures expected after implementation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_signatures: Vec<String>,
    /// Acceptance criteria that must be met
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_criteria: Vec<String>,
    /// Constraints (performance limits, existing APIs to call, patterns to follow)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
    /// Verification commands to run after implementation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification: Vec<String>,
    /// Known risks and mitigations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
}

/// A single subtask from the plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
    /// Subtask identifier (e.g., "ST-001")
    pub id: String,
    /// What needs to be done
    pub description: String,
    /// Files this subtask touches
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    /// Whether this subtask is complete
    #[serde(default)]
    pub status: SubtaskStatus,
    /// Subtask IDs this depends on
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

/// Status of a plan subtask.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubtaskStatus {
    #[default]
    Pending,
    Completed,
    Skipped,
}

/// A file target with its purpose or description of changes needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTarget {
    /// File path (relative to worktree root)
    pub path: String,
    /// Brief description of what to create or change
    pub purpose: String,
}

impl ImplementationSpec {
    /// Create a new spec from the Plan phase JSON output.
    ///
    /// This method parses the agent's raw plan JSON (which the plan prompt
    /// instructs the agent to write) into a structured spec suitable for
    /// injection into the Implement phase.
    pub fn from_plan_json(json: &str) -> Result<Self, serde_json::Error> {
        // Try parsing directly as ImplementationSpec first
        serde_json::from_str::<Self>(json).or_else(|_| {
            // Fall back to legacy plan format and convert
            serde_json::from_str::<LegacyPlan>(json).map(|legacy| Self::from_legacy(&legacy))
        })
    }

    /// Load a spec from a file on disk.
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_plan_json(&content)?)
    }

    /// Serialize this spec to a pretty JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Format the spec as a readable text block for injecting into
    /// the Implement agent's instruction message.
    pub fn to_instruction_block(&self) -> String {
        let mut out = String::from("## Implementation Specification\n\n");
        out.push_str(&format!("**Summary:** {}\n\n", self.summary));

        if !self.subtasks.is_empty() {
            out.push_str("### Subtasks\n\n");
            for sub in &self.subtasks {
                let status = match sub.status {
                    SubtaskStatus::Pending => "PENDING",
                    SubtaskStatus::Completed => "DONE",
                    SubtaskStatus::Skipped => "SKIPPED",
                };
                let deps = if sub.dependencies.is_empty() {
                    String::new()
                } else {
                    format!(" (depends on: {})", sub.dependencies.join(", "))
                };
                out.push_str(&format!(
                    "- [{}] {}{} {}\n",
                    sub.id, status, deps, sub.description
                ));
            }
            out.push('\n');
        }

        if !self.files_to_create.is_empty() {
            out.push_str("### Files to Create\n\n");
            for f in &self.files_to_create {
                out.push_str(&format!("- `{}` — {}\n", f.path, f.purpose));
            }
            out.push('\n');
        }

        if !self.files_to_modify.is_empty() {
            out.push_str("### Files to Modify\n\n");
            for f in &self.files_to_modify {
                out.push_str(&format!("- `{}` — {}\n", f.path, f.purpose));
            }
            out.push('\n');
        }

        if !self.acceptance_criteria.is_empty() {
            out.push_str("### Acceptance Criteria\n\n");
            for c in &self.acceptance_criteria {
                out.push_str(&format!("- {}\n", c));
            }
            out.push('\n');
        }

        if !self.constraints.is_empty() {
            out.push_str("### Constraints\n\n");
            for c in &self.constraints {
                out.push_str(&format!("- {}\n", c));
            }
            out.push('\n');
        }

        if !self.api_signatures.is_empty() {
            out.push_str("### Expected API Surface\n\n");
            for sig in &self.api_signatures {
                out.push_str(&format!("- {}\n", sig));
            }
            out.push('\n');
        }

        if !self.risks.is_empty() {
            out.push_str("### Risks\n\n");
            for r in &self.risks {
                out.push_str(&format!("- {}\n", r));
            }
            out.push('\n');
        }

        if !self.verification.is_empty() {
            out.push_str("### Verification Commands\n\n");
            for cmd in &self.verification {
                out.push_str(&format!("- `{}`\n", cmd));
            }
        }

        out
    }

    fn from_legacy(plan: &LegacyPlan) -> Self {
        Self {
            summary: plan.summary.clone(),
            subtasks: plan
                .subtasks
                .iter()
                .map(|s| Subtask {
                    id: s.id.clone(),
                    description: s.description.clone(),
                    files: s.files.clone(),
                    status: SubtaskStatus::Pending,
                    dependencies: s.dependencies.clone(),
                })
                .collect(),
            files_to_create: Vec::new(),
            files_to_modify: Vec::new(),
            api_signatures: Vec::new(),
            acceptance_criteria: plan.verification.clone(),
            constraints: Vec::new(),
            verification: plan.verification.clone(),
            risks: plan.risks.clone(),
        }
    }
}

/// The legacy plan format the Plan agent originally produced.
///
/// Kept for backwards compatibility — converted to ImplementationSpec on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyPlan {
    task_id: String,
    summary: String,
    subtasks: Vec<LegacySubtask>,
    risks: Vec<String>,
    verification: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacySubtask {
    id: String,
    description: String,
    files: Vec<String>,
    status: String,
    dependencies: Vec<String>,
}

// Legacy types are only used internally via `from_plan_json` and `load`.

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec() -> ImplementationSpec {
        ImplementationSpec {
            summary: "Add rate limiting".to_string(),
            subtasks: vec![Subtask {
                id: "ST-001".to_string(),
                description: "Create the rate limiter module".to_string(),
                files: vec!["src/rate_limiter.rs".to_string()],
                status: SubtaskStatus::Pending,
                dependencies: vec![],
            }],
            files_to_create: vec![FileTarget {
                path: "src/rate_limiter.rs".to_string(),
                purpose: "Token bucket rate limiter".to_string(),
            }],
            files_to_modify: vec![FileTarget {
                path: "src/main.rs".to_string(),
                purpose: "Wire rate limiter into request pipeline".to_string(),
            }],
            api_signatures: vec![
                "pub struct RateLimiter".to_string(),
                "impl RateLimiter::new(max_rpm: u32) -> Self".to_string(),
            ],
            acceptance_criteria: vec!["Returns 429 when limit exceeded".to_string()],
            constraints: vec!["Use 100 req/min default limit".to_string()],
            verification: vec!["cargo check".to_string()],
            risks: vec![],
        }
    }

    #[test]
    fn test_roundtrip_json() {
        let spec = sample_spec();
        let json = spec.to_json().unwrap();
        let parsed = ImplementationSpec::from_plan_json(&json).unwrap();
        assert_eq!(spec.summary, parsed.summary);
        assert_eq!(spec.subtasks.len(), parsed.subtasks.len());
        assert_eq!(spec.files_to_create.len(), parsed.files_to_create.len());
    }

    #[test]
    fn test_from_legacy_plan() {
        let legacy = r#"{
            "task_id": "T-001",
            "summary": "test plan",
            "subtasks": [
                {"id": "ST-001", "description": "do something", "files": ["a.rs"], "status": "pending", "dependencies": []}
            ],
            "risks": ["risk1"],
            "verification": ["cargo check"]
        }"#;

        let spec = ImplementationSpec::from_plan_json(legacy).unwrap();
        assert_eq!(spec.summary, "test plan");
        assert_eq!(spec.subtasks.len(), 1);
        assert_eq!(spec.subtasks[0].id, "ST-001");
        assert_eq!(spec.verification, vec!["cargo check"]);
    }

    #[test]
    fn test_to_instruction_block_contains_subtasks() {
        let spec = sample_spec();
        let block = spec.to_instruction_block();
        assert!(block.contains("rate limiter"));
        assert!(block.contains("ST-001"));
        assert!(block.contains("PENDING"));
    }

    #[test]
    fn test_subtask_status_default() {
        assert_eq!(SubtaskStatus::default(), SubtaskStatus::Pending);
    }
}
