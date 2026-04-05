//! Sandbox Configuration
//!
//! OS-level sandboxing for Bash tool execution.

use serde::{Deserialize, Serialize};

/// Sandbox execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    /// Use platform-native sandboxing (seatbelt on macOS, bubblewrap on Linux)
    Native,
    /// Use restricted mode without OS sandboxing
    Restricted,
    /// No sandboxing
    Disabled,
}

impl Default for SandboxMode {
    fn default() -> Self {
        Self::Disabled
    }
}

/// Network restriction configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct NetworkRestriction {
    /// Allowed domains (empty = allow all)
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    /// Blocked domains
    #[serde(default)]
    pub blocked_domains: Vec<String>,
    /// HTTP proxy port
    pub http_proxy_port: Option<u16>,
    /// SOCKS proxy port
    pub socks_proxy_port: Option<u16>,
}

impl Default for NetworkRestriction {
    fn default() -> Self {
        Self {
            allowed_domains: Vec::new(),
            blocked_domains: Vec::new(),
            http_proxy_port: None,
            socks_proxy_port: None,
        }
    }
}

/// Filesystem restriction configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct FilesystemRestriction {
    /// Denied read paths
    #[serde(default)]
    pub deny_read: Vec<String>,
    /// Allowed write paths
    #[serde(default)]
    pub allow_write: Vec<String>,
    /// Denied write paths
    #[serde(default)]
    pub deny_write: Vec<String>,
}

impl Default for FilesystemRestriction {
    fn default() -> Self {
        Self {
            deny_read: Vec::new(),
            allow_write: Vec::new(),
            deny_write: Vec::new(),
        }
    }
}

/// Sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct SandboxConfig {
    /// Sandbox mode
    #[serde(default)]
    pub mode: SandboxMode,
    /// Whether sandboxing is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Network restrictions
    #[serde(default)]
    pub network: NetworkRestriction,
    /// Filesystem restrictions
    #[serde(default)]
    pub filesystem: FilesystemRestriction,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            mode: SandboxMode::Disabled,
            enabled: false,
            network: NetworkRestriction::default(),
            filesystem: FilesystemRestriction::default(),
        }
    }
}
