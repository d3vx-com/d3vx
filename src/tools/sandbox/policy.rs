//! Sandbox Policy Conversion
//!
//! Translates a [`SandboxConfig`] into platform-specific sandbox arguments.
//! This module acts as a policy bridge between the configuration layer and
//! the concrete sandbox executors.

use crate::config::types::{FilesystemRestriction, NetworkRestriction, SandboxConfig, SandboxMode};

/// Resolved sandbox policy with platform-agnostic restrictions.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    /// Whether network access is allowed at all.
    pub network_allowed: bool,
    /// Domains that are explicitly permitted.
    pub permitted_domains: Vec<String>,
    /// Domains that are explicitly blocked.
    pub blocked_domains: Vec<String>,
    /// Paths that must be readable (added to platform allow-read list).
    pub readable_paths: Vec<String>,
    /// Paths that must be writable (added to platform allow-write list).
    pub writable_paths: Vec<String>,
    /// Paths where read access must be blocked.
    pub unreadable_paths: Vec<String>,
    /// Paths where write access must be blocked.
    pub unwritable_paths: Vec<String>,
}

impl SandboxPolicy {
    /// Derive a sandbox policy from the given configuration.
    pub fn from_config(config: &SandboxConfig) -> Self {
        Self {
            network_allowed: derive_network_allowance(&config.network),
            permitted_domains: config.network.allowed_domains.clone(),
            blocked_domains: config.network.blocked_domains.clone(),
            readable_paths: derive_readable_paths(&config.filesystem),
            writable_paths: config.filesystem.allow_write.clone(),
            unreadable_paths: config.filesystem.deny_read.clone(),
            unwritable_paths: config.filesystem.deny_write.clone(),
        }
    }

    /// Check whether the policy imposes any restrictions beyond the default.
    pub fn has_restrictions(&self) -> bool {
        !self.blocked_domains.is_empty()
            || !self.permitted_domains.is_empty()
            || !self.unreadable_paths.is_empty()
            || !self.writable_paths.is_empty()
            || !self.unwritable_paths.is_empty()
    }

    /// Return the effective sandbox mode, falling back to `Disabled` when
    /// the config's `enabled` flag is `false`.
    pub fn effective_mode(config: &SandboxConfig) -> SandboxMode {
        if config.enabled {
            config.mode
        } else {
            SandboxMode::Disabled
        }
    }

    /// Convert network policy into Seatbelt profile lines.
    pub fn to_seatbelt_network_rules(&self) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        if !self.network_allowed {
            lines.push("; network: denied (blocked domains present)".to_string());
            return lines;
        }
        if self.permitted_domains.is_empty() && self.blocked_domains.is_empty() {
            lines.push("(allow network* (remote))".to_string());
            return lines;
        }
        for d in &self.permitted_domains {
            lines.push(format!("(allow network (remote (domain \"{d}\")))"));
        }
        for d in &self.blocked_domains {
            lines.push(format!("(deny network (remote (domain \"{d}\")))"));
        }
        lines.push("(allow network (remote (domain \"localhost\")))".to_string());
        lines
    }

    /// Convert filesystem restrictions into Seatbelt profile lines.
    pub fn to_seatbelt_filesystem_rules(&self) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for p in &self.unreadable_paths {
            lines.push(format!("(deny file-read* (subpath \"{p}\"))"));
        }
        for p in &self.writable_paths {
            lines.push(format!("(allow file-write* (subpath \"{p}\"))"));
        }
        for p in &self.unwritable_paths {
            lines.push(format!("(deny file-write* (subpath \"{p}\"))"));
        }
        lines
    }

    /// Convert network policy into bwrap arguments (coarse-grained).
    pub fn to_bwrap_network_args(&self) -> Vec<String> {
        if !self.network_allowed {
            vec!["--unshare-net".to_string()]
        } else {
            vec![]
        }
    }

    /// Convert filesystem policy into bwrap arguments.
    pub fn to_bwrap_filesystem_args(&self) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        for p in &self.writable_paths {
            args.extend_from_slice(&["--bind".into(), p.clone(), p.clone()]);
        }
        for p in &self.unreadable_paths {
            args.extend_from_slice(&["--tmpfs".into(), p.clone()]);
        }
        for p in &self.unwritable_paths {
            args.extend_from_slice(&["--ro-bind".into(), p.clone(), p.clone()]);
        }
        args
    }
}

/// Network is denied when blocked domains exist without any allowed ones.
fn derive_network_allowance(net: &NetworkRestriction) -> bool {
    if !net.blocked_domains.is_empty() && net.allowed_domains.is_empty() {
        return false;
    }
    true
}

/// Reserved for future `allow_read` config field.
fn derive_readable_paths(_fs: &FilesystemRestriction) -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net_cfg(allowed: Vec<&str>, blocked: Vec<&str>) -> SandboxConfig {
        SandboxConfig {
            network: NetworkRestriction {
                allowed_domains: allowed.into_iter().map(String::from).collect(),
                blocked_domains: blocked.into_iter().map(String::from).collect(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn fs_cfg(dr: Vec<&str>, aw: Vec<&str>, dw: Vec<&str>) -> SandboxConfig {
        SandboxConfig {
            filesystem: FilesystemRestriction {
                deny_read: dr.into_iter().map(String::from).collect(),
                allow_write: aw.into_iter().map(String::from).collect(),
                deny_write: dw.into_iter().map(String::from).collect(),
            },
            ..Default::default()
        }
    }

    #[test]
    fn default_allows_network_no_restrictions() {
        let p = SandboxPolicy::from_config(&SandboxConfig::default());
        assert!(p.network_allowed);
        assert!(!p.has_restrictions());
    }

    #[test]
    fn blocked_only_disables_network() {
        assert!(!SandboxPolicy::from_config(&net_cfg(vec![], vec!["evil.com"])).network_allowed);
    }

    #[test]
    fn allowed_only_keeps_network() {
        assert!(SandboxPolicy::from_config(&net_cfg(vec!["a.com"], vec![])).network_allowed);
    }

    #[test]
    fn mixed_allow_block_keeps_network() {
        assert!(
            SandboxPolicy::from_config(&net_cfg(vec!["a.com"], vec!["evil.com"])).network_allowed
        );
    }

    #[test]
    fn filesystem_propagates() {
        let p = SandboxPolicy::from_config(&fs_cfg(vec!["/s"], vec!["/w"], vec!["/x"]));
        assert_eq!(p.unreadable_paths, vec!["/s".to_string()]);
        assert_eq!(p.writable_paths, vec!["/w".to_string()]);
        assert_eq!(p.unwritable_paths, vec!["/x".to_string()]);
        assert!(p.has_restrictions());
    }

    #[test]
    fn domains_forwarded() {
        let p = SandboxPolicy::from_config(&net_cfg(vec!["a.com", "b.com"], vec!["x.com"]));
        assert_eq!(p.permitted_domains, vec!["a.com", "b.com"]);
        assert_eq!(p.blocked_domains, vec!["x.com"]);
    }

    #[test]
    fn effective_mode_respects_enabled() {
        let on = SandboxConfig {
            mode: SandboxMode::Native,
            enabled: true,
            ..Default::default()
        };
        let off = SandboxConfig {
            mode: SandboxMode::Native,
            enabled: false,
            ..Default::default()
        };
        assert_eq!(SandboxPolicy::effective_mode(&on), SandboxMode::Native);
        assert_eq!(SandboxPolicy::effective_mode(&off), SandboxMode::Disabled);
    }

    #[test]
    fn seatbelt_net_default() {
        let r = SandboxPolicy::from_config(&SandboxConfig::default()).to_seatbelt_network_rules();
        assert!(r.contains(&"(allow network* (remote))".to_string()));
    }

    #[test]
    fn seatbelt_net_blocked() {
        let r =
            SandboxPolicy::from_config(&net_cfg(vec![], vec!["e.com"])).to_seatbelt_network_rules();
        assert!(r.iter().any(|l| l.contains("denied")));
    }

    #[test]
    fn seatbelt_net_allowed() {
        let r =
            SandboxPolicy::from_config(&net_cfg(vec!["a.com"], vec![])).to_seatbelt_network_rules();
        assert!(r.iter().any(|l| l.contains("a.com")));
        assert!(r.iter().any(|l| l.contains("localhost")));
    }

    #[test]
    fn seatbelt_fs_rules() {
        let r = SandboxPolicy::from_config(&fs_cfg(vec!["/s"], vec!["/w"], vec!["/x"]))
            .to_seatbelt_filesystem_rules();
        assert!(r
            .iter()
            .any(|l| l.contains("deny file-read") && l.contains("/s")));
        assert!(r
            .iter()
            .any(|l| l.contains("allow file-write") && l.contains("/w")));
        assert!(r
            .iter()
            .any(|l| l.contains("deny file-write") && l.contains("/x")));
    }

    #[test]
    fn bwrap_net_default_empty() {
        assert!(SandboxPolicy::from_config(&SandboxConfig::default())
            .to_bwrap_network_args()
            .is_empty());
    }

    #[test]
    fn bwrap_net_blocked_unshares() {
        assert!(SandboxPolicy::from_config(&net_cfg(vec![], vec!["e.com"]))
            .to_bwrap_network_args()
            .contains(&"--unshare-net".to_string()));
    }

    #[test]
    fn bwrap_fs_args() {
        let a = SandboxPolicy::from_config(&fs_cfg(vec!["/s"], vec!["/w"], vec!["/x"]))
            .to_bwrap_filesystem_args();
        assert!(a
            .windows(3)
            .any(|w| w[0] == "--bind" && w[1] == "/w" && w[2] == "/w"));
        assert!(a.windows(2).any(|w| w[0] == "--tmpfs" && w[1] == "/s"));
        assert!(a
            .windows(3)
            .any(|w| w[0] == "--ro-bind" && w[1] == "/x" && w[2] == "/x"));
    }
}
