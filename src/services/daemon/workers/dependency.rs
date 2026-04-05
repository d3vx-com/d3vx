//! Dependency checker daemon worker.

use tracing::{info, warn};

use super::{DaemonWorker, WorkerContext, WorkerResult, WorkerStatus};

/// Checks for outdated dependencies in Cargo.toml.
pub struct DependencyChecker;

impl DaemonWorker for DependencyChecker {
    fn name(&self) -> &str {
        "dependency_checker"
    }

    fn description(&self) -> &str {
        "Checks for outdated or missing dependencies in Cargo.toml"
    }

    fn schedule(&self) -> &str {
        "0 * * * *"
    }

    fn execute(&self, ctx: &WorkerContext) -> WorkerResult {
        info!(project = ?ctx.project_root, "DependencyChecker starting");

        let cargo_toml = ctx.project_root.join("Cargo.toml");
        if !cargo_toml.is_file() {
            return WorkerResult {
                status: WorkerStatus::Success,
                message: "No Cargo.toml found".to_string(),
                items_processed: 0,
            };
        }

        let content = match std::fs::read_to_string(&cargo_toml) {
            Ok(c) => c,
            Err(err) => {
                warn!(error = %err, "Failed to read Cargo.toml");
                return WorkerResult {
                    status: WorkerStatus::Failed,
                    message: format!("Failed to read Cargo.toml: {err}"),
                    items_processed: 0,
                };
            }
        };

        let mut warnings = Vec::new();

        // Check for use of wildcard versions.
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.contains("= \"*\"") || trimmed.contains("= \"*") {
                warnings.push(format!("Wildcard dependency: {trimmed}"));
            }
            // Check for git dependencies without a rev/tag.
            if trimmed.contains("git =") && !trimmed.contains("rev =") && !trimmed.contains("tag =")
            {
                warnings.push(format!("Git dependency without pinned rev/tag: {trimmed}"));
            }
        }

        // Check for missing Cargo.lock.
        let cargo_lock = ctx.project_root.join("Cargo.lock");
        if !cargo_lock.is_file() {
            warnings.push("Cargo.lock is missing (commit it for reproducible builds)".to_string());
        }

        let count = warnings.len();
        if count > 0 {
            let preview: Vec<String> = warnings.iter().take(5).cloned().collect();
            WorkerResult {
                status: WorkerStatus::Partial,
                message: format!("Found {count} dependency issues: {}", preview.join("; ")),
                items_processed: count,
            }
        } else {
            info!("DependencyChecker: no issues found");
            WorkerResult {
                status: WorkerStatus::Success,
                message: "All dependencies look good".to_string(),
                items_processed: 0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::daemon::scheduler::parse_interval_minutes;
    use std::path::PathBuf;

    #[test]
    fn test_dependency_checker_metadata() {
        let worker = DependencyChecker;
        assert_eq!(worker.name(), "dependency_checker");
        assert_eq!(worker.schedule(), "0 * * * *");
        let interval = parse_interval_minutes(worker.schedule());
        assert_eq!(interval, 60);
    }

    #[test]
    fn test_dependency_checker_no_cargo_toml() {
        let worker = DependencyChecker;
        let ctx = WorkerContext {
            project_root: PathBuf::from("/tmp/nonexistent_d3vx_dep_test_12345"),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = worker.execute(&ctx);
        assert_eq!(result.status, WorkerStatus::Success);
        assert_eq!(result.items_processed, 0);
    }

    #[test]
    fn test_dependency_checker_detects_wildcard() {
        let worker = DependencyChecker;
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[dependencies]\nfoo = \"*\"\n",
        )
        .expect("write");
        let ctx = WorkerContext {
            project_root: dir.path().to_path_buf(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = worker.execute(&ctx);
        assert_eq!(result.status, WorkerStatus::Partial);
        assert!(result.items_processed > 0);
    }
}
