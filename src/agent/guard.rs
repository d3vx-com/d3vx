//! Command Guard - Tool Execution Interceptor
//!
//! Manages the lifecycle of tool call approvals, providing a synchronized
//! bridge between the asynchronous agent loop and manual user decisions.
//!
//! This module integrates with the formal PermissionManager for state tracking
//! while maintaining the blocking oneshot channel mechanism for UX.

use crate::config::PermissionsConfig;
use crate::ipc::types::{ApprovalDecision, PermissionOption, PermissionRequest, ToolCall};
use crate::pipeline::permission::{
    PermissionDecision as FormalDecision, PermissionManager, PermissionReq, RiskLevel,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, info, warn};

/// A pending approval request
struct PendingRequest {
    /// The tool call being intercepted
    pub call: ToolCall,
    /// Optional diff for the change
    pub diff: Option<String>,
    /// The resource being accessed (e.g. file path)
    pub resource: Option<String>,
    /// Channel to send the decision back to the requester
    pub tx: oneshot::Sender<ApprovalDecision>,
    /// Associated permission request ID (for state tracking)
    pub permission_id: Option<String>,
}

/// Command Guard service
#[derive(Clone)]
pub struct CommandGuard {
    /// Active pending requests by tool ID
    pending: Arc<RwLock<HashMap<String, PendingRequest>>>,
    /// Permissions configuration
    config: Arc<RwLock<PermissionsConfig>>,
    /// Workspace ID associated with this guard
    workspace_id: String,
    /// Formal permission manager (state of record)
    permission_manager: Option<Arc<PermissionManager>>,
    /// Session ID for permission requests
    session_id: String,
}

impl CommandGuard {
    /// Create a new command guard without formal permission tracking.
    pub fn new(config: PermissionsConfig, workspace_id: String) -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config)),
            workspace_id,
            permission_manager: None,
            session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Create a new command guard with formal permission tracking.
    pub fn with_permission_manager(
        config: PermissionsConfig,
        workspace_id: String,
        session_id: String,
        permission_manager: Arc<PermissionManager>,
    ) -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config)),
            workspace_id,
            permission_manager: Some(permission_manager),
            session_id,
        }
    }

    /// Get the permission manager if configured.
    pub fn permission_manager(&self) -> Option<&Arc<PermissionManager>> {
        self.permission_manager.as_ref()
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Update the permissions configuration
    pub async fn update_config(&self, config: PermissionsConfig) {
        let mut guard = self.config.write().await;
        *guard = config;
    }

    /// Check if a tool call requires manual approval and wait for it if necessary.
    ///
    /// # Arguments
    ///
    /// * `call` - The tool call to validate
    ///
    /// # Returns
    ///
    /// The approval decision.
    pub async fn ask_for_approval(
        &self,
        call: ToolCall,
        diff: Option<String>,
        resource: Option<String>,
    ) -> ApprovalDecision {
        let rx = match self.prepare_approval(call, diff, resource).await {
            Ok(rx) => rx,
            Err(decision) => return decision,
        };

        self.wait_for_decision(rx).await
    }

    /// Check permissions and prepare a pending request if needed.
    ///
    /// Returns Ok(receiver) if approval is required.
    /// Returns Err(decision) if it was auto-approved or immediately denied.
    pub async fn prepare_approval(
        &self,
        call: ToolCall,
        diff: Option<String>,
        resource: Option<String>,
    ) -> Result<oneshot::Receiver<ApprovalDecision>, ApprovalDecision> {
        let config = self.config.read().await;

        let normalized_name =
            crate::tools::tool_access::ToolAccessValidator::normalize_tool_name(&call.name);

        // 1. Check Trust Mode
        if config.trust_mode {
            return Err(ApprovalDecision::Approve);
        }

        // 2. Check Auto-Approve list
        if config.auto_approve.iter().any(|t| {
            crate::tools::tool_access::ToolAccessValidator::normalize_tool_name(t)
                == normalized_name
        }) {
            debug!(tool = %call.name, "Auto-approving tool based on config");
            return Err(ApprovalDecision::Approve);
        }

        // 3. Check Explicit Require-Approval list
        let requires_approval = config.require_approval.iter().any(|t| {
            crate::tools::tool_access::ToolAccessValidator::normalize_tool_name(t)
                == normalized_name
        });

        if !requires_approval {
            // Default policy: if not in auto-approve and not in explicit require list,
            // we assume it needs approval for safety, UNLESS it's a "Read" tool.
            if is_read_only_tool(&normalized_name) {
                return Err(ApprovalDecision::Approve);
            }
        }

        info!(tool = %call.name, id = %call.id, "Tool requires manual approval");

        // 4. Create a pending request
        let (tx, rx) = oneshot::channel();
        let id = call.id.clone();

        // 5. Create formal permission request if manager is configured
        let permission_id = if let Some(ref mgr) = self.permission_manager {
            let risk = determine_risk_level(&call.name);
            let req = mgr
                .create(
                    self.session_id.clone(),
                    call.name.clone(),
                    call.id.clone(),
                    risk,
                    format!("Execute {}", call.name),
                    format!("{:?}", call.input),
                    resource.clone(),
                    diff.clone(),
                    format!("Tool execution requires approval: {}", call.name),
                )
                .await;
            let perm_id = req.id.clone();

            // Submit to pending state
            if let Err(e) = mgr.submit(&req.id).await {
                warn!(error = %e, "Failed to submit permission request");
            }

            Some(perm_id)
        } else {
            None
        };

        {
            let mut pending = self.pending.write().await;
            pending.insert(
                id.clone(),
                PendingRequest {
                    call,
                    diff,
                    resource,
                    tx,
                    permission_id,
                },
            );
        }

        Ok(rx)
    }

    /// Wait for a decision on a prepared request.
    pub async fn wait_for_decision(
        &self,
        rx: oneshot::Receiver<ApprovalDecision>,
    ) -> ApprovalDecision {
        // Wait for decision from UI
        match rx.await {
            Ok(decision) => {
                if decision == ApprovalDecision::ApproveAll {
                    let mut config = self.config.write().await;
                    config.trust_mode = true;
                }
                decision
            }
            Err(_) => {
                debug!("Approval request sender dropped - assuming Deny");
                ApprovalDecision::Deny
            }
        }
    }

    /// Get a pending request for a tool call ID.
    pub async fn get_pending_request(&self, tool_call_id: &str) -> Option<PermissionRequest> {
        let pending = self.pending.read().await;
        pending.get(tool_call_id).map(|pr| {
            // Create a PermissionRequest from the ToolCall
            PermissionRequest {
                id: uuid::Uuid::new_v4().to_string(), // Request ID
                workspace_id: Some(self.workspace_id.clone()),
                tool_call_id: Some(pr.call.id.clone()),
                tool_name: Some(pr.call.name.clone()),
                action: "execute".to_string(),
                resource: pr.resource.clone(),
                message: format!("Agent wants to execute tool: {}", pr.call.name),
                diff: pr.diff.clone(),
                options: vec![
                    PermissionOption {
                        label: "Approve (a)".to_string(),
                        value: "approve".to_string(),
                        is_default: true,
                    },
                    PermissionOption {
                        label: "Deny (d)".to_string(),
                        value: "deny".to_string(),
                        is_default: false,
                    },
                    PermissionOption {
                        label: "Always (v)".to_string(),
                        value: "approve_all".to_string(),
                        is_default: false,
                    },
                ],
            }
        })
    }

    /// Provide a decision for a pending approval request.
    ///
    /// This is called by the UI/IPC handler when the user makes a choice.
    /// Updates both the internal pending state and the formal PermissionManager.
    pub async fn provide_decision(&self, id: &str, decision: ApprovalDecision) -> bool {
        let pending_request = {
            let mut pending = self.pending.write().await;
            pending.remove(id)
        };

        if let Some(req) = pending_request {
            // Update formal permission manager if configured
            if let (Some(perm_id), Some(ref mgr)) = (&req.permission_id, &self.permission_manager) {
                let formal_decision = match decision {
                    ApprovalDecision::Approve | ApprovalDecision::ApproveAll => {
                        FormalDecision::Approve
                    }
                    ApprovalDecision::Deny => FormalDecision::Deny,
                };
                if let Err(e) = mgr.decide(perm_id, formal_decision, None).await {
                    warn!(
                        error = %e,
                        permission_id = %perm_id,
                        "Failed to record decision in permission manager"
                    );
                }
            }

            let _ = req.tx.send(decision);
            true
        } else {
            false
        }
    }

    /// Provide a decision by tool call ID (convenience method).
    ///
    /// Looks up the pending request by tool call ID and provides the decision.
    pub async fn provide_decision_by_tool_call(
        &self,
        tool_call_id: &str,
        decision: ApprovalDecision,
    ) -> bool {
        let tool_call_id_owned = tool_call_id.to_string();
        let pending_id = {
            let pending = self.pending.read().await;
            pending
                .iter()
                .find(|(_, pr)| pr.call.id == tool_call_id_owned)
                .map(|(id, _)| id.clone())
        };

        if let Some(id) = pending_id {
            self.provide_decision(&id, decision).await
        } else {
            false
        }
    }

    /// Get all pending tool calls.
    pub async fn list_pending(&self) -> Vec<ToolCall> {
        let pending = self.pending.read().await;
        pending.values().map(|r| r.call.clone()).collect()
    }
}

/// Helper to identify read-only tools that don't need approval by default
fn is_read_only_tool(name: &str) -> bool {
    matches!(
        name,
        "ReadTool" | "GlobTool" | "GrepTool" | "Think" | "Question" | "WebFetch" | "ReadInbox"
    )
}

/// Determine the risk level for a tool based on its name.
fn determine_risk_level(name: &str) -> RiskLevel {
    match name {
        // Read-only tools - low risk
        "ReadTool" | "GlobTool" | "GrepTool" | "Think" | "Question" | "WebFetch" | "ReadInbox" => {
            RiskLevel::Low
        }
        // File modification - medium risk
        "WriteTool" | "EditTool" | "MultiEditTool" | "RenameTool" | "CreateTool" => {
            RiskLevel::Medium
        }
        // File deletion - high risk
        "DeleteTool" => RiskLevel::High,
        // Shell commands - high to critical risk
        "BashTool" | "ShellTool" => RiskLevel::High,
        // Network operations - critical risk
        "HttpTool" | "FetchTool" => RiskLevel::Critical,
        // Default to medium
        _ => RiskLevel::Medium,
    }
}

impl CommandGuard {
    /// Get the formal permission request for a tool call ID (if tracked).
    pub async fn get_permission_request(&self, tool_call_id: &str) -> Option<PermissionReq> {
        let pending = self.pending.read().await;
        if let Some(pr) = pending.get(tool_call_id) {
            if let (Some(perm_id), Some(ref mgr)) = (&pr.permission_id, &self.permission_manager) {
                return mgr.get(perm_id).await;
            }
        }
        None
    }

    /// Get all pending permission requests from the manager.
    pub async fn get_pending_permission_requests(&self) -> Vec<PermissionReq> {
        if let Some(ref mgr) = self.permission_manager {
            mgr.pending_for_session(&self.session_id).await
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::types::ToolStatus;
    use crate::pipeline::PermissionState;

    #[tokio::test]
    async fn test_guard_auto_approve() {
        let config = PermissionsConfig {
            auto_approve: vec!["BashTool".to_string()],
            ..Default::default()
        };
        let guard = CommandGuard::new(config, "test-workspace".to_string());

        let call = ToolCall {
            id: "1".to_string(),
            name: "BashTool".to_string(),
            input: serde_json::Value::Null,
            status: ToolStatus::Pending,
            output: None,
            elapsed: None,
        };

        let decision = guard.ask_for_approval(call, None, None).await;
        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_guard_manual_approval() {
        let config = PermissionsConfig::default();
        let guard = Arc::new(CommandGuard::new(config, "test-workspace".to_string()));

        let call = ToolCall {
            id: "1".to_string(),
            name: "WriteTool".to_string(),
            input: serde_json::Value::Null,
            status: ToolStatus::Pending,
            output: None,
            elapsed: None,
        };

        let guard_clone = guard.clone();
        let handle =
            tokio::spawn(async move { guard_clone.ask_for_approval(call, None, None).await });

        // Small sleep to ensure the request is registered
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let pending = guard.list_pending().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "1");

        // Provide decision
        let found = guard.provide_decision("1", ApprovalDecision::Approve).await;
        assert!(found);

        let decision = handle.await.unwrap();
        assert_eq!(decision, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_guard_trust_mode_approve_all() {
        let config = PermissionsConfig::default();
        let guard = Arc::new(CommandGuard::new(config, "test-workspace".to_string()));

        let call = ToolCall {
            id: "1".to_string(),
            name: "WriteTool".to_string(),
            input: serde_json::Value::Null,
            status: ToolStatus::Pending,
            output: None,
            elapsed: None,
        };

        let guard_clone = guard.clone();
        let handle =
            tokio::spawn(async move { guard_clone.ask_for_approval(call, None, None).await });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Provide "ApproveAll" decision
        guard
            .provide_decision("1", ApprovalDecision::ApproveAll)
            .await;

        let decision = handle.await.unwrap();
        assert_eq!(decision, ApprovalDecision::ApproveAll);

        // Check if trust mode is now enabled
        let config = guard.config.read().await;
        assert!(config.trust_mode);

        // Subsequent tools should be auto-approved
        let call2 = ToolCall {
            id: "2".to_string(),
            name: "BashTool".to_string(),
            input: serde_json::Value::Null,
            status: ToolStatus::Pending,
            output: None,
            elapsed: None,
        };
        let decision2 = guard.ask_for_approval(call2, None, None).await;
        assert_eq!(decision2, ApprovalDecision::Approve);
    }

    #[tokio::test]
    async fn test_guard_with_permission_manager() {
        use crate::pipeline::permission::PermissionManager;
        use std::sync::Arc;

        let config = PermissionsConfig::default();
        let mgr = Arc::new(PermissionManager::new());
        let session_id = "test-session-perm".to_string();

        let guard = CommandGuard::with_permission_manager(
            config,
            "test-workspace".to_string(),
            session_id.clone(),
            mgr.clone(),
        );

        let call = ToolCall {
            id: "perm-test-1".to_string(),
            name: "WriteTool".to_string(),
            input: serde_json::Value::Null,
            status: ToolStatus::Pending,
            output: None,
            elapsed: None,
        };

        // Spawn task to wait for approval
        let guard_clone = guard.clone();
        let handle = tokio::spawn(async move {
            guard_clone
                .ask_for_approval(call.clone(), None, Some("/tmp/test.txt".to_string()))
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Check that permission request was created in the manager
        let perm_req = guard.get_permission_request("perm-test-1").await;
        assert!(perm_req.is_some(), "Permission request should be created");
        let perm_req = perm_req.unwrap();
        assert_eq!(perm_req.tool_name, "WriteTool");
        assert_eq!(perm_req.session_id, session_id);

        // Provide decision
        guard
            .provide_decision("perm-test-1", ApprovalDecision::Approve)
            .await;

        let decision = handle.await.unwrap();
        assert_eq!(decision, ApprovalDecision::Approve);

        // Verify the permission is now approved
        let perm_req = guard.get_permission_request("perm-test-1").await;
        assert!(perm_req.is_some());
        assert_eq!(perm_req.unwrap().state, PermissionState::Approved);
    }

    #[tokio::test]
    async fn test_guard_permission_manager_deny() {
        use crate::pipeline::permission::PermissionManager;
        use std::sync::Arc;

        let config = PermissionsConfig::default();
        let mgr = Arc::new(PermissionManager::new());
        let guard = CommandGuard::with_permission_manager(
            config,
            "test-workspace".to_string(),
            "test-session-deny".to_string(),
            mgr.clone(),
        );

        let call = ToolCall {
            id: "perm-test-deny".to_string(),
            name: "BashTool".to_string(),
            input: serde_json::json!({"cmd": "rm -rf /"}),
            status: ToolStatus::Pending,
            output: None,
            elapsed: None,
        };

        let guard_clone = guard.clone();
        let handle =
            tokio::spawn(
                async move { guard_clone.ask_for_approval(call.clone(), None, None).await },
            );

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Deny the request
        guard
            .provide_decision("perm-test-deny", ApprovalDecision::Deny)
            .await;

        let decision = handle.await.unwrap();
        assert_eq!(decision, ApprovalDecision::Deny);

        // Verify the permission is now denied
        let perm_req = guard.get_permission_request("perm-test-deny").await;
        assert!(perm_req.is_some());
        assert_eq!(perm_req.unwrap().state, PermissionState::Denied);
    }

    #[tokio::test]
    async fn test_guard_stale_decision_rejected() {
        use crate::pipeline::permission::PermissionManager;
        use std::sync::Arc;

        let config = PermissionsConfig::default();
        let mgr = Arc::new(PermissionManager::new());
        let guard = CommandGuard::with_permission_manager(
            config,
            "test-workspace".to_string(),
            "test-session-stale".to_string(),
            mgr.clone(),
        );

        let call = ToolCall {
            id: "perm-test-stale".to_string(),
            name: "WriteTool".to_string(),
            input: serde_json::Value::Null,
            status: ToolStatus::Pending,
            output: None,
            elapsed: None,
        };

        let guard_clone = guard.clone();
        let handle =
            tokio::spawn(
                async move { guard_clone.ask_for_approval(call.clone(), None, None).await },
            );

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Approve first time
        guard
            .provide_decision("perm-test-stale", ApprovalDecision::Approve)
            .await;

        let decision = handle.await.unwrap();
        assert_eq!(decision, ApprovalDecision::Approve);

        // Try to deny again - should not affect the already-decided request
        let found = guard
            .provide_decision("perm-test-stale", ApprovalDecision::Deny)
            .await;
        // The request was already removed from pending, so this returns false
        assert!(
            !found,
            "Stale decision should be rejected (request already removed)"
        );
    }

    #[tokio::test]
    async fn test_guard_decision_updates_permission_manager() {
        use crate::pipeline::permission::PermissionDecision as FormalDecision;
        use crate::pipeline::permission::PermissionManager;
        use crate::pipeline::permission::PermissionState;
        use std::sync::Arc;

        let config = PermissionsConfig::default();
        let mgr = Arc::new(PermissionManager::new());
        let session_id = "test-session-update".to_string();

        let guard = CommandGuard::with_permission_manager(
            config,
            "test-workspace".to_string(),
            session_id.clone(),
            mgr.clone(),
        );

        let call = ToolCall {
            id: "perm-test-update".to_string(),
            name: "WriteTool".to_string(),
            input: serde_json::Value::Null,
            status: ToolStatus::Pending,
            output: None,
            elapsed: None,
        };

        let guard_clone = guard.clone();
        let handle =
            tokio::spawn(
                async move { guard_clone.ask_for_approval(call.clone(), None, None).await },
            );

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Approve
        guard
            .provide_decision("perm-test-update", ApprovalDecision::Approve)
            .await;

        let _ = handle.await.unwrap();

        // Verify the permission manager has the updated state
        let pending = guard.get_pending_permission_requests().await;
        assert!(pending.is_empty(), "Approved request should not be pending");

        // Verify via manager directly
        let stats = mgr.stats().await;
        assert_eq!(stats.by_state.get("Approved"), Some(&1));
    }

    #[tokio::test]
    async fn test_determine_risk_level() {
        assert!(matches!(determine_risk_level("ReadTool"), RiskLevel::Low));
        assert!(matches!(determine_risk_level("GlobTool"), RiskLevel::Low));
        assert!(matches!(
            determine_risk_level("WriteTool"),
            RiskLevel::Medium
        ));
        assert!(matches!(
            determine_risk_level("EditTool"),
            RiskLevel::Medium
        ));
        assert!(matches!(
            determine_risk_level("DeleteTool"),
            RiskLevel::High
        ));
        assert!(matches!(determine_risk_level("BashTool"), RiskLevel::High));
        assert!(matches!(
            determine_risk_level("UnknownTool"),
            RiskLevel::Medium
        ));
    }
}
