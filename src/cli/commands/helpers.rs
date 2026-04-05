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

/// ANSI-colored status symbol: OK=green, WARN=yellow, FAIL=red, INFO=grey.
pub(crate) fn status_symbol(status: &str) -> &'static str {
    match status {
        "OK" => "\x1b[32m✔\x1b[0m",
        "WARN" => "\x1b[33m!\x1b[0m",
        "FAIL" => "\x1b[31m✘\x1b[0m",
        _ => "\x1b[90m·\x1b[0m",
    }
}

/// Format a doctor status line: `✔ Label              detail`
pub(crate) fn doctor_status(label: &str, status: &str, detail: impl AsRef<str>) -> String {
    format!(
        "  {} {:<18} {}\n",
        status_symbol(status),
        label,
        detail.as_ref()
    )
}

/// Available disk space in MB at the given path, using `df -m`.
/// Returns None if `df` is unavailable or output cannot be parsed.
pub fn free_disk_mb(path: &str) -> Option<u64> {
    let out = std::process::Command::new("df")
        .args(["-m", path])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    // df line: <filesystem> <1M-blocks> <Used> <Avail> ...
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().nth(1)?;
    // Field index 3 = Available on Linux, index 4 on macOS (Avail).
    // Try both — pick whichever parses as a number ≥ 0.
    let fields: Vec<&str> = line.split_whitespace().collect();
    fields.get(3).or_else(|| fields.get(4))?.parse().ok()
}

/// Get version string for a command
pub(crate) fn command_version(command: &str, arg: &str) -> Option<String> {
    let out = std::process::Command::new(command).arg(arg).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Prompt user for input with optional default. Keeps the cursor on the same line.
pub(crate) fn prompt_input(label: &str, default: Option<&str>) -> Result<String> {
    if let Some(d) = default {
        print!("  {} [{}]: ", label, d);
    } else {
        print!("  {}: ", label);
    }
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();

    Ok(if trimmed.is_empty() {
        default.unwrap_or_default().to_string()
    } else {
        trimmed.to_string()
    })
}

/// Prompt user for a yes/no confirmation.
pub(crate) fn prompt_yes_no(label: &str, default: bool) -> Result<bool> {
    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    loop {
        let answer = prompt_input(&format!("{} {}", label, suffix), None)?;
        match answer.to_lowercase().as_str() {
            "" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("  Please answer y or n."),
        }
    }
}

/// Get default models for a provider: (cheap, standard, premium).
pub(crate) fn provider_default_models(provider: &str) -> (String, String, String) {
    let config = crate::config::defaults::default_config();
    let provider_cfg = config
        .providers
        .configs
        .as_ref()
        .and_then(|cfgs| cfgs.get(provider));

    let standard = provider_cfg
        .map(|c| c.default_model.clone())
        .unwrap_or_else(|| config.model.clone());
    let cheap = provider_cfg
        .and_then(|c| c.cheap_model.clone().or_else(|| c.research_model.clone()))
        .unwrap_or_else(|| standard.clone());
    let premium = standard.clone();

    (cheap, standard, premium)
}
