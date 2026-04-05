//! Security audit daemon worker.

use tracing::{info, warn};

use super::{collect_source_files, DaemonWorker, WorkerContext, WorkerResult, WorkerStatus};

/// Scans source files for common vulnerability patterns.
pub struct SecurityAuditWorker;

/// Patterns that indicate potential security issues.
const SECRET_PATTERNS: &[&str] = &[
    "password = \"",
    "api_key = \"",
    "secret_key = \"",
    "private_key = \"",
    "token = \"",
    "BEGIN RSA PRIVATE KEY",
    "BEGIN PRIVATE KEY",
    "aws_secret_access_key",
];

const SQL_INJECTION_PATTERNS: &[&str] = &[
    "format!(\"SELECT",
    "format!(\"INSERT",
    "format!(\"UPDATE",
    "format!(\"DELETE",
    "format!(\"DROP",
    "String::from(\"SELECT",
    "String::from(\"INSERT",
    "String::from(\"UPDATE",
    "String::from(\"DELETE",
];

const EVAL_PATTERNS: &[&str] = &["eval(", "exec(", "shell_exec(", "subprocess.call("];

impl DaemonWorker for SecurityAuditWorker {
    fn name(&self) -> &str {
        "security_audit"
    }

    fn description(&self) -> &str {
        "Scans for hardcoded secrets, SQL injection patterns, and eval usage"
    }

    fn schedule(&self) -> &str {
        "*/15 * * * *"
    }

    fn execute(&self, ctx: &WorkerContext) -> WorkerResult {
        info!(project = ?ctx.project_root, "SecurityAuditWorker starting");

        let src_dir = ctx.project_root.join("src");
        if !src_dir.is_dir() {
            return WorkerResult {
                status: WorkerStatus::Success,
                message: "No src/ directory to audit".to_string(),
                items_processed: 0,
            };
        }

        let source_files = collect_source_files(&src_dir);
        let mut findings = Vec::new();

        for file_path in &source_files {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                let short_path = file_path
                    .strip_prefix(&ctx.project_root)
                    .unwrap_or(file_path)
                    .to_string_lossy();

                for line in content.lines().enumerate() {
                    let (line_num, line_text) = line;
                    let lower = line_text.to_lowercase();

                    for pattern in SECRET_PATTERNS {
                        if lower.contains(&pattern.to_lowercase()) {
                            findings.push(format!(
                                "{short_path}:{}: possible hardcoded secret ({pattern})",
                                line_num + 1
                            ));
                        }
                    }

                    for pattern in SQL_INJECTION_PATTERNS {
                        if line_text.contains(pattern) {
                            findings.push(format!(
                                "{short_path}:{}: possible SQL injection ({pattern})",
                                line_num + 1
                            ));
                        }
                    }

                    for pattern in EVAL_PATTERNS {
                        if line_text.contains(pattern) {
                            findings.push(format!(
                                "{short_path}:{}: eval/exec usage ({pattern})",
                                line_num + 1
                            ));
                        }
                    }
                }
            }
        }

        let count = findings.len();
        if count > 0 {
            warn!(findings = count, "SecurityAuditWorker found issues");
            let preview: Vec<String> = findings.iter().take(5).cloned().collect();
            WorkerResult {
                status: WorkerStatus::Partial,
                message: format!("Found {count} potential issues: {}", preview.join("; ")),
                items_processed: count,
            }
        } else {
            info!("SecurityAuditWorker: no issues found");
            WorkerResult {
                status: WorkerStatus::Success,
                message: "No security issues detected".to_string(),
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
    fn test_security_audit_worker_metadata() {
        let worker = SecurityAuditWorker;
        assert_eq!(worker.name(), "security_audit");
        assert_eq!(worker.schedule(), "*/15 * * * *");
        let interval = parse_interval_minutes(worker.schedule());
        assert_eq!(interval, 15);
    }

    #[test]
    fn test_security_audit_no_src_dir() {
        let worker = SecurityAuditWorker;
        let ctx = WorkerContext {
            project_root: PathBuf::from("/tmp/nonexistent_d3vx_sec_test_12345"),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = worker.execute(&ctx);
        assert_eq!(result.status, WorkerStatus::Success);
        assert_eq!(result.items_processed, 0);
    }

    #[test]
    fn test_security_audit_detects_secret() {
        let worker = SecurityAuditWorker;
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        std::fs::write(src.join("config.rs"), "# config: password = \"secret\"\n").expect("write");
        let ctx = WorkerContext {
            project_root: dir.path().to_path_buf(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = worker.execute(&ctx);
        assert_eq!(result.status, WorkerStatus::Partial);
        assert!(result.items_processed > 0);
    }
}
