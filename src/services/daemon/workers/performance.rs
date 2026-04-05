//! Performance benchmarking daemon worker.

use tracing::info;

use super::{collect_source_files, DaemonWorker, WorkerContext, WorkerResult, WorkerStatus};

/// Tracks file sizes and warns on bloat.
pub struct PerformanceBenchmarker;

/// File size threshold in bytes (100 KB).
const FILE_SIZE_THRESHOLD: u64 = 100_000;

impl DaemonWorker for PerformanceBenchmarker {
    fn name(&self) -> &str {
        "performance_benchmarker"
    }

    fn description(&self) -> &str {
        "Tracks file sizes and warns when files exceed bloat thresholds"
    }

    fn schedule(&self) -> &str {
        "*/20 * * * *"
    }

    fn execute(&self, ctx: &WorkerContext) -> WorkerResult {
        info!(project = ?ctx.project_root, "PerformanceBenchmarker starting");

        let src_dir = ctx.project_root.join("src");
        if !src_dir.is_dir() {
            return WorkerResult {
                status: WorkerStatus::Success,
                message: "No src/ directory to benchmark".to_string(),
                items_processed: 0,
            };
        }

        let source_files = collect_source_files(&src_dir);
        let mut large_files = Vec::new();
        let mut total_size: u64 = 0;

        for file_path in &source_files {
            if let Ok(metadata) = file_path.metadata() {
                let size = metadata.len();
                total_size += size;
                if size > FILE_SIZE_THRESHOLD {
                    let short_path = file_path
                        .strip_prefix(&ctx.project_root)
                        .unwrap_or(file_path)
                        .to_string_lossy();
                    let kb = size / 1024;
                    large_files.push(format!("{short_path} ({kb} KB)"));
                }
            }
        }

        let count = large_files.len();
        if count > 0 {
            let preview: Vec<String> = large_files.iter().take(5).cloned().collect();
            let total_kb = total_size / 1024;
            WorkerResult {
                status: WorkerStatus::Partial,
                message: format!(
                    "{count} large files (>{:.0} KB), total src size: {total_kb} KB: {}",
                    FILE_SIZE_THRESHOLD as f64 / 1024.0,
                    preview.join(", ")
                ),
                items_processed: count,
            }
        } else {
            let total_kb = total_size / 1024;
            info!(
                total_kb,
                "PerformanceBenchmarker: all files within thresholds"
            );
            WorkerResult {
                status: WorkerStatus::Success,
                message: format!("All source files under size threshold (total: {total_kb} KB)"),
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
    fn test_performance_benchmarker_metadata() {
        let worker = PerformanceBenchmarker;
        assert_eq!(worker.name(), "performance_benchmarker");
        assert_eq!(worker.schedule(), "*/20 * * * *");
        let interval = parse_interval_minutes(worker.schedule());
        assert_eq!(interval, 20);
    }

    #[test]
    fn test_performance_benchmarker_no_src_dir() {
        let worker = PerformanceBenchmarker;
        let ctx = WorkerContext {
            project_root: PathBuf::from("/tmp/nonexistent_d3vx_perf_test_12345"),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = worker.execute(&ctx);
        assert_eq!(result.status, WorkerStatus::Success);
    }

    #[test]
    fn test_performance_benchmarker_detects_large_file() {
        let worker = PerformanceBenchmarker;
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        // Write a file larger than 100 KB.
        let large_content = "x".repeat(101_000);
        std::fs::write(src.join("big.rs"), &large_content).expect("write");
        let ctx = WorkerContext {
            project_root: dir.path().to_path_buf(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = worker.execute(&ctx);
        assert_eq!(result.status, WorkerStatus::Partial);
        assert_eq!(result.items_processed, 1);
    }
}
