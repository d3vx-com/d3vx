//! CLI Command Helpers
//!
//! Shared utility functions for CLI command implementations.

use anyhow::Result;
use std::io::Write;

/// Runtime context for doctor command
#[derive(Debug, Clone, Default)]
pub struct DoctorRuntimeContext {
    pub cwd: Option<String>,
    pub db_connected: Option<bool>,
    pub provider_initialized: Option<bool>,
}

/// Format a doctor status line
pub(crate) fn doctor_status(label: &str, status: &str, detail: impl AsRef<str>) -> String {
    format!("{:<18} {:<6} {}\n", label, status, detail.as_ref())
}

/// Get version string for a command
pub(crate) fn command_version(command: &str, arg: &str) -> Option<String> {
    let output = std::process::Command::new(command).arg(arg).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Prompt user for input with optional default
pub(crate) fn prompt_input(label: &str, default: Option<&str>) -> Result<String> {
    if let Some(default) = default {
        println!("{} [{}]: ", label, default);
    } else {
        println!("{}: ", label);
    }
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        Ok(default.unwrap_or_default().to_string())
    } else {
        Ok(input.to_string())
    }
}

/// Prompt user for yes/no answer
pub(crate) fn prompt_yes_no(label: &str, default: bool) -> Result<bool> {
    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    loop {
        let answer = prompt_input(&format!("{} {}", label, suffix), None)?;
        let normalized = answer.trim().to_lowercase();
        if normalized.is_empty() {
            return Ok(default);
        }
        match normalized.as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Please answer y or n."),
        }
    }
}

/// Get default models for a provider (cheap, standard, premium)
pub(crate) fn provider_default_models(provider: &str) -> (String, String, String) {
    let config = crate::config::defaults::default_config();
    let provider_config = config
        .providers
        .configs
        .as_ref()
        .and_then(|configs| configs.get(provider));

    let standard = provider_config
        .map(|cfg| cfg.default_model.clone())
        .unwrap_or_else(|| config.model.clone());
    let cheap = provider_config
        .and_then(|cfg| {
            cfg.cheap_model
                .clone()
                .or_else(|| cfg.research_model.clone())
        })
        .unwrap_or_else(|| standard.clone());
    let premium = standard.clone();

    (cheap, standard, premium)
}
