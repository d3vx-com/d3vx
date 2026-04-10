//! Doctor Command — Environment Diagnostics
//!
//! Validates every prerequisite d3vx needs to run correctly:
//!   config, API key, git, git worktrees, SQLite, disk space, daemon.
//!
//! Each check is isolated so failures in one never hide others.

use anyhow::Result;

use crate::cli::commands::daemon::{process_running, read_daemon_pid};
use crate::cli::commands::helpers::{
    command_version, doctor_status, free_disk_mb, DoctorRuntimeContext,
};
use crate::config::{get_provider_config, load_config, LoadConfigOptions};
use crate::providers::SUPPORTED_PROVIDERS;

/// Minimum free disk space before we emit a warning (MB).
const MIN_DISK_MB: u64 = 512;

// ─────────────────────────────────────────────────────────────────────────────
// Individual checks
// ─────────────────────────────────────────────────────────────────────────────

/// Returns (formatted line, missing_api_key_env_var_name_if_any)
fn check_config() -> (String, Option<String>) {
    match load_config(LoadConfigOptions::default()) {
        Ok(config) => {
            let (_, api_key, _) = get_provider_config(&config);
            let status = if SUPPORTED_PROVIDERS.is_supported(&config.provider) {
                "OK"
            } else {
                "WARN"
            };
            let detail = format!("provider={} model={}", config.provider, config.model);
            let missing_env = if api_key.is_none() {
                SUPPORTED_PROVIDERS
                    .api_key_env(&config.provider)
                    .map(String::from)
            } else {
                None
            };
            (doctor_status("Config", status, detail), missing_env)
        }
        Err(e) => (
            doctor_status("Config", "FAIL", format!("cannot load: {e}")),
            None,
        ),
    }
}

fn check_api_key(missing_env: Option<&str>, provider: &str) -> String {
    // Even if env var is missing, the key might be in the OS keychain
    if missing_env.is_some() && crate::config::keychain::has_key(provider) {
        return doctor_status("API Key", "OK", "stored in OS keychain");
    }
    match missing_env {
        None => doctor_status("API Key", "OK", "found in environment"),
        Some(var) => doctor_status(
            "API Key",
            "FAIL",
            format!("{var} not set  →  run: d3vx setup"),
        ),
    }
}

fn check_git() -> String {
    match command_version("git", "--version") {
        Some(v) => doctor_status("Git", "OK", v),
        None => doctor_status("Git", "FAIL", "git not found — install git first"),
    }
}

fn check_git_repo(cwd: &str) -> String {
    let in_repo = std::process::Command::new("git")
        .args(["-C", cwd, "rev-parse", "--show-toplevel"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if in_repo {
        doctor_status("Git Repo", "OK", cwd)
    } else {
        doctor_status(
            "Git Repo",
            "WARN",
            "not a git repo — --vex tasks require git  →  run: git init",
        )
    }
}

fn check_worktree_support(cwd: &str) -> String {
    let supported = std::process::Command::new("git")
        .args(["-C", cwd, "worktree", "list"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if supported {
        doctor_status("Git Worktrees", "OK", "supported")
    } else {
        doctor_status(
            "Git Worktrees",
            "WARN",
            "not available — upgrade git to >=2.5 for --vex",
        )
    }
}

fn check_disk(cwd: &str) -> String {
    match free_disk_mb(cwd) {
        Some(mb) if mb >= MIN_DISK_MB => doctor_status("Disk Space", "OK", format!("{mb} MB free")),
        Some(mb) => doctor_status(
            "Disk Space",
            "WARN",
            format!("{mb} MB free — at least {MIN_DISK_MB} MB recommended"),
        ),
        None => doctor_status("Disk Space", "INFO", "could not determine"),
    }
}

fn check_sqlite() -> String {
    match crate::store::database::Database::open_default() {
        Ok(_) => doctor_status("SQLite DB", "OK", "task database accessible"),
        Err(e) => doctor_status("SQLite DB", "FAIL", format!("cannot open: {e}")),
    }
}

fn check_github_cli() -> String {
    match command_version("gh", "--version").and_then(|s| s.lines().next().map(String::from)) {
        Some(v) => doctor_status("GitHub CLI", "OK", v),
        None => doctor_status("GitHub CLI", "INFO", "gh not installed (optional)"),
    }
}

fn check_daemon() -> String {
    match read_daemon_pid().ok().flatten() {
        Some(pid) if process_running(pid) => {
            doctor_status("Daemon", "OK", format!("running (pid {pid})"))
        }
        Some(pid) => doctor_status(
            "Daemon",
            "WARN",
            format!("stale pid file (pid {pid}) — run: d3vx daemon start"),
        ),
        None => doctor_status(
            "Daemon",
            "INFO",
            "not running (optional for background tasks)",
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Build the full doctor report as a string. Kept synchronous so it can also
/// be called from TUI status panels without spawning a runtime.
pub(crate) fn build_doctor_report(context: DoctorRuntimeContext) -> Result<String> {
    let cwd = context
        .cwd
        .clone()
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| ".".to_string());

    let mut out = String::from("\n  \x1b[1md3vx doctor\x1b[0m\n\n");

    // Config + API key (linked: config tells us which env var to look for)
    let (config_line, missing_env) = check_config();
    let provider = load_config(LoadConfigOptions::default())
        .map(|c| c.provider.clone())
        .unwrap_or_else(|_| "anthropic".to_string());
    out.push_str(&config_line);
    out.push_str(&check_api_key(missing_env.as_deref(), &provider));

    // Git
    out.push_str(&check_git());
    out.push_str(&check_git_repo(&cwd));
    out.push_str(&check_worktree_support(&cwd));

    // Storage
    out.push_str(&check_disk(&cwd));
    out.push_str(&check_sqlite());

    // Optional tooling
    out.push_str(&check_github_cli());

    // Runtime context injected when called from within a live TUI session
    if let Some(connected) = context.db_connected {
        out.push_str(&doctor_status(
            "TUI DB",
            if connected { "OK" } else { "WARN" },
            if connected {
                "connected"
            } else {
                "not connected"
            },
        ));
    }
    if let Some(initialized) = context.provider_initialized {
        out.push_str(&doctor_status(
            "TUI Provider",
            if initialized { "OK" } else { "WARN" },
            if initialized {
                "initialized"
            } else {
                "not initialized"
            },
        ));
    }

    // Daemon
    out.push_str(&check_daemon());

    out.push('\n');
    Ok(out)
}

pub(crate) async fn execute_doctor() -> Result<()> {
    print!("{}", build_doctor_report(DoctorRuntimeContext::default())?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_has_all_sections() {
        let report = build_doctor_report(DoctorRuntimeContext::default()).unwrap();
        for section in ["Config", "API Key", "Git", "SQLite", "Daemon"] {
            assert!(report.contains(section), "missing section: {section}");
        }
    }

    #[test]
    fn test_api_key_missing_shows_env_var() {
        let line = check_api_key(Some("ANTHROPIC_API_KEY"), "anthropic");
        assert!(line.contains("ANTHROPIC_API_KEY"));
        assert!(line.contains("FAIL") || line.contains('\x1b'));
    }

    #[test]
    fn test_api_key_present_shows_ok() {
        let line = check_api_key(None, "anthropic");
        assert!(line.contains("OK") || line.contains('\x1b'));
    }

    #[test]
    fn test_disk_check_does_not_panic() {
        let _ = check_disk(".");
    }

    #[test]
    fn test_runtime_context_sections_appear() {
        let ctx = DoctorRuntimeContext {
            cwd: Some(".".to_string()),
            db_connected: Some(true),
            provider_initialized: Some(false),
        };
        let report = build_doctor_report(ctx).unwrap();
        assert!(report.contains("TUI DB"));
        assert!(report.contains("TUI Provider"));
    }
}
