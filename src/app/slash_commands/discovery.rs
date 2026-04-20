//! Discoverability slash commands — `/dashboard`, `/daemon`, `/vex list`.
//!
//! These aren't new capabilities; they surface features that already
//! exist (the Axum dashboard, the background daemon, running vex
//! tasks) so a user doesn't have to remember the URL that was printed
//! to stdout once at startup, run a separate CLI to check the daemon,
//! or guess what's running in the background.
//!
//! Each handler is intentionally terse: look up the state we already
//! have, render a single system message with the salient facts, and
//! (for the dashboard) best-effort-open the browser. No new state
//! fields, no background polling.
//!
//! Output style: short, dense, one-line-per-fact. Claude-Code-like —
//! the user reads it, makes a decision, moves on.

use anyhow::Result;

use crate::app::state::NotificationType;
use crate::app::App;
use crate::utils::open_url::open_url;

/// True if the background daemon is currently running — checks pidfile
/// + signals the pid with 0 to confirm the process actually exists.
///
/// Lives here rather than on `App` because the daemon's existence is
/// process-level state, not UI state; the filesystem is the source of
/// truth. Cheap enough to call every render tick for the status strip.
pub fn daemon_is_running() -> bool {
    use crate::cli::commands::daemon::{process_running, read_daemon_pid};
    match read_daemon_pid() {
        Ok(Some(pid)) => process_running(pid),
        _ => false,
    }
}

/// `/dashboard` — print the dashboard URL and open it in the browser.
///
/// If no dashboard is running (`--no-dashboard`, daemon-only launch),
/// the user gets a clear message explaining why, not a silent no-op.
pub fn handle_dashboard(app: &mut App, _args: &[&str]) -> Result<()> {
    let url = match app.dashboard.as_ref() {
        Some(d) => d.url(),
        None => {
            app.add_system_message(
                "Dashboard isn't running in this session. Re-launch without `--no-dashboard` to enable it.",
            );
            return Ok(());
        }
    };

    match open_url(&url) {
        Ok(()) => {
            app.add_notification(
                format!("Dashboard opened: {url}"),
                NotificationType::Success,
            );
            app.add_system_message(&format!("Dashboard: {url} (opened in your browser)"));
        }
        Err(e) => {
            // Browser-opener failed — the URL is still valid, the user
            // can click or copy it manually. Don't downgrade to an
            // error toast; the dashboard itself is fine.
            app.add_system_message(&format!(
                "Dashboard: {url}\n(couldn't auto-open browser: {e})"
            ));
        }
    }
    Ok(())
}

/// `/daemon` — show whether the background daemon is alive and what
/// it's working on. Reads `~/.d3vx/daemon-status.json`, which the
/// daemon writes on every dispatch cycle.
pub fn handle_daemon(app: &mut App, _args: &[&str]) -> Result<()> {
    let msg = daemon_status_summary();
    app.add_system_message(&msg);
    Ok(())
}

/// Build the multi-line daemon-status summary. Factored out so it's
/// unit-testable without needing an `App`.
fn daemon_status_summary() -> String {
    use crate::cli::commands::daemon::{daemon_state_path, read_daemon_pid};

    let pid_state = read_daemon_pid().ok().flatten();
    let path = daemon_state_path();
    let raw = std::fs::read_to_string(&path);

    match (pid_state, raw) {
        (Some(pid), Ok(json)) => {
            match serde_json::from_str::<serde_json::Value>(&json) {
                Ok(v) => format_daemon_json(pid, &v),
                Err(_) => format!("Daemon pid {pid} running but status file is unreadable."),
            }
        }
        (Some(pid), Err(_)) => format!(
            "Daemon pid {pid} has a stale pidfile — no status yet. Run `d3vx daemon status` for details."
        ),
        (None, _) => "Daemon is not running. Start it with `d3vx daemon start --detach`.".into(),
    }
}

fn format_daemon_json(pid: i32, v: &serde_json::Value) -> String {
    let started = v.get("started_at").and_then(|s| s.as_str()).unwrap_or("?");
    let heartbeat = v.get("last_heartbeat").and_then(|s| s.as_str()).unwrap_or("?");
    let queued = v.get("queue_queued").and_then(|n| n.as_u64()).unwrap_or(0);
    let in_progress = v
        .get("queue_in_progress")
        .and_then(|n| n.as_u64())
        .unwrap_or(0);
    let failed = v.get("queue_failed").and_then(|n| n.as_u64()).unwrap_or(0);
    let active = v.get("active_tasks").and_then(|n| n.as_u64()).unwrap_or(0);
    let last_err = v
        .get("last_dispatch_error")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty());

    let mut out = format!(
        "Daemon pid {pid}\n\
         started: {started}\n\
         heartbeat: {heartbeat}\n\
         active: {active}  queued: {queued}  in-progress: {in_progress}  failed: {failed}"
    );
    if let Some(err) = last_err {
        out.push_str(&format!("\nlast error: {err}"));
    }
    out
}

/// `/vex list` — show background tasks the TUI knows about. Called
/// from the existing `/vex` handler when `args[0] == "list"`, so a
/// user can discover this by typing `/vex` with no args and seeing
/// the usage hint.
pub fn handle_vex_list(app: &mut App) -> Result<()> {
    // Refresh from the orchestrator on demand. The background poll
    // runs every 500ms, so the cached list is usually fresh — but a
    // user typing `/vex list` right after `/vex <task>` expects to
    // see the task *now*, not in half a second.
    app.background_active_tasks = tokio::task::block_in_place(|| {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(app.orchestrator.active_tasks_list())
    });

    if app.background_active_tasks.is_empty() {
        if daemon_is_running() {
            app.add_system_message(
                "No background tasks running. Start one with `/vex <description>`.",
            );
        } else {
            app.add_system_message(
                "No background tasks running — and the daemon is offline. Start it with `d3vx daemon start --detach`, then `/vex <description>`.",
            );
        }
        return Ok(());
    }

    let mut out = format!(
        "Background tasks ({}):\n",
        app.background_active_tasks.len()
    );
    for (id, name) in &app.background_active_tasks {
        let short_id = if id.len() >= 8 { &id[..8] } else { id.as_str() };
        out.push_str(&format!("  [{short_id}] {name}\n"));
    }
    if !daemon_is_running() {
        out.push_str(
            "\n⚠ Daemon is not running — these tasks are queued but idle.\n  Start: d3vx daemon start --detach",
        );
    }
    app.add_system_message(&out);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_summary_formats_full_status_block() {
        let raw = serde_json::json!({
            "started_at": "2026-04-20T10:00:00Z",
            "last_heartbeat": "2026-04-20T10:00:05Z",
            "queue_queued": 3,
            "queue_in_progress": 1,
            "queue_failed": 0,
            "active_tasks": 2,
            "last_dispatch_error": null,
        });
        let s = format_daemon_json(42, &raw);
        assert!(s.contains("Daemon pid 42"));
        assert!(s.contains("active: 2"));
        assert!(s.contains("queued: 3"));
        assert!(s.contains("in-progress: 1"));
        assert!(!s.contains("last error"), "should not show empty error");
    }

    #[test]
    fn daemon_summary_surfaces_last_dispatch_error() {
        let raw = serde_json::json!({
            "started_at": "t",
            "last_heartbeat": "t",
            "queue_queued": 0,
            "queue_in_progress": 0,
            "queue_failed": 1,
            "active_tasks": 0,
            "last_dispatch_error": "lost connection to orchestrator",
        });
        let s = format_daemon_json(1, &raw);
        assert!(s.contains("last error: lost connection"));
    }

    #[test]
    fn daemon_summary_handles_missing_fields_gracefully() {
        // A partially-written status file shouldn't panic.
        let raw = serde_json::json!({ "started_at": "t" });
        let s = format_daemon_json(5, &raw);
        assert!(s.contains("Daemon pid 5"));
        assert!(s.contains("active: 0"));
    }
}
