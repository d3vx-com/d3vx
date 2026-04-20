//! One-shot and Interactive Command Implementations
//!
//! Launch modes: single-query processing and full interactive TUI session.

use anyhow::Result;
use std::path::PathBuf;

use crate::config::{
    defaults::default_config, get_provider_config, load_config,
    onboarding::check_onboarding_status, LoadConfigOptions,
};
use crate::pipeline::dashboard::{Dashboard, DashboardConfig};
use crate::ui::runner::{run_tui, TuiOptions};

use crate::cli::args::Cli;

fn cwd_string(cwd: &Option<PathBuf>) -> Option<String> {
    cwd.as_ref().map(|p| p.to_string_lossy().to_string())
}

/// Detect first run or missing API key and offer interactive setup.
/// Returns `true` if the user should proceed to TUI.
async fn handle_auto_setup_if_needed() -> bool {
    let status = check_onboarding_status();

    // Nothing to warn about — has an API key or has config with a key
    if !status.is_first_run && !status.needs_api_key_setup {
        return true;
    }

    if status.is_first_run {
        println!();
        println!("  \x1b[1mWelcome to d3vx!\x1b[0m The autonomous software engineering CLI.\n");
        println!(
            "  \x1b[90mNo config found at {}\x1b[0m",
            crate::config::defaults::get_global_config_path()
        );
    } else if status.needs_api_key_setup {
        println!();
        println!("  \x1b[1mAPI key not configured\x1b[0m for the current provider.\n");
        println!(
            "  Your config at {} exists but the API key is missing.",
            crate::config::defaults::get_global_config_path()
        );
        println!(
            "  Provider: \x1b[1m{}\x1b[0m, expected env: \x1b[1m{}\x1b[0m",
            status.missing_provider.as_deref().unwrap_or("unknown"),
            status.provider_api_key_env
        );
    }

    println!("  {}\n", "─".repeat(50));
    println!("  \x1b[90mChoose an option:\x1b[0m\n");
    println!("    1. \x1b[1mRun setup wizard (recommended)\x1b[0m — interactive provider + model selection");
    println!("    2. \x1b[1mSkip — configure later via \x1b[33m`d3vx setup`\x1b[0m\n");
    print!("  Run setup? [Y/n]: ");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return true;
    }

    match input.trim().to_lowercase().as_str() {
        "n" | "no" => {
            show_skip_hint();
            true
        }
        _ => {
            // Delegate to the authoritative setup wizard
            if let Err(e) = super::setup::execute_setup(None).await {
                eprintln!("  \x1b[31mSetup error: {}\x1b[0m", e);
                return false;
            }
            false
        }
    }
}

/// Ensure the background daemon is running — auto-start it detached
/// if not. Idempotent: silent no-op if the daemon is already up.
///
/// Why: without a daemon, vex tasks queued from the TUI freeze the
/// moment the TUI exits (in-memory orchestrator dies with the
/// process; SQLite records remain but no one dispatches them). The
/// daemon is a separate OS process that owns the dispatch loop, so
/// this closes the silent data-loss loophole where a user types
/// `/vex "build X"`, quits, and comes back to a task that never ran.
///
/// Opt-out: `--no-daemon`. Intentionally not a config key — the
/// desired default *is* "on", and the flag exists only for the
/// transient "I'm just poking around" case.
async fn ensure_daemon_running() {
    use crate::cli::commands::daemon::{
        process_running, read_daemon_pid, start_daemon_detached,
    };

    // Already running → nothing to do, stay silent.
    if let Ok(Some(pid)) = read_daemon_pid() {
        if process_running(pid) {
            return;
        }
    }

    // Not running — attempt to spawn. This is async because the
    // underlying helper is async-signatured (though the body just
    // spawns an OS child); we await it directly rather than using
    // `Handle::current().block_on(...)`, which would panic from
    // inside the tokio runtime that the caller is already on.
    match start_daemon_detached().await {
        Ok(()) => {} // `start_daemon_detached` already prints its own line
        Err(e) => {
            eprintln!(
                "  \x1b[33m! Daemon auto-start failed: {e}\x1b[0m\n  \x1b[90m  (background vex tasks will not survive TUI exit; run `d3vx daemon start --detach` manually)\x1b[0m"
            );
        }
    }
}

/// Try to start the dashboard server.
/// Returns Some(Dashboard) on success, None if it fails (non-fatal).
fn try_start_dashboard() -> Option<Dashboard> {
    let db_result = crate::store::Database::open_default()
        .map_err(|e| tracing::warn!("Dashboard: could not open database: {}", e));
    let db = match db_result {
        Ok(db) => std::sync::Arc::new(parking_lot::Mutex::new(db)),
        Err(_) => return None,
    };

    let dashboard = Dashboard::new(
        DashboardConfig {
            enabled: true,
            ..Default::default()
        },
        db.clone(),
    );

    let url = dashboard.url();
    let dashboard_clone = dashboard.clone();
    tokio::spawn(async move {
        if let Err(e) = dashboard_clone.serve().await {
            tracing::warn!("Dashboard server error: {}", e);
        }
    });

    println!("  Dashboard available at \x1b[4m{}\x1b[0m", url);
    Some(dashboard)
}

fn show_skip_hint() {
    println!("\n  \x1b[90mSkipping setup. You'll need to configure d3vx before use:\x1b[0m");
    if !check_onboarding_status().provider_api_key_env.is_empty() {
        println!(
            "    export {}=\"your-key-here\"",
            check_onboarding_status().provider_api_key_env
        );
    }
    println!("    d3vx setup");
    println!("    d3vx doctor\x1b[90m          — check your environment\x1b[0m");
    println!();
}

pub(crate) async fn execute_oneshot(query: &str, cli: &Cli) -> Result<()> {
    // Auto‑detect missing setup before anything else
    let should_proceed = handle_auto_setup_if_needed().await;
    if !should_proceed {
        return Ok(());
    }

    let config_result = load_config(LoadConfigOptions {
        project_root: cwd_string(&cli.cwd),
        ..Default::default()
    });

    let (mut config, resolved_model) = match config_result {
        Ok(cfg) => {
            let (model, _, _) = get_provider_config(&cfg);
            let final_model = cli.model.clone().unwrap_or(model);
            (Some(cfg), Some(final_model))
        }
        Err(_) => (None, cli.model.clone()),
    };

    if cli.bypass_permissions || cli.trust {
        if let Some(ref mut c) = config {
            c.permissions.trust_mode = true;
        } else {
            let mut default_cfg = default_config();
            default_cfg.permissions.trust_mode = true;
            config = Some(default_cfg);
        }
    }

    if !cli.no_daemon {
        ensure_daemon_running().await;
    }
    let dashboard = try_start_dashboard();

    let tui_opts = TuiOptions {
        verbose: cli.verbose,
        cwd: cwd_string(&cli.cwd),
        model: resolved_model,
        session_id: cli.session.clone(),
        ui_mode: cli.ui.clone(),
        stream_out: cli.stream_out.clone(),
        config,
        dashboard,
        resume: cli.resume,
    };

    println!("Processing query: {}", query);
    run_tui(tui_opts).await
}

pub(crate) async fn execute_interactive(cli: &Cli) -> Result<()> {
    // Auto‑detect missing setup before anything else
    let should_proceed = handle_auto_setup_if_needed().await;
    if !should_proceed {
        return Ok(());
    }

    let config_result = load_config(LoadConfigOptions {
        project_root: cwd_string(&cli.cwd),
        ..Default::default()
    });

    let (mut config, resolved_model) = match config_result {
        Ok(cfg) => {
            let (model, _, _) = get_provider_config(&cfg);
            let final_model = cli.model.clone().unwrap_or(model);
            (Some(cfg), Some(final_model))
        }
        Err(_) => (None, cli.model.clone()),
    };

    if cli.bypass_permissions || cli.trust {
        if let Some(ref mut c) = config {
            c.permissions.trust_mode = true;
        } else {
            let mut default_cfg = default_config();
            default_cfg.permissions.trust_mode = true;
            config = Some(default_cfg);
        }
    }

    if !cli.no_daemon {
        ensure_daemon_running().await;
    }
    let dashboard = try_start_dashboard();

    let tui_opts = TuiOptions {
        verbose: cli.verbose,
        cwd: cwd_string(&cli.cwd),
        model: resolved_model,
        session_id: cli.session.clone(),
        ui_mode: cli.ui.clone(),
        stream_out: cli.stream_out.clone(),
        config,
        dashboard,
        resume: cli.resume,
    };

    run_tui(tui_opts).await
}
