use crate::plugin::core::{Plugin, PluginError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Plugin slots (extension points) available in d3vx.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginSlot {
    Runtime,
    Workspace,
    Scm,
    Tracker,
    Notifier,
    Terminal,
    AgentBackend,
}

impl std::fmt::Display for PluginSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Runtime => "runtime",
            Self::Workspace => "workspace",
            Self::Scm => "scm",
            Self::Tracker => "tracker",
            Self::Notifier => "notifier",
            Self::Terminal => "terminal",
            Self::AgentBackend => "agent_backend",
        };
        write!(f, "{}", s)
    }
}

/// Metadata describing a plugin and its capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescriptor {
    pub name: String,
    pub version: String,
    pub slot: PluginSlot,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Common Types for Adapter Traits
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHandle {
    pub id: String,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrInfo {
    pub number: u64,
    pub url: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrStatus {
    pub state: String,
    pub mergeable: Option<bool>,
    pub ci_passed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewInfo {
    pub author: String,
    pub state: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueInfo {
    pub id: String,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalHandle {
    pub id: String,
}

// ---------------------------------------------------------------------------
// Adapter Traits (The "Slots")
// ---------------------------------------------------------------------------

pub trait AdapterPlugin: Plugin {
    fn slot(&self) -> PluginSlot;
    fn descriptor(&self) -> PluginDescriptor;
}

#[async_trait]
pub trait RuntimeAdapter: AdapterPlugin {
    async fn start(&self, task_id: &str, command: &str) -> Result<String, PluginError>;
    async fn stop(&self, task_id: &str) -> Result<(), PluginError>;
    async fn is_running(&self, task_id: &str) -> bool;
    async fn output(&self, task_id: &str) -> Result<Option<String>, PluginError>;
}

pub trait WorkspaceAdapter: AdapterPlugin {
    fn create(&self, project_root: &Path, branch: &str) -> Result<String, PluginError>;
    fn remove(&self, workspace_path: &Path) -> Result<(), PluginError>;
    fn exists(&self, workspace_path: &Path) -> bool;
}

#[async_trait]
pub trait ScmAdapter: AdapterPlugin {
    async fn create_pr(&self, branch: &str, title: &str, body: &str)
        -> Result<PrInfo, PluginError>;
    async fn get_pr_status(&self, pr_number: u64) -> Result<PrStatus, PluginError>;
    async fn merge_pr(&self, pr_number: u64) -> Result<(), PluginError>;
    async fn list_pr_checks(&self, pr_number: u64) -> Result<Vec<CheckResult>, PluginError>;
    async fn get_pr_reviews(&self, pr_number: u64) -> Result<Vec<ReviewInfo>, PluginError>;
}

#[async_trait]
pub trait TrackerAdapter: AdapterPlugin {
    async fn fetch_issue(&self, id: &str) -> Result<IssueInfo, PluginError>;
    async fn list_open_issues(&self) -> Result<Vec<IssueInfo>, PluginError>;
    async fn add_comment(&self, id: &str, comment: &str) -> Result<(), PluginError>;
}

#[async_trait]
pub trait NotifierAdapter: AdapterPlugin {
    async fn notify(&self, title: &str, message: &str, priority: &str) -> Result<(), PluginError>;
}

#[async_trait]
pub trait TerminalAdapter: AdapterPlugin {
    async fn create_session(&self, command: &str) -> Result<TerminalHandle, PluginError>;
    async fn read_output(&self, handle: &TerminalHandle) -> Result<String, PluginError>;
    async fn write_input(&self, handle: &TerminalHandle, input: &str) -> Result<(), PluginError>;
    async fn kill(&self, handle: &TerminalHandle) -> Result<(), PluginError>;
}

#[async_trait]
pub trait AgentBackendAdapter: AdapterPlugin {
    async fn spawn(&self, prompt: &str, workspace: &Path) -> Result<AgentHandle, PluginError>;
    async fn send_message(&self, handle: &AgentHandle, message: &str) -> Result<(), PluginError>;
    async fn read_output(&self, handle: &AgentHandle) -> Result<String, PluginError>;
    async fn terminate(&self, handle: &AgentHandle) -> Result<(), PluginError>;
    async fn is_alive(&self, handle: &AgentHandle) -> bool;
}
