//! Doctor Command Implementation
//!
//! Environment diagnostics and health check reporting.

use anyhow::Result;

use crate::cli::commands::daemon::{process_running, read_daemon_pid};
use crate::cli::commands::helpers::{command_version, doctor_status, DoctorRuntimeContext};
use crate::config::{get_provider_config, load_config, LoadConfigOptions};
use crate::providers::SUPPORTED_PROVIDERS;

pub(crate) fn build_doctor_report(context: DoctorRuntimeContext) -> Result<String> {
    let mut report = String::from("d3vx Doctor\n\n");

    let config_result = load_config(LoadConfigOptions::default());
    match &config_result {
        Ok(config) => {
            let provider = &config.provider;
            let provider_support = if SUPPORTED_PROVIDERS.is_supported(provider) {
                "OK"
            } else {
                "WARN"
            };
            let provider_detail = if SUPPORTED_PROVIDERS.is_supported(provider) {
                format!("provider={} model={}", provider, config.model)
            } else {
                format!(
                    "provider={} (UNSUPPORTED - available: {})",
                    provider,
                    SUPPORTED_PROVIDERS
                        .ids()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            report.push_str(&doctor_status("Config", provider_support, provider_detail));

            let (_, api_key, base_url) = get_provider_config(config);
            if api_key.is_some() {
                report.push_str(&doctor_status("Provider Key", "OK", "API key available"));
            } else {
                report.push_str(&doctor_status(
                    "Provider Key",
                    "WARN",
                    "No provider API key configured",
                ));
            }
            if let Some(url) = base_url {
                report.push_str(&doctor_status("Base URL", "INFO", url));
            }
            if let Some(github) = config.integrations.as_ref().and_then(|i| i.github.as_ref()) {
                let token_present = std::env::var(&github.token_env).is_ok();
                let detail = format!(
                    "repo={} token_env={} token_present={}",
                    github.repository.as_deref().unwrap_or("-"),
                    github.token_env,
                    if token_present { "yes" } else { "no" }
                );
                report.push_str(&doctor_status(
                    "GitHub",
                    if token_present { "OK" } else { "WARN" },
                    detail,
                ));
            }
        }
        Err(error) => {
            let detail: String = error.to_string();
            report.push_str(&doctor_status("Config", "FAIL", detail));
        }
    }

    if let Some(version) = command_version("git", "--version") {
        report.push_str(&doctor_status("Git", "OK", version));
    } else {
        report.push_str(&doctor_status("Git", "FAIL", "git not found"));
    }

    if let Some(version) = command_version("rustc", "--version") {
        report.push_str(&doctor_status("Rust", "OK", version));
    } else {
        report.push_str(&doctor_status("Rust", "WARN", "rustc not found"));
    }

    if let Some(version) = command_version("gh", "--version")
        .and_then(|out| out.lines().next().map(|line| line.to_string()))
    {
        report.push_str(&doctor_status("GitHub CLI", "OK", version));
    } else {
        report.push_str(&doctor_status("GitHub CLI", "WARN", "gh not installed"));
    }

    let cwd = context
        .cwd
        .clone()
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| ".".to_string());
    report.push_str(&doctor_status("Workspace", "INFO", &cwd));

    let git_root = std::process::Command::new("git")
        .arg("-C")
        .arg(&cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string());
    if let Some(root) = git_root {
        report.push_str(&doctor_status("Repo", "OK", root));
    } else {
        report.push_str(&doctor_status(
            "Repo",
            "WARN",
            "current workspace is not a git repository",
        ));
    }

    match crate::store::database::Database::open_default() {
        Ok(_) => report.push_str(&doctor_status("SQLite", "OK", "task database opened")),
        Err(error) => report.push_str(&doctor_status("SQLite", "FAIL", error.to_string())),
    }

    if let Some(connected) = context.db_connected {
        report.push_str(&doctor_status(
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
        report.push_str(&doctor_status(
            "TUI Provider",
            if initialized { "OK" } else { "WARN" },
            if initialized {
                "initialized"
            } else {
                "not initialized"
            },
        ));
    }

    let daemon_state = read_daemon_pid().ok().flatten();
    match daemon_state {
        Some(pid) if process_running(pid) => {
            report.push_str(&doctor_status(
                "Daemon",
                "OK",
                format!("running pid={}", pid),
            ));
        }
        Some(pid) => {
            report.push_str(&doctor_status(
                "Daemon",
                "WARN",
                format!("stale pid file pid={}", pid),
            ));
        }
        None => {
            report.push_str(&doctor_status("Daemon", "INFO", "not running"));
        }
    }

    Ok(report)
}

pub(crate) async fn execute_doctor() -> Result<()> {
    println!("{}", build_doctor_report(DoctorRuntimeContext::default())?);
    Ok(())
}
