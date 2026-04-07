//! Daemon Command Implementation
//!
//! Background daemon management for autonomous task processing.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::{load_config, LoadConfigOptions};
use crate::pipeline::{GitHubConfig, OrchestratorConfig, PipelineOrchestrator};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DaemonState {
    pub pid: u32,
    pub started_at: String,
    pub last_heartbeat: String,
    pub recovered_tasks: usize,
    pub last_recovery_error: Option<String>,
    pub github_polling: bool,
    pub dispatch_iterations: u64,
    pub queue_total: usize,
    pub queue_queued: usize,
    pub queue_in_progress: usize,
    pub queue_failed: usize,
    pub active_tasks: usize,
    pub last_dispatch_error: Option<String>,
}

pub(crate) fn daemon_dir() -> PathBuf {
    PathBuf::from(crate::config::get_global_config_dir())
}

pub(crate) fn daemon_pid_path() -> PathBuf {
    daemon_dir().join("daemon.pid")
}

pub(crate) fn daemon_state_path() -> PathBuf {
    daemon_dir().join("daemon-status.json")
}

pub(crate) fn daemon_log_path() -> PathBuf {
    daemon_dir().join("d3vx.log")
}

pub(crate) fn write_daemon_state(state: &DaemonState) -> Result<()> {
    let dir = daemon_dir();
    fs::create_dir_all(&dir)?;
    fs::write(daemon_state_path(), serde_json::to_vec_pretty(state)?)?;
    fs::write(daemon_pid_path(), state.pid.to_string())?;
    Ok(())
}

pub(crate) fn clear_daemon_state() {
    let _ = fs::remove_file(daemon_pid_path());
    let _ = fs::remove_file(daemon_state_path());
}

pub(crate) fn read_daemon_pid() -> Result<Option<i32>> {
    let path = daemon_pid_path();
    if !path.exists() {
        return Ok(None);
    }
    let pid = fs::read_to_string(path)?.trim().parse::<i32>()?;
    Ok(Some(pid))
}

#[cfg(unix)]
pub(crate) fn process_running(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(not(unix))]
pub(crate) fn process_running(_pid: i32) -> bool {
    false
}

#[cfg(unix)]
pub(crate) fn signal_process(pid: i32, signal: i32) -> Result<()> {
    let rc = unsafe { libc::kill(pid, signal) };
    if rc != 0 {
        return Err(anyhow::anyhow!("failed to signal pid {}", pid));
    }
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn signal_process(_pid: i32, _signal: i32) -> Result<()> {
    anyhow::bail!("daemon signaling is only implemented on unix")
}

pub(crate) async fn start_daemon_detached() -> Result<()> {
    if let Some(pid) = read_daemon_pid()? {
        if process_running(pid) {
            anyhow::bail!("daemon already running with pid {}", pid);
        }
        clear_daemon_state();
    }

    let exe = std::env::current_exe()?;
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(daemon_log_path())?;
    let err_log = log.try_clone()?;

    let child = std::process::Command::new(exe)
        .arg("daemon")
        .arg("start")
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err_log))
        .spawn()?;

    println!("Daemon started in background with pid {}", child.id());
    Ok(())
}

pub(crate) async fn run_daemon_foreground() -> Result<()> {
    if let Some(pid) = read_daemon_pid()? {
        if process_running(pid) {
            anyhow::bail!("daemon already running with pid {}", pid);
        }
        clear_daemon_state();
    }

    let config = load_config(LoadConfigOptions::default())?;
    let mut orch_config = OrchestratorConfig::default();
    orch_config.checkpoint_dir =
        PathBuf::from(crate::config::get_global_config_dir()).join("checkpoints");
    orch_config.github = config.integrations.as_ref().and_then(|i| i.github.clone());
    let orchestrator = PipelineOrchestrator::new(orch_config, None).await?;

    let (recovered, last_recovery_error) = match orchestrator.recover_interrupted_tasks().await {
        Ok(tasks) => (tasks, None),
        Err(error) => {
            eprintln!("daemon recovery failed: {}", error);
            (Vec::new(), Some(error.to_string()))
        }
    };
    let started_at = Utc::now().to_rfc3339();

    let github_enabled =
        if let Some(github) = config.integrations.as_ref().and_then(|i| i.github.clone()) {
            if let Some(repository) = github.repository.clone() {
                orchestrator
                    .start_github_poller(GitHubConfig {
                        repositories: vec![repository],
                        trigger_labels: vec!["d3vx".to_string()],
                        auto_process_labels: vec!["d3vx-auto".to_string()],
                        poll_interval_secs: 300,
                        webhook_secret: None,
                        sync_status: true,
                        token_env: github.token_env,
                        api_base_url: github.api_base_url,
                    })
                    .await?;
                true
            } else {
                false
            }
        } else {
            false
        };

    let pid = std::process::id();
    let max_parallel = config.pipeline.max_concurrent_agents as usize;
    let mut dispatch_iterations = 0u64;
    let mut last_dispatch_error: Option<String> = None;

    loop {
        dispatch_iterations += 1;
        let queue = orchestrator.queue_stats().await;
        let active = orchestrator.active_tasks_list().await;
        let state = DaemonState {
            pid,
            started_at: started_at.clone(),
            last_heartbeat: Utc::now().to_rfc3339(),
            recovered_tasks: recovered.len(),
            last_recovery_error: last_recovery_error.clone(),
            github_polling: github_enabled,
            dispatch_iterations,
            queue_total: queue.total,
            queue_queued: queue.queued,
            queue_in_progress: queue.in_progress,
            queue_failed: queue.failed,
            active_tasks: active.len(),
            last_dispatch_error: last_dispatch_error.clone(),
        };
        let _ = write_daemon_state(&state);

        match orchestrator.dispatch_tasks_parallel(max_parallel).await {
            Ok(results) => {
                last_dispatch_error = None;
                // Run reaction engine on failed tasks (re-queue, cancel, escalate).
                orchestrator.post_process_results(&results).await;
                // Refresh all tracked PRs (CI, reviews, mergeability).
                orchestrator.audit_active_prs().await;
            }
            Err(error) => {
                eprintln!("daemon dispatch failed: {}", error);
                last_dispatch_error = Some(error.to_string());
            }
        }

        let shutdown = tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(5)) => false,
            _ = tokio::signal::ctrl_c() => true,
            _ = wait_for_sigterm() => true,
        };

        if shutdown {
            clear_daemon_state();
            return Ok(());
        }
    }
}

#[cfg(unix)]
async fn wait_for_sigterm() {
    use tokio::signal::unix::{signal, SignalKind};
    if let Ok(mut stream) = signal(SignalKind::terminate()) {
        let _ = stream.recv().await;
    }
}

#[cfg(not(unix))]
async fn wait_for_sigterm() {
    futures::future::pending::<()>().await;
}

pub(crate) async fn stop_daemon(force: bool) -> Result<()> {
    let Some(pid) = read_daemon_pid()? else {
        println!("Daemon is not running.");
        return Ok(());
    };

    if !process_running(pid) {
        clear_daemon_state();
        println!("Removed stale daemon state.");
        return Ok(());
    }

    #[cfg(unix)]
    {
        signal_process(pid, libc::SIGTERM)?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        if process_running(pid) {
            if force {
                signal_process(pid, libc::SIGKILL)?;
            } else {
                anyhow::bail!("daemon did not stop gracefully; retry with --force");
            }
        }
    }

    clear_daemon_state();
    println!("Daemon stopped.");
    Ok(())
}

pub(crate) async fn daemon_status() -> Result<()> {
    let pid = read_daemon_pid()?;
    let state_path = daemon_state_path();
    if pid.is_none() || !state_path.exists() {
        println!("Daemon status: stopped");
        return Ok(());
    }

    let state: DaemonState = serde_json::from_slice(&fs::read(state_path)?)?;
    let running = process_running(state.pid as i32);
    println!(
        "Daemon status: {}",
        if running { "running" } else { "stale" }
    );
    println!("PID: {}", state.pid);
    println!("Last heartbeat: {}", state.last_heartbeat);
    println!("Recovered tasks on startup: {}", state.recovered_tasks);
    if let Some(error) = state.last_recovery_error {
        println!("Last recovery error: {}", error);
    }
    println!("Dispatch iterations: {}", state.dispatch_iterations);
    println!(
        "Queue: total={} queued={} running={} failed={}",
        state.queue_total, state.queue_queued, state.queue_in_progress, state.queue_failed
    );
    println!("Active tasks: {}", state.active_tasks);
    println!(
        "GitHub polling: {}",
        if state.github_polling {
            "enabled"
        } else {
            "disabled"
        }
    );
    if let Some(error) = state.last_dispatch_error {
        println!("Last dispatch error: {}", error);
    }
    Ok(())
}

pub(crate) async fn daemon_logs(follow: bool, lines: Option<usize>) -> Result<()> {
    let log_path = daemon_log_path();
    if !log_path.exists() {
        anyhow::bail!("daemon log not found at {}", log_path.display());
    }

    let lines = lines.unwrap_or(50);
    let content = fs::read_to_string(&log_path)?;
    let collected: Vec<&str> = content.lines().collect();
    let start = collected.len().saturating_sub(lines);
    for line in &collected[start..] {
        println!("{}", line);
    }

    if follow {
        let file = File::open(&log_path)?;
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::End(0))?;
        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line)?;
            if bytes > 0 {
                print!("{}", line);
            } else {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    Ok(())
}
