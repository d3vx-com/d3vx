//! CLI Command Execution
//!
//! This module handles command routing and execution logic.
//! It maps CLI arguments to their respective handlers.

mod config_cmd;
mod daemon;
mod doctor;
pub mod error;
mod helpers;
mod oneshot;
mod setup;
mod status;
mod stubs;
mod worktree;

// Re-export types needed by other modules
pub use error::AppError;
pub use helpers::{free_disk_mb, DoctorRuntimeContext};

use anyhow::Result;

use crate::cli::args::{Cli, CliCommand, DaemonAction};

pub(crate) use doctor::build_doctor_report;

/// Execute a CLI command
pub async fn execute(cli: Cli) -> Result<()> {
    // Handle global options first
    if cli.verbose {
        // Set log level to debug
        std::env::set_var("RUST_LOG", "debug");
    }

    // Route to appropriate handler
    match &cli.command {
        Some(cmd) => execute_command(cmd, &cli).await,
        None => {
            // No subcommand - run interactive chat or one-shot query
            if let Some(query) = &cli.query {
                oneshot::execute_oneshot(query, &cli).await
            } else {
                oneshot::execute_interactive(&cli).await
            }
        }
    }
}

/// Execute a specific subcommand
async fn execute_command(cmd: &CliCommand, _cli: &Cli) -> Result<()> {
    match cmd {
        CliCommand::Init { path } => setup::execute_init(path.as_ref()).await,
        CliCommand::Setup { provider } => setup::execute_setup(provider.as_deref()).await,
        CliCommand::Doctor => doctor::execute_doctor().await,
        CliCommand::Notify { action } => stubs::execute_notify(action).await,
        CliCommand::Config { action, key, value } => {
            config_cmd::execute_config(action, key.as_deref(), value.as_deref()).await
        }
        CliCommand::Pricing { action, model } => {
            config_cmd::execute_pricing(action, model.as_deref()).await
        }
        CliCommand::Status => status::execute_status().await,
        CliCommand::Implement {
            instruction,
            fast,
            quick,
            role,
            queue,
        } => stubs::execute_implement(instruction, *fast, *quick, role.as_deref(), *queue).await,
        CliCommand::Worktree {
            list,
            review,
            merge,
            discard,
            recover,
        } => {
            worktree::execute_worktree(
                *list,
                review.as_deref(),
                merge.as_deref(),
                discard.as_deref(),
                *recover,
            )
            .await
        }
        CliCommand::Resume { task_id } => stubs::execute_resume(task_id.as_deref()).await,
        CliCommand::Hooks { action, args } => stubs::execute_hooks(action, args).await,
        CliCommand::Memory { action } => stubs::execute_memory(action).await,
        CliCommand::Daemon { action } => execute_daemon(action).await,
        CliCommand::Spawn { action } => stubs::execute_spawn(action).await,
        CliCommand::Batch { action } => stubs::execute_batch(action).await,
        CliCommand::Docs { action } => stubs::execute_docs(action).await,
        CliCommand::Autonomous { action } => stubs::execute_autonomous(action).await,
    }
}

async fn execute_daemon(action: &DaemonAction) -> Result<()> {
    use daemon::{
        daemon_logs, daemon_status, run_daemon_foreground, start_daemon_detached, stop_daemon,
    };

    match action {
        DaemonAction::Start { detach } => {
            if *detach {
                start_daemon_detached().await
            } else {
                run_daemon_foreground().await
            }
        }
        DaemonAction::Stop { force } => stop_daemon(*force).await,
        DaemonAction::Status => daemon_status().await,
        DaemonAction::Logs { follow, lines } => daemon_logs(*follow, *lines).await,
        DaemonAction::Restart { detach } => {
            let _ = stop_daemon(false).await;
            if *detach {
                start_daemon_detached().await
            } else {
                run_daemon_foreground().await
            }
        }
    }
}
