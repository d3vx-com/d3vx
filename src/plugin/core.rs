//! Core Plugin Types
//!
//! Base trait and context for all plugins.

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),
    #[error("Initialization failed: {0}")]
    InitFailed(String),
    #[error("Already loaded: {0}")]
    AlreadyLoaded(String),
    #[error("Capability not supported: {0}")]
    CapabilityNotSupported(String),
    #[error("Plugin execution failed: {0}")]
    Execution(String),
    #[error("Plugin not configured: {0}")]
    NotConfigured(String),
    #[error("Plugin I/O error: {0}")]
    Io(String),
}

#[derive(Debug, Clone)]
pub struct PluginContext {
    pub app_dir: std::path::PathBuf,
    pub config_dir: std::path::PathBuf,
    pub cache_dir: std::path::PathBuf,
    pub data_dir: std::path::PathBuf,
}

impl PluginContext {
    pub fn new() -> Self {
        let base = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".d3vx");
        Self {
            app_dir: base.clone(),
            config_dir: base.join("config"),
            cache_dir: base.join("cache"),
            data_dir: base.join("data"),
        }
    }
}

impl Default for PluginContext {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn description(&self) -> Option<&str> {
        None
    }
    fn init(&self, _context: &PluginContext) -> Result<(), PluginError> {
        Ok(())
    }
    fn shutdown(&self) -> Result<(), PluginError> {
        Ok(())
    }
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: self.description().map(String::from),
            author: None,
            entry: None,
            capabilities: vec![],
        }
    }
}
