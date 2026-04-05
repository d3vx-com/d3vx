//! Team coordinator: manages swarm lifecycle, membership, and global registry.
//!
//! Wraps a `MessageBus` and tracks `MemberDescriptor`s in memory.  A global
//! registry (`ACTIVE_SWARMS`) allows tools and dispatch code to look up a
//! swarm's bus by name without holding a full coordinator reference.

use crate::agent::specialists::AgentType;
use crate::team::message_bus::MessageBus;
use crate::team::workspace::{MemberEntry, TeamManifest, TeamWorkspace};
use crate::tools::AgentRole;
use anyhow::{bail, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock as StdRwLock};
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

// -- Configuration ----------------------------------------------------------

/// Immutable configuration supplied when creating a swarm.
#[derive(Debug, Clone)]
pub struct SwarmConfig {
    pub name: String,
    pub description: String,
    pub base_cwd: String,
    pub max_members: u8,
}

// -- Member types -----------------------------------------------------------

/// Runtime status of an individual team member.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemberStatus {
    Idle,
    Working,
    Done,
    Failed,
}

/// In-memory descriptor for a team member (richer than `MemberEntry`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberDescriptor {
    pub agent_id: String,
    pub call_sign: String,
    pub agent_type: AgentType,
    pub role: AgentRole,
    pub status: MemberStatus,
}

// -- Inner mutable state ----------------------------------------------------

struct SwarmInner {
    config: SwarmConfig,
    members: HashMap<String, MemberDescriptor>,
    lead_call_sign: Option<String>,
    is_active: bool,
}

// -- TeamCoordinator --------------------------------------------------------

/// Orchestrates multi-agent team lifecycle and membership.
pub struct TeamCoordinator {
    inner: Arc<RwLock<SwarmInner>>,
    bus: MessageBus,
}

impl TeamCoordinator {
    /// Create a new, empty coordinator backed by the given bus.
    pub fn new(config: SwarmConfig, bus: MessageBus) -> Self {
        debug!(name = %config.name, "created TeamCoordinator");
        Self {
            inner: Arc::new(RwLock::new(SwarmInner {
                config,
                members: HashMap::new(),
                lead_call_sign: None,
                is_active: true,
            })),
            bus,
        }
    }

    /// Add a member. Errors when the cap is reached or call sign is taken.
    pub async fn enroll_member(
        &self,
        call_sign: String,
        agent_type: AgentType,
        role: AgentRole,
    ) -> Result<MemberDescriptor> {
        let mut inner = self.inner.write().await;
        if inner.members.len() >= inner.config.max_members as usize {
            bail!(
                "swarm '{}' is full (max {} members)",
                inner.config.name,
                inner.config.max_members
            );
        }
        if inner.members.contains_key(&call_sign) {
            bail!("call sign '{}' is already enrolled", call_sign);
        }
        let descriptor = MemberDescriptor {
            agent_id: Uuid::new_v4().to_string(),
            call_sign: call_sign.clone(),
            agent_type,
            role,
            status: MemberStatus::Idle,
        };
        debug!(call_sign = %call_sign, team = %inner.config.name, "enrolled member");
        inner.members.insert(call_sign, descriptor.clone());
        Ok(descriptor)
    }

    /// Designate the lead member by call sign.
    pub async fn set_lead(&self, call_sign: &str) {
        self.inner.write().await.lead_call_sign = Some(call_sign.to_string());
    }

    /// Look up a member descriptor by call sign.
    pub async fn find_member(&self, call_sign: &str) -> Option<MemberDescriptor> {
        self.inner.read().await.members.get(call_sign).cloned()
    }

    /// Transition a member to a new status.
    pub async fn update_member_status(&self, call_sign: &str, status: MemberStatus) -> Result<()> {
        let mut inner = self.inner.write().await;
        let member = inner
            .members
            .get_mut(call_sign)
            .ok_or_else(|| anyhow::anyhow!("member '{}' not found", call_sign))?;
        debug!(call_sign = %call_sign, old = ?member.status, new = ?status, "updating member status");
        member.status = status;
        Ok(())
    }

    /// Return a snapshot of all current members.
    pub async fn list_members(&self) -> Vec<MemberDescriptor> {
        self.inner.read().await.members.values().cloned().collect()
    }

    /// Whether the swarm is still active.
    pub async fn is_active(&self) -> bool {
        self.inner.read().await.is_active
    }

    /// Mark the swarm as inactive (e.g. after shutdown).
    pub async fn deactivate(&self) {
        let name = self.inner.read().await.config.name.clone();
        self.inner.write().await.is_active = false;
        debug!(name = %name, "deactivating swarm");
    }

    /// Return a clone of the configuration.
    pub async fn config(&self) -> SwarmConfig {
        self.inner.read().await.config.clone()
    }

    /// Access the underlying message bus.
    pub fn bus(&self) -> &MessageBus {
        &self.bus
    }

    /// Return the lead member's call sign, if set.
    pub async fn lead_call_sign(&self) -> Option<String> {
        self.inner.read().await.lead_call_sign.clone()
    }

    /// Persist the current swarm state to a workspace directory.
    pub async fn persist_to_workspace(&self, workspace: &TeamWorkspace) -> Result<()> {
        let inner = self.inner.read().await;
        let entries: Vec<MemberEntry> = inner
            .members
            .values()
            .map(|m| MemberEntry {
                agent_id: m.agent_id.clone(),
                call_sign: m.call_sign.clone(),
                agent_type_key: serde_json::to_string(&m.agent_type)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
                role_key: serde_json::to_string(&m.role)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
                status: match m.status {
                    MemberStatus::Idle => "idle",
                    MemberStatus::Working => "working",
                    MemberStatus::Done => "done",
                    MemberStatus::Failed => "failed",
                }
                .to_string(),
            })
            .collect();

        let manifest = TeamManifest {
            name: inner.config.name.clone(),
            description: inner.config.description.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            lead_call_sign: inner.lead_call_sign.clone(),
            members: entries,
            workspace_root: inner.config.base_cwd.clone(),
        };

        if workspace.exists() {
            workspace.update(&manifest)
        } else {
            workspace.create(&manifest)
        }
    }
}

// -- Global swarm registry --------------------------------------------------

static ACTIVE_SWARMS: Lazy<StdRwLock<HashMap<String, Arc<TeamCoordinator>>>> =
    Lazy::new(|| StdRwLock::new(HashMap::new()));

/// Insert a coordinator into the global registry.
pub fn register_swarm(name: &str, coord: Arc<TeamCoordinator>) {
    debug!(name = %name, "registering swarm in global registry");
    ACTIVE_SWARMS
        .write()
        .unwrap()
        .insert(name.to_string(), coord);
}

/// Retrieve a swarm's message bus by name (for tool / dispatch use).
pub fn get_swarm(name: &str) -> Option<MessageBus> {
    ACTIVE_SWARMS
        .read()
        .unwrap()
        .get(name)
        .map(|c| c.bus().clone())
}

/// Retrieve a full coordinator handle by name.
pub fn get_coordinator(name: &str) -> Option<Arc<TeamCoordinator>> {
    ACTIVE_SWARMS.read().unwrap().get(name).cloned()
}

/// Remove a swarm from the global registry.
pub fn unregister_swarm(name: &str) {
    debug!(name = %name, "unregistering swarm from global registry");
    ACTIVE_SWARMS.write().unwrap().remove(name);
}

// -- Tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(name: &str) -> SwarmConfig {
        SwarmConfig {
            name: name.to_string(),
            description: "test swarm".to_string(),
            base_cwd: "/tmp".to_string(),
            max_members: 4,
        }
    }

    #[tokio::test]
    async fn enroll_member_adds_to_list() {
        let coord = TeamCoordinator::new(test_config("t1"), MessageBus::new());
        let desc = coord
            .enroll_member("alpha".into(), AgentType::General, AgentRole::TechLead)
            .await
            .expect("enroll");
        assert_eq!(desc.call_sign, "alpha");
        assert_eq!(desc.status, MemberStatus::Idle);
        assert_eq!(coord.list_members().await.len(), 1);
    }

    #[tokio::test]
    async fn enroll_member_rejects_when_full() {
        let mut cfg = test_config("t2");
        cfg.max_members = 1;
        let coord = TeamCoordinator::new(cfg, MessageBus::new());
        coord
            .enroll_member("a".into(), AgentType::General, AgentRole::Executor)
            .await
            .expect("first");
        let err = coord
            .enroll_member("b".into(), AgentType::Backend, AgentRole::Executor)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("full"));
    }

    #[tokio::test]
    async fn set_lead_and_find_member() {
        let coord = TeamCoordinator::new(test_config("t3"), MessageBus::new());
        coord
            .enroll_member("lead".into(), AgentType::General, AgentRole::TechLead)
            .await
            .expect("enroll");
        coord.set_lead("lead").await;
        assert_eq!(coord.lead_call_sign().await, Some("lead".to_string()));
        assert_eq!(
            coord.find_member("lead").await.expect("find").role,
            AgentRole::TechLead
        );
        assert!(coord.find_member("ghost").await.is_none());
    }

    #[tokio::test]
    async fn update_member_status_transitions() {
        let coord = TeamCoordinator::new(test_config("t4"), MessageBus::new());
        coord
            .enroll_member("w".into(), AgentType::Backend, AgentRole::Executor)
            .await
            .expect("enroll");
        coord
            .update_member_status("w", MemberStatus::Working)
            .await
            .expect("to working");
        assert_eq!(
            coord.find_member("w").await.unwrap().status,
            MemberStatus::Working
        );
        coord
            .update_member_status("w", MemberStatus::Done)
            .await
            .expect("to done");
        assert_eq!(
            coord.find_member("w").await.unwrap().status,
            MemberStatus::Done
        );
    }

    #[tokio::test]
    async fn deactivate_changes_state() {
        let coord = TeamCoordinator::new(test_config("t5"), MessageBus::new());
        assert!(coord.is_active().await);
        coord.deactivate().await;
        assert!(!coord.is_active().await);
    }

    #[test]
    fn global_registry_cycle() {
        let name = "registry-test";
        let coord = Arc::new(TeamCoordinator::new(test_config(name), MessageBus::new()));
        register_swarm(name, coord.clone());
        assert!(get_swarm(name).is_some());
        assert!(get_coordinator(name).is_some());
        unregister_swarm(name);
        assert!(get_swarm(name).is_none());
        assert!(get_coordinator(name).is_none());
    }
}
