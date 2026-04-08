//! Vex Mode Handler
//!
//! Handles `d3vx --vex "task description"` — creates a background
//! autonomous task that runs in an isolated tmux session.

use std::process::Stdio;
use std::sync::Arc;

use anyhow::Result;

use crate::cli::args::Cli;
use crate::config::{load_config, LoadConfigOptions};
use crate::pipeline::orchestrator::PipelineOrchestrator;
use crate::pipeline::phases::{PhaseContext, TaskStatus};
use crate::pipeline::scheduler::ExecutionGuard;
use crate::pipeline::WorkerPool;
use tracing::info;

use super::tools;

fn tmux_session_name(task_id: &str) -> String {
    format!("d3vx-{}", task_id)
}

async fn spawn_tmux_session(session_name: &str, command: &str) -> Result<()> {
    let escaped_cmd = command.replace("'", "'\\''");
    let session_arg = format!("'{}'", escaped_cmd);

    tokio::process::Command::new("sh")
        .arg("-c")
        .arg(format!("tmux new-session -d -s {} {}", session_name, session_arg))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to create tmux session: {}", e))?;

    Ok(())
}

pub async fn run_vex_mode(query: &str, cli: &Cli) -> Result<()> {
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

    let project_path = cli
        .cwd
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        });

    let db = crate::store::database::Database::open_default()
        .ok()
        .map(|d| Arc::new(parking_lot::Mutex::new(d)));

    let mut orch_config = crate::pipeline::orchestrator::OrchestratorConfig::default();
    orch_config.checkpoint_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".d3vx/checkpoints");
    orch_config.github = config.integrations.as_ref().and_then(|i| i.github.clone());

    let orchestrator = PipelineOrchestrator::new(orch_config, db).await?;
    let vex_manager = orchestrator.vex_manager();

    let handle = vex_manager.create_task(query, &project_path, None).await?;

    let session = tmux_session_name(&handle.task_id);
    let worktree = handle.worktree_path.to_string_lossy();

    let task_cmd = format!(
        "d3vx task {} --cwd {} --worktree '{}'",
        handle.task_id,
        project_path.replace(" ", "\\ "),
        worktree.replace(" ", "\\ ")
    );

    spawn_tmux_session(&session, &task_cmd).await?;

    info!(
        task_id = %handle.task_id,
        worktree = %worktree,
        session = %session,
        "Vex task spawned in tmux"
    );

    println!();
    println!("  \x1b[1m🚀 Vex task started in tmux\x1b[0m");
    println!();
    println!("  Task ID:    \x1b[33m{}\x1b[0m", handle.task_id);
    println!("  Session:    \x1b[36mtmux attach -t {}\x1b[0m", session);
    println!("  Worktree:   {}", worktree);
    println!();
    println!("  \x1b[90mMonitor progress:\x1b[0m");
    println!("    d3vx status              — view task status");
    println!("    open http://localhost:9876  — open dashboard");
    println!("    tmux attach -t {}         — attach to session", session);
    println!();
    println!("  \x1b[90mCancel task:\x1b[0m");
    println!("    tmux kill-session -t {}   — cancel and cleanup", session);
    println!();

    Ok(())
}

pub async fn run_task_detached(task_id: String, cwd: String, worktree: String) -> Result<()> {
    info!(task_id = %task_id, cwd = %cwd, worktree = %worktree, "Running task in detached mode");

    let db = crate::store::database::Database::open_default()
        .map(|d| Arc::new(parking_lot::Mutex::new(d)))
        .map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))?;

    let mut orch_config = crate::pipeline::orchestrator::OrchestratorConfig::default();
    orch_config.checkpoint_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".d3vx/checkpoints");

    let orchestrator = PipelineOrchestrator::new(orch_config, Some(db.clone())).await?;
    let queue = orchestrator.queue();

    let task = queue.get_task(&task_id).await
        .ok_or_else(|| anyhow::anyhow!("Task {} not found in queue", task_id))?;

    queue.update_status(&task_id, TaskStatus::InProgress).await?;

    let config = load_config(LoadConfigOptions::default())
        .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?;

    let provider = tools::create_provider(&config)?;

    let tools = tools::build_vex_tools().await;

    let db_handle = Some(db.clone());
    let agent = tools::create_vex_agent(&config, provider, tools, &cwd, &task_id, db_handle)?;

    let worktree_pool = Arc::new(WorkerPool::with_defaults());
    let lease = worktree_pool.acquire_worker(&task_id).await
        .map_err(|e| anyhow::anyhow!(e))?;
    let _guard = ExecutionGuard::new(worktree_pool.clone(), task_id.clone(), lease);

    let context = PhaseContext::new(task.clone(), &cwd, &worktree);

    let result = orchestrator.engine().run_with_agent(task.clone(), context, Arc::new(agent)).await
        .map_err(|e| anyhow::anyhow!("Task execution failed: {}", e))?;

    if result.success {
        info!(task_id = %task_id, "Task completed successfully");
        println!();
        println!("  \x1b[1m✅ Task {} completed\x1b[0m", task_id);
        println!();
    } else {
        let error = result.error.unwrap_or_else(|| "Unknown error".to_string());
        info!(task_id = %task_id, error = %error, "Task failed");
        println!();
        println!("  \x1b[1m❌ Task {} failed: {}\x1b[0m", task_id, error);
        println!();
    }

    Ok(())
}
