//! Codebase indexer daemon worker.

use tracing::{info, warn};

use super::{DaemonWorker, WorkerContext, WorkerResult, WorkerStatus};

/// Re-indexes the code map periodically.
pub struct CodebaseIndexer;

impl DaemonWorker for CodebaseIndexer {
    fn name(&self) -> &str {
        "codebase_indexer"
    }

    fn description(&self) -> &str {
        "Re-indexes the project code map for symbol search"
    }

    fn schedule(&self) -> &str {
        "*/10 * * * *"
    }

    fn execute(&self, ctx: &WorkerContext) -> WorkerResult {
        info!(project = ?ctx.project_root, "CodebaseIndexer starting");

        match crate::services::analysis::code_map::build_code_map(&ctx.project_root) {
            Ok(code_map) => {
                let count = code_map.files.len();
                info!(files = count, "CodebaseIndexer completed");
                WorkerResult {
                    status: WorkerStatus::Success,
                    message: format!("Indexed {count} source files"),
                    items_processed: count,
                }
            }
            Err(err) => {
                warn!(error = %err, "CodebaseIndexer failed");
                WorkerResult {
                    status: WorkerStatus::Failed,
                    message: format!("Indexing failed: {err}"),
                    items_processed: 0,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::daemon::scheduler::parse_interval_minutes;

    #[test]
    fn test_codebase_indexer_name_and_schedule() {
        let worker = CodebaseIndexer;
        assert_eq!(worker.name(), "codebase_indexer");
        assert_eq!(worker.schedule(), "*/10 * * * *");
        let interval = parse_interval_minutes(worker.schedule());
        assert_eq!(interval, 10);
    }
}
