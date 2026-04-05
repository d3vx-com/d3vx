//! Plugin configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for individual plugin slots
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PluginSetting {
    /// Type identifier for the plugin (e.g., "process", "claude")
    #[serde(rename = "type")]
    pub plugin_type: String,
    /// Additional dynamic configuration keys for the plugin
    #[serde(flatten)]
    pub properties: std::collections::HashMap<String, serde_json::Value>,
}

/// Top-level plugin configuration mappings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct PluginsConfig {
    // --- Built-in plugin slots (backward compatible) ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<PluginSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<PluginSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<PluginSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracker: Option<PluginSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scm: Option<PluginSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifier: Option<PluginSetting>,
    // --- Enhanced plugin management ---
    /// Plugin discovery settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery: Option<PluginDiscovery>,
    /// Explicitly enabled plugins
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enabled: Vec<PluginEnabled>,
    /// Plugin-specific options (keyed by plugin name)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub options: HashMap<String, serde_json::Value>,
}

/// Plugin discovery settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PluginDiscovery {
    /// Directories to search for plugins
    #[serde(default)]
    pub search_paths: Vec<String>,
    /// File patterns to match
    #[serde(default)]
    pub patterns: Vec<String>,
}

/// Plugin manifest
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
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
    pub capabilities: Vec<PluginCapability>,
}

/// Plugin capability
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PluginCapability {
    Tool,
    Agent,
    Hook,
    Provider,
    Ui,
}

/// Individual plugin enablement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PluginEnabled {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
}

fn default_true() -> bool {
    true
}
