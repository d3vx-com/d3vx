//! macOS Seatbelt Sandbox Implementation
//!
//! Generates a Seatbelt profile from [`SandboxConfig`] and wraps command
//! execution via `sandbox-exec -f <profile> bash -c <cmd>`.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

use super::{ProcessSandbox, SandboxError};
use crate::config::types::{FilesystemRestriction, NetworkRestriction, SandboxConfig};

/// macOS sandbox-exec (Seatbelt) sandbox executor.
pub struct SeatbeltSandbox;

impl ProcessSandbox for SeatbeltSandbox {
    fn build_command(
        &self,
        cmd: &str,
        cwd: &Path,
        config: &SandboxConfig,
    ) -> Result<std::process::Command, SandboxError> {
        // Verify sandbox-exec is available.
        which_exists("sandbox-exec")
            .map_err(|_| SandboxError::ExecutableNotFound("sandbox-exec".to_string()))?;

        let profile = generate_profile(config);
        let profile_path = write_profile(&profile)?;

        let mut command = std::process::Command::new("sandbox-exec");
        command.arg("-f").arg(&profile_path);
        command.arg("bash").arg("-c").arg(cmd);
        command.current_dir(cwd);

        debug!(
            profile = %profile_path.display(),
            cmd = %cmd,
            cwd = ?cwd.display(),
            "built seatbelt sandbox command"
        );

        Ok(command)
    }
}

/// Write the Seatbelt profile string to a temporary file and return its path.
fn write_profile(content: &str) -> Result<PathBuf, SandboxError> {
    let dir = std::env::temp_dir().join("d3vx-sandbox");
    fs::create_dir_all(&dir).map_err(|e| SandboxError::ProfileWriteFailed(e.to_string()))?;

    // Use process id + thread id to avoid filename collisions.
    let pid = std::process::id();
    let tid = std::thread::current().id();
    let filename = format!("profile-{pid}-{tid:?}.sb");
    let path = dir.join(filename);

    fs::write(&path, content).map_err(|e| SandboxError::ProfileWriteFailed(e.to_string()))?;

    Ok(path)
}

/// Check whether an executable exists on `$PATH`.
fn which_exists(name: &str) -> Result<(), ()> {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| if o.status.success() { Ok(()) } else { Err(()) })
        .unwrap_or(Err(()))
}

/// Generate a Seatbelt profile from the sandbox configuration.
///
/// The profile follows the version 1 format:
/// - `(deny default)` blocks everything not explicitly allowed.
/// - Network rules are added based on [`NetworkRestriction`].
/// - Filesystem rules are added based on [`FilesystemRestriction`].
fn generate_profile(config: &SandboxConfig) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("(version 1)".to_string());
    lines.push("(deny default)".to_string());
    lines.push(String::new()); // blank separator

    // -- Process basics (always allow) --
    lines.push("(allow process-exec)".to_string());
    lines.push("(allow process-fork)".to_string());
    lines.push("(allow signal (target self))".to_string());
    lines.push(String::new());

    // -- Network rules --
    append_network_rules(&config.network, &mut lines);
    lines.push(String::new());

    // -- Filesystem rules --
    append_filesystem_rules(&config.filesystem, &mut lines);

    lines.join("\n")
}

/// Append network allow/deny rules to the profile.
fn append_network_rules(network: &NetworkRestriction, lines: &mut Vec<String>) {
    let has_allowed = !network.allowed_domains.is_empty();
    let has_blocked = !network.blocked_domains.is_empty();

    if !has_allowed && !has_blocked {
        // No restrictions specified: allow all outbound.
        lines.push("; network: unrestricted".to_string());
        lines.push("(allow network* (remote))".to_string());
        return;
    }

    // Explicit allowed domains.
    for domain in &network.allowed_domains {
        lines.push(format!("(allow network (remote (domain \"{}\")))", domain));
    }

    // Explicit blocked domains.
    for domain in &network.blocked_domains {
        lines.push(format!("(deny network (remote (domain \"{}\")))", domain));
    }

    // Always allow localhost for IPC.
    lines.push("(allow network (remote (domain \"localhost\")))".to_string());
    lines.push("(allow network (remote (domain \"127.0.0.1\")))".to_string());
}

/// Append filesystem read/write allow/deny rules to the profile.
fn append_filesystem_rules(fs: &FilesystemRestriction, lines: &mut Vec<String>) {
    // Minimal required paths so that basic commands work.
    lines.push("; filesystem: essential paths".to_string());
    for essential in &["/usr", "/bin", "/sbin", "/lib", "/tmp", "/dev", "/var"] {
        lines.push(format!("(allow file-read* (subpath \"{essential}\"))"));
    }

    // User-specified denied read paths.
    for path in &fs.deny_read {
        lines.push(format!("(deny file-read* (subpath \"{path}\"))"));
    }

    // User-specified allowed write paths.
    for path in &fs.allow_write {
        lines.push(format!("(allow file-write* (subpath \"{path}\"))"));
    }

    // User-specified denied write paths.
    for path in &fs.deny_write {
        lines.push(format!("(deny file-write* (subpath \"{path}\"))"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> SandboxConfig {
        SandboxConfig::default()
    }

    #[test]
    fn test_profile_contains_version_header() {
        let profile = generate_profile(&default_config());
        assert!(profile.contains("(version 1)"));
        assert!(profile.contains("(deny default)"));
    }

    #[test]
    fn test_profile_allows_process_exec() {
        let profile = generate_profile(&default_config());
        assert!(profile.contains("(allow process-exec)"));
        assert!(profile.contains("(allow process-fork)"));
    }

    #[test]
    fn test_network_unrestricted_by_default() {
        let profile = generate_profile(&default_config());
        assert!(profile.contains("(allow network* (remote))"));
    }

    #[test]
    fn test_network_allow_domains() {
        let config = SandboxConfig {
            network: NetworkRestriction {
                allowed_domains: vec!["api.example.com".into(), "cdn.example.com".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let profile = generate_profile(&config);
        assert!(profile.contains("(allow network (remote (domain \"api.example.com\")))"));
        assert!(profile.contains("(allow network (remote (domain \"cdn.example.com\")))"));
    }

    #[test]
    fn test_network_block_domains() {
        let config = SandboxConfig {
            network: NetworkRestriction {
                blocked_domains: vec!["evil.com".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let profile = generate_profile(&config);
        assert!(profile.contains("(deny network (remote (domain \"evil.com\")))"));
        assert!(profile.contains("(allow network (remote (domain \"localhost\")))"));
    }

    #[test]
    fn test_filesystem_essential_paths() {
        let profile = generate_profile(&default_config());
        assert!(profile.contains("(allow file-read* (subpath \"/usr\"))"));
        assert!(profile.contains("(allow file-read* (subpath \"/bin\"))"));
        assert!(profile.contains("(allow file-read* (subpath \"/tmp\"))"));
    }

    #[test]
    fn test_filesystem_deny_read() {
        let config = SandboxConfig {
            filesystem: FilesystemRestriction {
                deny_read: vec!["/etc/secret".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let profile = generate_profile(&config);
        assert!(profile.contains("(deny file-read* (subpath \"/etc/secret\"))"));
    }

    #[test]
    fn test_filesystem_allow_write() {
        let config = SandboxConfig {
            filesystem: FilesystemRestriction {
                allow_write: vec!["/workspace".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let profile = generate_profile(&config);
        assert!(profile.contains("(allow file-write* (subpath \"/workspace\"))"));
    }

    #[test]
    fn test_filesystem_deny_write() {
        let config = SandboxConfig {
            filesystem: FilesystemRestriction {
                deny_write: vec!["/system".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let profile = generate_profile(&config);
        assert!(profile.contains("(deny file-write* (subpath \"/system\"))"));
    }

    #[test]
    fn test_write_profile_creates_file() {
        let profile = generate_profile(&default_config());
        let path = write_profile(&profile).expect("should write profile");
        assert!(path.exists());
        // Read back and verify.
        let contents = fs::read_to_string(&path).expect("should read profile");
        assert!(contents.contains("(version 1)"));
        // Clean up.
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_write_profile_path_under_d3vx_sandbox_dir() {
        let path = write_profile("(version 1)").expect("should write");
        assert!(path.to_string_lossy().contains("d3vx-sandbox"));
        let _ = fs::remove_file(&path);
    }
}
