//! Test gap analyzer daemon worker.

use std::path::Path;

use tracing::{debug, info};

use super::{collect_source_files, DaemonWorker, WorkerContext, WorkerResult, WorkerStatus};

/// Identifies source files without corresponding test files.
pub struct TestGapAnalyzer;

/// Directory names where test files are commonly located.
pub(crate) const TEST_DIR_NAMES: &[&str] = &["tests", "test", "__tests__", "spec"];

impl DaemonWorker for TestGapAnalyzer {
    fn name(&self) -> &str {
        "test_gap_analyzer"
    }

    fn description(&self) -> &str {
        "Detects source files that lack corresponding test coverage"
    }

    fn schedule(&self) -> &str {
        "*/30 * * * *"
    }

    fn execute(&self, ctx: &WorkerContext) -> WorkerResult {
        info!(project = ?ctx.project_root, "TestGapAnalyzer starting");

        let src_dir = ctx.project_root.join("src");
        if !src_dir.is_dir() {
            return WorkerResult {
                status: WorkerStatus::Success,
                message: "No src/ directory found; nothing to analyse".to_string(),
                items_processed: 0,
            };
        }

        let source_files = collect_source_files(&src_dir);
        let mut untested = Vec::new();

        for src_file in &source_files {
            if !has_corresponding_test(&ctx.project_root, src_file) {
                untested.push(src_file.clone());
            }
        }

        let count = untested.len();
        debug!(untested = count, "TestGapAnalyzer finished");

        let message = if count == 0 {
            "All source files have corresponding tests".to_string()
        } else {
            let preview: Vec<String> = untested
                .iter()
                .take(5)
                .map(|p| {
                    p.strip_prefix(&ctx.project_root)
                        .unwrap_or(p)
                        .to_string_lossy()
                        .to_string()
                })
                .collect();
            format!(
                "Found {count} untested files (e.g.: {})",
                preview.join(", ")
            )
        };

        WorkerResult {
            status: if count == 0 {
                WorkerStatus::Success
            } else {
                WorkerStatus::Partial
            },
            message,
            items_processed: count,
        }
    }
}

/// Check whether a test file corresponding to `src_file` exists under the
/// project root. Looks in `tests/`, `src/` (Rust convention), and
/// `__tests__/` directories.
fn has_corresponding_test(project_root: &Path, src_file: &Path) -> bool {
    let stem = src_file.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    // Derive relative path from src/ for matching test directory structure.
    let src_dir = project_root.join("src");
    let relative = src_file
        .strip_prefix(&src_dir)
        .unwrap_or(src_file)
        .parent()
        .unwrap_or(Path::new(""));

    let candidates = build_test_candidates(project_root, stem, relative);
    candidates.iter().any(|p| p.exists())
}

/// Build a list of possible test file paths for a given source file.
fn build_test_candidates(
    project_root: &Path,
    stem: &str,
    relative: &Path,
) -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();

    for dir_name in TEST_DIR_NAMES {
        // <project>/tests/<relative>/<stem>_test.<ext> or test_<stem>.<ext>
        let test_dir = project_root.join(dir_name).join(relative);
        candidates.push(test_dir.join(format!("{stem}_test.rs")));
        candidates.push(test_dir.join(format!("test_{stem}.rs")));
        candidates.push(test_dir.join(format!("{stem}.test.ts")));
        candidates.push(test_dir.join(format!("{stem}.test.js")));
        candidates.push(test_dir.join(format!("{stem}_test.py")));
        candidates.push(test_dir.join(format!("test_{stem}.py")));
    }

    // Rust convention: src/<relative>/<stem>.rs -> src/<relative>/<stem>/tests.rs
    if let Some(ext) = relative.to_str() {
        if !ext.is_empty() {
            candidates.push(
                project_root
                    .join("src")
                    .join(relative)
                    .join(format!("{stem}/tests.rs")),
            );
        }
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::daemon::scheduler::parse_interval_minutes;
    use std::path::PathBuf;

    #[test]
    fn test_test_gap_analyzer_detects_missing_tests() {
        let worker = TestGapAnalyzer;
        assert_eq!(worker.name(), "test_gap_analyzer");
        assert_eq!(worker.schedule(), "*/30 * * * *");
        let interval = parse_interval_minutes(worker.schedule());
        assert_eq!(interval, 30);
    }

    #[test]
    fn test_test_gap_analyzer_no_src_dir() {
        let worker = TestGapAnalyzer;
        let ctx = WorkerContext {
            project_root: PathBuf::from("/tmp/nonexistent_d3vx_test_12345"),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = worker.execute(&ctx);
        assert_eq!(result.status, WorkerStatus::Success);
        assert_eq!(result.items_processed, 0);
    }
}
