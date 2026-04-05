//! Memory consolidation daemon worker.

use tracing::{debug, info, warn};

use super::{DaemonWorker, WorkerContext, WorkerResult, WorkerStatus};

/// Cleans up stale session data.
pub struct MemoryConsolidator;

impl DaemonWorker for MemoryConsolidator {
    fn name(&self) -> &str {
        "memory_consolidator"
    }

    fn description(&self) -> &str {
        "Cleans up stale session data older than 24 hours"
    }

    fn schedule(&self) -> &str {
        "0 * * * *"
    }

    fn execute(&self, ctx: &WorkerContext) -> WorkerResult {
        info!(project = ?ctx.project_root, "MemoryConsolidator starting");

        let sessions_dir = ctx.project_root.join(".d3vx").join("sessions");
        if !sessions_dir.is_dir() {
            debug!("No sessions directory found");
            return WorkerResult {
                status: WorkerStatus::Success,
                message: "No sessions directory to consolidate".to_string(),
                items_processed: 0,
            };
        }

        let mut cleaned = 0usize;
        let threshold = std::time::SystemTime::now() - std::time::Duration::from_secs(24 * 3600);

        match std::fs::read_dir(&sessions_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    if let Ok(metadata) = path.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if modified < threshold {
                                debug!(path = ?path, "Removing stale session file");
                                if std::fs::remove_file(&path).is_err() {
                                    warn!(path = ?path, "Failed to remove stale session");
                                } else {
                                    cleaned += 1;
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => {
                warn!(error = %err, "Failed to read sessions directory");
                return WorkerResult {
                    status: WorkerStatus::Failed,
                    message: format!("Failed to read sessions: {err}"),
                    items_processed: 0,
                };
            }
        }

        info!(cleaned, "MemoryConsolidator completed");
        WorkerResult {
            status: WorkerStatus::Success,
            message: format!("Cleaned {cleaned} stale session files"),
            items_processed: cleaned,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::daemon::scheduler::parse_interval_minutes;

    #[test]
    fn test_memory_consolidator_name() {
        let worker = MemoryConsolidator;
        assert_eq!(worker.name(), "memory_consolidator");
        assert_eq!(
            worker.description(),
            "Cleans up stale session data older than 24 hours"
        );
        let interval = parse_interval_minutes(worker.schedule());
        assert_eq!(interval, 60);
    }
}
