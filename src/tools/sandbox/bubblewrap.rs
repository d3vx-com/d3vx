//! Linux Bubblewrap (bwrap) Sandbox Implementation
//!
//! Builds `bwrap` argument lists from [`SandboxConfig`] and wraps command
//! execution inside a bubblewrap namespace.

use std::path::Path;
use tracing::debug;

use super::{ProcessSandbox, SandboxError};
use crate::config::types::{FilesystemRestriction, NetworkRestriction, SandboxConfig};

/// Linux bubblewrap (`bwrap`) sandbox executor.
pub struct BubblewrapSandbox;

impl ProcessSandbox for BubblewrapSandbox {
    fn build_command(
        &self,
        cmd: &str,
        cwd: &Path,
        config: &SandboxConfig,
    ) -> Result<std::process::Command, SandboxError> {
        // Verify bwrap is available.
        which_exists("bwrap").map_err(|_| SandboxError::ExecutableNotFound("bwrap".to_string()))?;

        let mut command = std::process::Command::new("bwrap");
        let args = build_args(config);

        for arg in &args {
            command.arg(arg);
        }

        // Run bash -c <cmd> inside the sandbox.
        command.arg("--");
        command.args(["bash", "-c", cmd]);
        command.current_dir(cwd);

        debug!(
            cmd = %cmd,
            cwd = ?cwd.display(),
            args_count = args.len(),
            "built bubblewrap sandbox command"
        );

        Ok(command)
    }
}

/// Build the full `bwrap` argument list from the configuration.
///
/// The resulting arguments create a sandbox that:
/// - Mounts the host filesystem read-only by default.
/// - Optionally shares the network namespace (or unshares it).
/// - Binds specific paths as writable or denies access entirely.
fn build_args(config: &SandboxConfig) -> Vec<String> {
    let mut args = Vec::new();

    // ---- Base filesystem: read-only bind of the entire root ----
    args.push("--ro-bind".to_string());
    args.push("/".to_string());
    args.push("/".to_string());

    // ---- Required mounts for a working shell ----
    for (src, dest) in REQUIRED_BINDS {
        args.push("--bind".to_string());
        args.push((*src).to_string());
        args.push((*dest).to_string());
    }

    // ---- Proc and dev ----
    args.push("--proc".to_string());
    args.push("/proc".to_string());
    args.push("--dev".to_string());
    args.push("/dev".to_string());

    // ---- tmpfs on /tmp so writes there are isolated ----
    args.push("--tmpfs".to_string());
    args.push("/tmp".to_string());

    // ---- Unshare network unless explicitly allowed ----
    if !network_is_allowed(&config.network) {
        args.push("--unshare-net".to_string());
    }

    // ---- Filesystem restrictions ----
    append_filesystem_args(&config.filesystem, &mut args);

    // ---- Die on parent exit ----
    args.push("--die-with-parent".to_string());

    args
}

/// Paths that must be bind-mounted for a functional shell environment.
static REQUIRED_BINDS: &[(&str, &str)] = &[("/sys", "/sys"), ("/run", "/run")];

/// Check whether the network restriction allows any outbound access.
fn network_is_allowed(network: &NetworkRestriction) -> bool {
    // If allowed_domains is non-empty, network is explicitly permitted
    // (to those domains).  If there are no restrictions at all we still
    // allow network (the default config is "open").
    let has_explicit_deny = !network.blocked_domains.is_empty();
    let has_explicit_allow = !network.allowed_domains.is_empty();

    if has_explicit_deny && !has_explicit_allow {
        // Domains are blocked but nothing is explicitly allowed.
        // bwrap can only unshare-all or share-all; we conservatively
        // unshare and let the allowed list be enforced at a higher level.
        return false;
    }

    // Default: allow network.
    true
}

/// Append filesystem bind-mount arguments from the configuration.
fn append_filesystem_args(fs: &FilesystemRestriction, args: &mut Vec<String>) {
    // Allowed write paths: upgrade from ro-bind to bind (writable).
    for path in &fs.allow_write {
        args.push("--bind".to_string());
        args.push(path.clone());
        args.push(path.clone());
    }

    // Denied read paths: overlay with an empty tmpfs to make them
    // inaccessible inside the sandbox.
    for path in &fs.deny_read {
        args.push("--tmpfs".to_string());
        args.push(path.clone());
    }

    // Denied write paths: remount read-only.
    for path in &fs.deny_write {
        args.push("--ro-bind".to_string());
        args.push(path.clone());
        args.push(path.clone());
    }
}

/// Check whether an executable exists on `$PATH`.
fn which_exists(name: &str) -> Result<(), ()> {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| if o.status.success() { Ok(()) } else { Err(()) })
        .unwrap_or(Err(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> SandboxConfig {
        SandboxConfig::default()
    }

    #[test]
    fn test_build_args_contains_ro_bind_root() {
        let args = build_args(&default_config());
        let ro_bind_idx = args.iter().position(|a| a == "--ro-bind");
        assert!(ro_bind_idx.is_some());
        // The next two elements should be "/" and "/".
        let idx = ro_bind_idx.unwrap();
        assert_eq!(args[idx + 1], "/");
        assert_eq!(args[idx + 2], "/");
    }

    #[test]
    fn test_build_args_contains_proc() {
        let args = build_args(&default_config());
        let has_proc = args.windows(2).any(|w| w[0] == "--proc" && w[1] == "/proc");
        assert!(has_proc);
    }

    #[test]
    fn test_build_args_contains_dev() {
        let args = build_args(&default_config());
        let has_dev = args.windows(2).any(|w| w[0] == "--dev" && w[1] == "/dev");
        assert!(has_dev);
    }

    #[test]
    fn test_build_args_contains_tmpfs_tmp() {
        let args = build_args(&default_config());
        let has_tmpfs = args.windows(2).any(|w| w[0] == "--tmpfs" && w[1] == "/tmp");
        assert!(has_tmpfs);
    }

    #[test]
    fn test_build_args_contains_die_with_parent() {
        let args = build_args(&default_config());
        assert!(args.contains(&"--die-with-parent".to_string()));
    }

    #[test]
    fn test_build_args_network_allowed_by_default() {
        let args = build_args(&default_config());
        assert!(!args.contains(&"--unshare-net".to_string()));
    }

    #[test]
    fn test_build_args_unshare_net_when_blocked() {
        let config = SandboxConfig {
            network: NetworkRestriction {
                blocked_domains: vec!["evil.com".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let args = build_args(&config);
        assert!(args.contains(&"--unshare-net".to_string()));
    }

    #[test]
    fn test_build_args_allow_write_bind() {
        let config = SandboxConfig {
            filesystem: FilesystemRestriction {
                allow_write: vec!["/workspace".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let args = build_args(&config);
        // Should contain --bind /workspace /workspace
        let bind_idx = args
            .windows(3)
            .position(|w| w[0] == "--bind" && w[1] == "/workspace" && w[2] == "/workspace");
        assert!(bind_idx.is_some());
    }

    #[test]
    fn test_build_args_deny_read_tmpfs() {
        let config = SandboxConfig {
            filesystem: FilesystemRestriction {
                deny_read: vec!["/etc/secret".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let args = build_args(&config);
        let has_tmpfs = args
            .windows(2)
            .any(|w| w[0] == "--tmpfs" && w[1] == "/etc/secret");
        assert!(has_tmpfs);
    }

    #[test]
    fn test_build_args_deny_write_ro_bind() {
        let config = SandboxConfig {
            filesystem: FilesystemRestriction {
                deny_write: vec!["/system".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let args = build_args(&config);
        let has_ro = args
            .windows(3)
            .any(|w| w[0] == "--ro-bind" && w[1] == "/system" && w[2] == "/system");
        assert!(has_ro);
    }

    #[test]
    fn test_build_command_ends_with_bash_c() {
        let _sandbox = BubblewrapSandbox;
        // bwrap may not be on PATH, so we test arg construction indirectly
        // by calling build_args directly.
        let args = build_args(&default_config());
        // The args themselves should not contain bash -c; those are added
        // separately in build_command.  Verify that "--" separator is NOT
        // in build_args output.
        assert!(!args.contains(&"--".to_string()));
        assert!(!args.contains(&"bash".to_string()));
    }

    #[test]
    fn test_network_is_allowed_default() {
        assert!(network_is_allowed(&NetworkRestriction::default()));
    }

    #[test]
    fn test_network_is_allowed_with_allowed_domains() {
        let net = NetworkRestriction {
            allowed_domains: vec!["api.example.com".into()],
            ..Default::default()
        };
        assert!(network_is_allowed(&net));
    }

    #[test]
    fn test_network_is_not_allowed_only_blocked() {
        let net = NetworkRestriction {
            blocked_domains: vec!["evil.com".into()],
            ..Default::default()
        };
        assert!(!network_is_allowed(&net));
    }
}
