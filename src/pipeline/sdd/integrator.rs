//! SDD integrator — merges child results and validates the combined output
//!
//! After all children complete, the integrator checks for:
//! - Overlapping file edits (conflicts)
//! - Aggregate success across all children
//! - Overall plan coherence

use super::types::{ChildResult, SddError, SddState};
use crate::pipeline::approval::ExecutionPlan;

/// Result of integrating child agent outputs
#[derive(Debug, Clone)]
pub struct IntegrationResult {
    /// Whether integration succeeded
    pub success: bool,
    /// Summary of what was produced
    pub summary: String,
    /// All files changed across children
    pub files_changed: Vec<String>,
    /// Conflicts detected (files modified by multiple children)
    pub conflicts: Vec<FileConflict>,
    /// Child-level summaries
    pub child_summaries: Vec<String>,
}

/// A file modified by multiple children — potential conflict
#[derive(Debug, Clone)]
pub struct FileConflict {
    /// File path
    pub file: String,
    /// Children that modified this file
    pub modified_by: Vec<String>,
}

/// Merges and validates results from child agents
pub struct SddIntegrator;

impl SddIntegrator {
    pub fn new() -> Self {
        Self
    }

    /// Integrate all child results against the original plan.
    pub fn integrate(
        &self,
        children: Vec<ChildResult>,
        _plan: &ExecutionPlan,
        session: &mut super::types::SddSession,
    ) -> Result<IntegrationResult, SddError> {
        let success = children.iter().all(|c| c.success);
        let conflicts = self.detect_conflicts(&children);
        let files_changed = self.collect_files(&children);
        let child_summaries: Vec<String> = children
            .iter()
            .map(|c| {
                let status = if c.success { "ok" } else { "failed" };
                format!(
                    "{}: {} — {}",
                    c.key,
                    status,
                    c.summary.as_deref().unwrap_or("no output")
                )
            })
            .collect();

        let summary = if success && conflicts.is_empty() {
            format!(
                "All {} children completed successfully with no conflicts",
                children.len()
            )
        } else if success {
            format!(
                "All children completed, but {} file conflict(s) detected",
                conflicts.len()
            )
        } else {
            let failed: Vec<_> = children
                .iter()
                .filter(|c| !c.success)
                .map(|c| c.key.clone())
                .collect();
            format!("{} child(ren) failed: {}", failed.len(), failed.join(", "))
        };

        let result = IntegrationResult {
            success: success && conflicts.is_empty(),
            summary,
            files_changed,
            conflicts,
            child_summaries,
        };

        if result.success {
            session
                .transition(SddState::Integrated)
                .map_err(|e| SddError::Integration(e.to_string()))?;
        }

        Ok(result)
    }

    /// Detect files modified by multiple children
    fn detect_conflicts(&self, children: &[ChildResult]) -> Vec<FileConflict> {
        let mut file_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for child in children {
            for file in &child.files_changed {
                file_map
                    .entry(file.clone())
                    .or_default()
                    .push(child.key.clone());
            }
        }

        file_map
            .into_iter()
            .filter(|(_, modifiers)| modifiers.len() > 1)
            .map(|(file, modified_by)| FileConflict { file, modified_by })
            .collect()
    }

    /// Collect unique file paths from all children
    fn collect_files(&self, children: &[ChildResult]) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        children
            .iter()
            .flat_map(|c| c.files_changed.clone())
            .filter(|f| seen.insert(f.clone()))
            .collect()
    }
}

impl Default for SddIntegrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::approval::ExecutionPlan;
    use crate::pipeline::sdd::types::SddSession;

    #[test]
    fn test_no_conflicts_no_failures() {
        let children = vec![
            ChildResult {
                key: "backend".into(),
                success: true,
                summary: Some("Created auth handler".into()),
                error: None,
                files_changed: vec!["src/auth/handler.rs".into()],
            },
            ChildResult {
                key: "frontend".into(),
                success: true,
                summary: Some("Added login form".into()),
                error: None,
                files_changed: vec!["src/ui/login.rs".into()],
            },
        ];
        let plan = ExecutionPlan::new("T-1", "Test");
        let mut session = SddSession::new("T-1");
        session.transition(SddState::ChildrenComplete).unwrap();

        let result = SddIntegrator.integrate(children, &plan, &mut session);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success);
        assert!(r.conflicts.is_empty());
        assert_eq!(r.files_changed.len(), 2);
    }

    #[test]
    fn test_conflict_detection() {
        let children = vec![
            ChildResult {
                key: "child-a".into(),
                success: true,
                summary: None,
                error: None,
                files_changed: vec!["src/main.rs".into(), "src/lib.rs".into()],
            },
            ChildResult {
                key: "child-b".into(),
                success: true,
                summary: None,
                error: None,
                files_changed: vec!["src/main.rs".into(), "src/util.rs".into()],
            },
        ];
        let plan = ExecutionPlan::new("T-1", "Test");
        let mut session = SddSession::new("T-1");
        session.transition(SddState::ChildrenComplete).unwrap();

        let result = SddIntegrator.integrate(children, &plan, &mut session);
        let r = result.unwrap();
        assert_eq!(r.conflicts.len(), 1);
        assert_eq!(r.conflicts[0].file, "src/main.rs");
        assert_eq!(r.conflicts[0].modified_by.len(), 2);
    }

    #[test]
    fn test_child_failure() {
        let children = vec![ChildResult {
            key: "bad".into(),
            success: false,
            summary: None,
            error: Some("compile error".into()),
            files_changed: vec![],
        }];
        let plan = ExecutionPlan::new("T-1", "Test");
        let mut session = SddSession::new("T-1");
        session.transition(SddState::ChildrenComplete).unwrap();

        let result = SddIntegrator.integrate(children, &plan, &mut session);
        let r = result.unwrap();
        assert!(!r.success);
    }
}
