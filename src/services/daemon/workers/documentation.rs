//! Auto-documentation daemon worker.

use tracing::info;

use super::{collect_source_files, DaemonWorker, WorkerContext, WorkerResult, WorkerStatus};

/// Scans for public items missing doc comments.
pub struct AutoDocumentationWorker;

impl DaemonWorker for AutoDocumentationWorker {
    fn name(&self) -> &str {
        "auto_documentation"
    }

    fn description(&self) -> &str {
        "Scans Rust source files for public items missing documentation"
    }

    fn schedule(&self) -> &str {
        "*/30 * * * *"
    }

    fn execute(&self, ctx: &WorkerContext) -> WorkerResult {
        info!(project = ?ctx.project_root, "AutoDocumentationWorker starting");

        let src_dir = ctx.project_root.join("src");
        if !src_dir.is_dir() {
            return WorkerResult {
                status: WorkerStatus::Success,
                message: "No src/ directory to scan".to_string(),
                items_processed: 0,
            };
        }

        let source_files = collect_source_files(&src_dir);
        let mut undocumented = Vec::new();

        for file_path in &source_files {
            // Only scan Rust files for doc comments.
            let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "rs" {
                continue;
            }

            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let short_path = file_path
                .strip_prefix(&ctx.project_root)
                .unwrap_or(file_path)
                .to_string_lossy();

            let mut prev_line_is_doc = false;
            for (i, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                let is_doc = trimmed.starts_with("///") || trimmed.starts_with("//!");

                // Check if this line declares a public item without a preceding doc comment.
                if !prev_line_is_doc
                    && (trimmed.starts_with("pub fn ")
                        || trimmed.starts_with("pub struct ")
                        || trimmed.starts_with("pub enum ")
                        || trimmed.starts_with("pub trait ")
                        || trimmed.starts_with("pub const ")
                        || trimmed.starts_with("pub type "))
                {
                    let item_name = trimmed
                        .split_whitespace()
                        .nth(2)
                        .unwrap_or("?")
                        .trim_end_matches(|c: char| c == '{' || c == '(' || c == ':')
                        .to_string();
                    undocumented.push(format!("{short_path}:{}: {item_name}", i + 1));
                }

                prev_line_is_doc = is_doc;
            }
        }

        let count = undocumented.len();
        if count > 0 {
            let preview: Vec<String> = undocumented.iter().take(10).cloned().collect();
            WorkerResult {
                status: WorkerStatus::Partial,
                message: format!(
                    "Found {count} undocumented public items (e.g.: {})",
                    preview.join(", ")
                ),
                items_processed: count,
            }
        } else {
            info!("AutoDocumentationWorker: all public items documented");
            WorkerResult {
                status: WorkerStatus::Success,
                message: "All public items have documentation".to_string(),
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
    fn test_auto_documentation_worker_metadata() {
        let worker = AutoDocumentationWorker;
        assert_eq!(worker.name(), "auto_documentation");
        assert_eq!(worker.schedule(), "*/30 * * * *");
        let interval = parse_interval_minutes(worker.schedule());
        assert_eq!(interval, 30);
    }

    #[test]
    fn test_auto_documentation_no_src_dir() {
        let worker = AutoDocumentationWorker;
        let ctx = WorkerContext {
            project_root: PathBuf::from("/tmp/nonexistent_d3vx_doc_test_12345"),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = worker.execute(&ctx);
        assert_eq!(result.status, WorkerStatus::Success);
        assert_eq!(result.items_processed, 0);
    }

    #[test]
    fn test_auto_documentation_detects_undocumented() {
        let worker = AutoDocumentationWorker;
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(src.join("lib.rs"), "pub fn hello() {}\npub struct Foo;\n").expect("write");
        let ctx = WorkerContext {
            project_root: dir.path().to_path_buf(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = worker.execute(&ctx);
        assert_eq!(result.status, WorkerStatus::Partial);
        assert!(result.items_processed > 0);
    }

    #[test]
    fn test_auto_documentation_ignores_documented() {
        let worker = AutoDocumentationWorker;
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(
            src.join("lib.rs"),
            "/// Documented\npub fn hello() {}\n/// Also documented\npub struct Foo;\n",
        )
        .expect("write");
        let ctx = WorkerContext {
            project_root: dir.path().to_path_buf(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = worker.execute(&ctx);
        assert_eq!(result.status, WorkerStatus::Success);
        assert_eq!(result.items_processed, 0);
    }
}
