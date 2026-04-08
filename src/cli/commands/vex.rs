//! Vex Mode Handler
//!
//! Handles `d3vx --vex "task description"` — creates a background
//! autonomous task that runs in an isolated worktree.

use std::sync::Arc;

use anyhow::Result;

use crate::cli::args::Cli;
use crate::config::{load_config, LoadConfigOptions};
use crate::pipeline::orchestrator::PipelineOrchestrator;
use crate::pipeline::vex_manager::VexManager;
use tracing::info;

/// Run a task in Vex mode (background, isolated worktree).
///
/// This creates a new Vex task and dispatches it immediately, returning
/// the task ID so the user can monitor progress.
pub async fn run_vex_mode(query: &str, cli: &Cli) -> Result<()> {
    // Load config
    let config = match load_config(LoadConfigOptions {
        project_root: cli.cwd.as_ref().map(|p| p.to_string_lossy().to_string()),
        ..Default::default()
    }) {
        Ok(cfg) => cfg,
        Err(e) => {
            anyhow::bail!(
                "Failed to load config: {}. Run `d3vx setup` first.",
                e
            );
        }
    };

    // Get project path
    let project_path = cli
        .cwd
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string()));

    // Initialize database for task persistence
    let db = crate::store::database::Database::open_default()
        .ok()
        .map(|d| Arc::new(parking_lot::Mutex::new(d)));

    // Create orchestrator with config-backed settings
    let mut orch_config = crate::pipeline::orchestrator::OrchestratorConfig::default();
    orch_config.checkpoint_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".d3vx/checkpoints");
    orch_config.github = config.integrations.as_ref().and_then(|i| i.github.clone());

    let orchestrator = PipelineOrchestrator::new(orch_config, db).await?;

    let vex_manager = orchestrator.vex_manager();

    // Create and dispatch the Vex task
    let handle = vex_manager.create_task(query, &project_path, None).await?;
    vex_manager.dispatch_task(&handle).await?;

    info!(
        task_id = %handle.task_id,
        worktree = %handle.worktree_path.display(),
        "Vex task started"
    );

    println!();
    println!("  \x1b[1m🚀 Vex task started\x1b[0m");
    println!();
    println!("  Task ID:    \x1b[33m{}\x1b[0m", handle.task_id);
    println!("  Worktree:   {}", handle.worktree_path.display());
    println!();
    println!("  \x1b[90mMonitor progress:\x1b[0m");
    println!("    d3vx status           — view task status");
    println!("    open http://localhost:9876  — open dashboard");
    println!();
    println!("  \x1b[90mCancel task:\x1b[0m");
    println!("    d3vx task cancel {}", handle.task_id);
    println!();

    Ok(())
}
