//! Daemon worker trait, shared types, and built-in worker registry.

mod codebase_indexer;
mod dependency;
mod documentation;
mod memory_consolidation;
mod performance;
mod security;
mod test_gap;

use std::path::Path;

pub use codebase_indexer::CodebaseIndexer;
pub use dependency::DependencyChecker;
pub use documentation::AutoDocumentationWorker;
pub use memory_consolidation::MemoryConsolidator;
pub use performance::PerformanceBenchmarker;
pub use security::SecurityAuditWorker;
pub use test_gap::TestGapAnalyzer;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A daemon worker that performs periodic maintenance tasks.
pub trait DaemonWorker: Send + Sync {
    /// Unique name for this worker.
    fn name(&self) -> &str;

    /// Human-readable description of what this worker does.
    fn description(&self) -> &str;

    /// Cron schedule (5-field expression). Only simple interval forms
    /// like `*/10 * * * *` or `0 * * * *` are supported for interval
    /// parsing. The scheduler falls back to 60 minutes for unrecognised
    /// expressions.
    fn schedule(&self) -> &str;

    /// Execute the worker's task.
    fn execute(&self, ctx: &WorkerContext) -> WorkerResult;
}

/// Outcome of a single worker execution.
#[derive(Debug, Clone)]
pub struct WorkerResult {
    pub status: WorkerStatus,
    pub message: String,
    pub items_processed: usize,
}

/// Status of a worker run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStatus {
    Success,
    Partial,
    Failed,
}

/// Context passed to workers for execution.
#[derive(Debug, Clone)]
pub struct WorkerContext {
    pub project_root: std::path::PathBuf,
    pub timestamp: String,
}

/// Convenience struct exposing all built-in workers.
pub struct BuiltinWorkers;

impl BuiltinWorkers {
    /// Return a boxed list of every built-in daemon worker.
    pub fn all() -> Vec<Box<dyn DaemonWorker>> {
        vec![
            Box::new(CodebaseIndexer),
            Box::new(TestGapAnalyzer),
            Box::new(MemoryConsolidator),
            Box::new(SecurityAuditWorker),
            Box::new(PerformanceBenchmarker),
            Box::new(DependencyChecker),
            Box::new(AutoDocumentationWorker),
        ]
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Source file extensions recognised by the scanner.
const SOURCE_EXTENSIONS: &[&str] = &["rs", "ts", "js", "py"];

/// Recursively collect source files under `dir`.
pub(crate) fn collect_source_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return files,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_source_files(&path));
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| SOURCE_EXTENSIONS.contains(&e))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    files
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_workers_returns_all() {
        let workers = BuiltinWorkers::all();
        assert_eq!(workers.len(), 7);
        let names: Vec<&str> = workers.iter().map(|w| w.name()).collect();
        assert!(names.contains(&"codebase_indexer"));
        assert!(names.contains(&"test_gap_analyzer"));
        assert!(names.contains(&"memory_consolidator"));
        assert!(names.contains(&"security_audit"));
        assert!(names.contains(&"performance_benchmarker"));
        assert!(names.contains(&"dependency_checker"));
        assert!(names.contains(&"auto_documentation"));
    }

    #[test]
    fn test_collect_source_files_empty_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = collect_source_files(dir.path());
        assert!(files.is_empty());
    }

    #[test]
    fn test_collect_source_files_finds_sources() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
        std::fs::write(dir.path().join("readme.md"), "hello").expect("write");
        let files = collect_source_files(dir.path());
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("main.rs"));
    }

    #[test]
    fn test_worker_result_debug() {
        let result = WorkerResult {
            status: WorkerStatus::Success,
            message: "ok".to_string(),
            items_processed: 5,
        };
        let debug_str = format!("{result:?}");
        assert!(debug_str.contains("Success"));
    }

    #[test]
    fn test_worker_status_equality() {
        assert_eq!(WorkerStatus::Success, WorkerStatus::Success);
        assert_ne!(WorkerStatus::Success, WorkerStatus::Failed);
    }
}
