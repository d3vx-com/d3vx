//! Permissions and role-based access configuration

use serde::{Deserialize, Serialize};

// Re-export security types for convenience (matches original types.rs)
pub use super::super::security::{BashToolConfig, SecurityConfig};

/// Permission configuration for tools
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct PermissionsConfig {
    /// Tools that are auto-approved
    #[serde(default)]
    pub auto_approve: Vec<String>,
    /// Tools that require explicit user approval
    #[serde(default)]
    pub require_approval: Vec<String>,
    /// Glob patterns for allowed operations
    #[serde(default)]
    pub allow: Vec<String>,
    /// Glob patterns for denied operations
    #[serde(default)]
    pub deny: Vec<String>,
    /// Glob patterns that prompt user
    #[serde(default)]
    pub ask: Vec<String>,
    /// Persistent deny rules
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_always: Option<Vec<String>>,
    /// Trust mode: auto-approve everything
    #[serde(default)]
    pub trust_mode: bool,
}

/// Role-based tool access configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct RoleConfig {
    /// Tech Lead role configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech_lead: Option<RoleToolPermissions>,
    /// QA Engineer role configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qa_engineer: Option<RoleToolPermissions>,
    /// Backend Developer role configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_developer: Option<RoleToolPermissions>,
}

/// Tool permissions for a specific role
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct RoleToolPermissions {
    /// Tools explicitly allowed for this role
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Tools explicitly blocked for this role
    #[serde(default)]
    pub blocked_tools: Vec<String>,
    /// Whether to use strict mode (only allowed tools, nothing else)
    #[serde(default)]
    pub strict_mode: bool,
}
