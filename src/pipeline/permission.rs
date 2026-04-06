//! Formal Permission Request Lifecycle
//!
//! Provides a structured permission request model with explicit state transitions
//! for tool execution approvals. This formalizes the approval flow to be
//! interruption-safe and provides clear lifecycle tracking.
//!
//! # States
//!
//! ```text
//! Created → Pending → Approved → (executed)
//!                   → Denied
//!                   → Canceled
//! ```
//!
//! # Features
//!
//! - Explicit state transitions with validation
//! - Duplicate/stale response handling
//! - Request cancellation support
//! - Timeout handling
//! - Event emission for state changes

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Permission request lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    /// Request created, not yet submitted.
    Created,
    /// Request is awaiting user decision.
    Pending,
    /// Request was approved by user.
    Approved,
    /// Request was denied by user.
    Denied,
    /// Request was canceled (no longer relevant).
    Canceled,
    /// Request timed out waiting for decision.
    Expired,
}

impl PermissionState {
    /// Check if this is a terminal state (no further transitions allowed).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PermissionState::Approved
                | PermissionState::Denied
                | PermissionState::Canceled
                | PermissionState::Expired
        )
    }

    /// Check if execution can proceed from this state.
    pub fn can_execute(&self) -> bool {
        matches!(self, PermissionState::Approved)
    }

    /// Check if this is a blocking state (user is waiting).
    pub fn is_blocking(&self) -> bool {
        matches!(self, PermissionState::Pending)
    }

    /// Valid transitions from this state.
    pub fn valid_transitions(&self) -> &'static [PermissionState] {
        match self {
            PermissionState::Created => &[PermissionState::Pending, PermissionState::Canceled],
            PermissionState::Pending => &[
                PermissionState::Approved,
                PermissionState::Denied,
                PermissionState::Canceled,
                PermissionState::Expired,
            ],
            PermissionState::Approved
            | PermissionState::Denied
            | PermissionState::Canceled
            | PermissionState::Expired => &[],
        }
    }

    /// Check if transition to the given state is valid.
    pub fn can_transition_to(&self, target: PermissionState) -> bool {
        self.valid_transitions().contains(&target)
    }
}

impl std::fmt::Display for PermissionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionState::Created => write!(f, "created"),
            PermissionState::Pending => write!(f, "pending"),
            PermissionState::Approved => write!(f, "approved"),
            PermissionState::Denied => write!(f, "denied"),
            PermissionState::Canceled => write!(f, "canceled"),
            PermissionState::Expired => write!(f, "expired"),
        }
    }
}

/// Outcome of a permission decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    /// Approve the request.
    Approve,
    /// Deny the request.
    Deny,
    /// Cancel the request (no longer relevant).
    Cancel,
}

/// Risk level for the action being requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            RiskLevel::Low => "Read-only or safe operation",
            RiskLevel::Medium => "Modifies files or state",
            RiskLevel::High => "Potentially destructive or system-level",
            RiskLevel::Critical => "Security-sensitive or irreversible",
        }
    }
}

/// A formal permission request with full lifecycle tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionReq {
    /// Unique request identifier.
    pub id: String,
    /// Session this request belongs to.
    pub session_id: String,
    /// Tool name being requested.
    pub tool_name: String,
    /// Tool call ID from the agent.
    pub tool_call_id: String,
    /// Risk level of the action.
    pub risk_level: RiskLevel,
    /// Short summary of what the tool will do.
    pub summary: String,
    /// Detailed input summary.
    pub input_summary: String,
    /// Resource being accessed (e.g., file path).
    pub resource: Option<String>,
    /// Unified diff if file modification.
    pub diff: Option<String>,
    /// Reason/risk description for the user.
    pub reason: String,
    /// Current state.
    pub state: PermissionState,
    /// When the request was created.
    pub created_at: DateTime<Utc>,
    /// When the request transitioned to pending.
    pub pending_at: Option<DateTime<Utc>>,
    /// When the request was decided/canceled.
    pub decided_at: Option<DateTime<Utc>>,
    /// User feedback if provided.
    pub feedback: Option<String>,
    /// Number of times decision was attempted (for stale detection).
    pub decision_attempts: u32,
}

impl PermissionReq {
    /// Create a new permission request.
    pub fn new(
        session_id: String,
        tool_name: String,
        tool_call_id: String,
        risk_level: RiskLevel,
        summary: String,
        input_summary: String,
        resource: Option<String>,
        diff: Option<String>,
        reason: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            tool_name,
            tool_call_id,
            risk_level,
            summary,
            input_summary,
            resource,
            diff,
            reason,
            state: PermissionState::Created,
            created_at: Utc::now(),
            pending_at: None,
            decided_at: None,
            feedback: None,
            decision_attempts: 0,
        }
    }

    /// Get the request ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the current state.
    pub fn state(&self) -> PermissionState {
        self.state
    }

    /// Check if this request is still actionable.
    pub fn is_actionable(&self) -> bool {
        matches!(
            self.state,
            PermissionState::Created | PermissionState::Pending
        )
    }

    /// Transition to pending state.
    pub fn mark_pending(&mut self) -> Result<(), PermissionStateError> {
        self.transition_to(PermissionState::Pending)?;
        self.pending_at = Some(Utc::now());
        Ok(())
    }

    /// Apply a decision to this request.
    pub fn decide(
        &mut self,
        decision: PermissionDecision,
        feedback: Option<String>,
    ) -> Result<(), PermissionStateError> {
        self.decision_attempts += 1;

        let target_state = match decision {
            PermissionDecision::Approve => PermissionState::Approved,
            PermissionDecision::Deny => PermissionState::Denied,
            PermissionDecision::Cancel => PermissionState::Canceled,
        };

        self.transition_to(target_state)?;
        self.feedback = feedback;
        self.decided_at = Some(Utc::now());
        Ok(())
    }

    /// Build a directory-level cache entry from this approval.
    /// When a permission is approved for a file, this creates an entry
    /// that auto-approves future requests for any file in the same directory.
    pub fn to_directory_approval(
        &self,
        ttl: std::time::Duration,
    ) -> crate::pipeline::tool_permissions::CachedApproval {
        let resource = self
            .resource
            .as_deref()
            .map(crate::pipeline::tool_permissions::directory_prefix)
            .unwrap_or_default();

        crate::pipeline::tool_permissions::build_approval(
            &self.tool_name,
            if resource.is_empty() || resource == "/" {
                Some(&self.resource.as_deref().unwrap_or(""))
            } else {
                Some(&resource)
            },
            &format!("{:?}", self.risk_level),
            None,
            ttl,
        )
    }

    /// Build an exact-match cache entry from this approval.
    pub fn to_exact_approval(
        &self,
        ttl: std::time::Duration,
    ) -> crate::pipeline::tool_permissions::CachedApproval {
        crate::pipeline::tool_permissions::build_approval(
            &self.tool_name,
            self.resource.as_deref(),
            &format!("{:?}", self.risk_level),
            None,
            ttl,
        )
    }

    /// Mark as expired (timeout).
    pub fn expire(&mut self) -> Result<(), PermissionStateError> {
        self.transition_to(PermissionState::Expired)?;
        self.decided_at = Some(Utc::now());
        Ok(())
    }

    /// Internal state transition with validation.
    fn transition_to(&mut self, target: PermissionState) -> Result<(), PermissionStateError> {
        if !self.state.can_transition_to(target) {
            return Err(PermissionStateError::InvalidTransition {
                from: self.state,
                to: target,
            });
        }
        self.state = target;
        Ok(())
    }

    /// Check if this request is stale (decision attempted after terminal state).
    pub fn is_stale(&self, _decision: PermissionDecision) -> bool {
        self.state.is_terminal() && self.decision_attempts > 0
    }

    /// Get time spent in pending state.
    pub fn pending_duration(&self) -> Option<chrono::Duration> {
        self.pending_at.map(|pending| Utc::now() - pending)
    }
}

/// Errors related to permission state transitions.
#[derive(Debug, thiserror::Error)]
pub enum PermissionStateError {
    #[error("Invalid state transition from {from} to {to}")]
    InvalidTransition {
        from: PermissionState,
        to: PermissionState,
    },

    #[error("Request {id} not found")]
    NotFound { id: String },

    #[error("Request {id} is in terminal state {state}")]
    TerminalState { id: String, state: PermissionState },
}

/// Permission manager - handles the lifecycle of permission requests.
pub struct PermissionManager {
    /// Active requests by ID.
    requests: Arc<RwLock<HashMap<String, PermissionReq>>>,
    /// Requests by tool call ID (for correlation).
    by_tool_call: Arc<RwLock<HashMap<String, String>>>,
    /// Default timeout for pending requests.
    timeout: chrono::Duration,
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionManager {
    /// Create a new permission manager.
    pub fn new() -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            by_tool_call: Arc::new(RwLock::new(HashMap::new())),
            timeout: chrono::Duration::minutes(10),
        }
    }

    /// Create with custom timeout.
    pub fn with_timeout(timeout: chrono::Duration) -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            by_tool_call: Arc::new(RwLock::new(HashMap::new())),
            timeout,
        }
    }

    /// Create a new permission request.
    pub async fn create(
        &self,
        session_id: String,
        tool_name: String,
        tool_call_id: String,
        risk_level: RiskLevel,
        summary: String,
        input_summary: String,
        resource: Option<String>,
        diff: Option<String>,
        reason: String,
    ) -> PermissionReq {
        let req = PermissionReq::new(
            session_id,
            tool_name,
            tool_call_id,
            risk_level,
            summary,
            input_summary,
            resource,
            diff,
            reason,
        );

        let id = req.id.clone();
        let tool_call_id = req.tool_call_id.clone();

        let mut requests = self.requests.write().await;
        requests.insert(id.clone(), req.clone());

        let mut by_tool = self.by_tool_call.write().await;
        by_tool.insert(tool_call_id, id);

        debug!(request_id = %req.id, tool = %req.tool_name, "Permission request created");
        req
    }

    /// Submit a request for approval (mark as pending).
    pub async fn submit(&self, id: &str) -> Result<PermissionReq, PermissionStateError> {
        let mut requests = self.requests.write().await;
        let req = requests
            .get_mut(id)
            .ok_or_else(|| PermissionStateError::NotFound { id: id.to_string() })?;

        if !req.is_actionable() {
            return Err(PermissionStateError::TerminalState {
                id: id.to_string(),
                state: req.state,
            });
        }

        req.mark_pending()?;
        info!(request_id = %id, tool = %req.tool_name, "Permission request submitted");
        Ok(req.clone())
    }

    /// Submit by tool call ID.
    pub async fn submit_by_tool_call(
        &self,
        tool_call_id: &str,
    ) -> Result<PermissionReq, PermissionStateError> {
        let id = {
            let by_tool = self.by_tool_call.read().await;
            by_tool
                .get(tool_call_id)
                .cloned()
                .ok_or_else(|| PermissionStateError::NotFound {
                    id: tool_call_id.to_string(),
                })?
        };
        self.submit(&id).await
    }

    /// Record a decision for a request.
    pub async fn decide(
        &self,
        id: &str,
        decision: PermissionDecision,
        feedback: Option<String>,
    ) -> Result<PermissionReq, PermissionStateError> {
        let mut requests = self.requests.write().await;
        let req = requests
            .get_mut(id)
            .ok_or_else(|| PermissionStateError::NotFound { id: id.to_string() })?;

        // Check for stale decision (request already decided)
        if req.state.is_terminal() {
            warn!(
                request_id = %id,
                current_state = %req.state,
                "Stale decision attempted on terminal request"
            );
            return Err(PermissionStateError::TerminalState {
                id: id.to_string(),
                state: req.state,
            });
        }

        req.decide(decision, feedback)?;
        let final_state = req.state;
        info!(
            request_id = %id,
            decision = ?decision,
            final_state = %final_state,
            "Permission request decided"
        );

        Ok(req.clone())
    }

    /// Cancel a request (e.g., if no longer relevant).
    pub async fn cancel(&self, id: &str) -> Result<PermissionReq, PermissionStateError> {
        self.decide(id, PermissionDecision::Cancel, None).await
    }

    /// Cancel all pending requests for a session.
    pub async fn cancel_session(&self, session_id: &str) -> usize {
        let mut requests = self.requests.write().await;
        let mut canceled = 0;

        for req in requests.values_mut() {
            if req.session_id == session_id && req.is_actionable() {
                let _ = req.decide(
                    PermissionDecision::Cancel,
                    Some("Session ended".to_string()),
                );
                canceled += 1;
            }
        }

        debug!(session_id = %session_id, canceled = canceled, "Session pending requests canceled");
        canceled
    }

    /// Cancel requests by tool call ID.
    pub async fn cancel_by_tool_call(
        &self,
        tool_call_id: &str,
    ) -> Result<(), PermissionStateError> {
        let by_tool = self.by_tool_call.read().await;
        if let Some(id) = by_tool.get(tool_call_id) {
            let id = id.clone();
            drop(by_tool);
            self.cancel(&id).await?;
        }
        Ok(())
    }

    /// Get a request by ID.
    pub async fn get(&self, id: &str) -> Option<PermissionReq> {
        let requests = self.requests.read().await;
        requests.get(id).cloned()
    }

    /// Get request by tool call ID.
    pub async fn get_by_tool_call(&self, tool_call_id: &str) -> Option<PermissionReq> {
        let id = {
            let by_tool = self.by_tool_call.read().await;
            by_tool.get(tool_call_id).cloned()?
        };
        self.get(&id).await
    }

    /// Get all pending requests for a session.
    pub async fn pending_for_session(&self, session_id: &str) -> Vec<PermissionReq> {
        let requests = self.requests.read().await;
        requests
            .values()
            .filter(|r| r.session_id == session_id && r.state == PermissionState::Pending)
            .cloned()
            .collect()
    }

    /// Get all pending requests.
    pub async fn pending(&self) -> Vec<PermissionReq> {
        let requests = self.requests.read().await;
        requests
            .values()
            .filter(|r| r.state == PermissionState::Pending)
            .cloned()
            .collect()
    }

    /// Expire timed-out requests.
    pub async fn expire_timed_out(&self) -> usize {
        let timeout = self.timeout;
        let now = Utc::now();
        let mut requests = self.requests.write().await;
        let mut expired = 0;

        for req in requests.values_mut() {
            if req.state == PermissionState::Pending {
                if let Some(pending_at) = req.pending_at {
                    if now - pending_at > timeout {
                        let _ = req.expire();
                        expired += 1;
                    }
                }
            }
        }

        if expired > 0 {
            info!(expired = expired, "Expired timed-out permission requests");
        }

        expired
    }

    /// Clean up terminal requests older than the given duration.
    pub async fn cleanup_old(&self, max_age: chrono::Duration) -> usize {
        let cutoff = Utc::now() - max_age;
        let mut requests = self.requests.write().await;
        let mut removed = 0;

        let ids_to_remove: Vec<String> = requests
            .iter()
            .filter(|(_, r)| r.state.is_terminal() && r.decided_at.map_or(false, |d| d < cutoff))
            .map(|(id, _)| id.clone())
            .collect();

        for id in &ids_to_remove {
            if let Some(req) = requests.remove(id) {
                let mut by_tool = self.by_tool_call.write().await;
                by_tool.remove(&req.tool_call_id);
                removed += 1;
            }
        }

        debug!(removed = removed, "Cleaned up old permission requests");
        removed
    }

    /// Get request statistics.
    pub async fn stats(&self) -> PermissionStats {
        let requests = self.requests.read().await;

        let mut by_state: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut by_risk: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for req in requests.values() {
            *by_state.entry(format!("{:?}", req.state)).or_insert(0) += 1;
            *by_risk.entry(format!("{:?}", req.risk_level)).or_insert(0) += 1;
        }

        PermissionStats {
            total: requests.len(),
            by_state,
            by_risk,
        }
    }

    /// Check if a tool request should be auto-approved based on previously
    /// granted permissions in the context cache.
    ///
    /// Returns `Some(result)` if the cache matched (auto-approved or no match),
    /// the caller should use the result to decide whether to skip the full
    /// permission request flow.
    pub fn check_context(
        cache: &crate::pipeline::tool_permissions::ContextPermissionCache,
        tool_name: &str,
        resource: Option<&str>,
        project_path: Option<&str>,
    ) -> crate::pipeline::tool_permissions::ContextPermissionResult {
        cache.check(tool_name, resource, project_path)
    }
}

/// Statistics about permission requests.
#[derive(Debug, Clone, Default)]
pub struct PermissionStats {
    pub total: usize,
    pub by_state: std::collections::HashMap<String, usize>,
    pub by_risk: std::collections::HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_submit_request() {
        let mgr = PermissionManager::new();

        let req = mgr
            .create(
                "session-1".to_string(),
                "Write".to_string(),
                "tool-123".to_string(),
                RiskLevel::Medium,
                "Write to file".to_string(),
                "file_path: src/main.rs".to_string(),
                Some("src/main.rs".to_string()),
                None,
                "Writing file".to_string(),
            )
            .await;

        assert_eq!(req.state, PermissionState::Created);
        assert!(req.is_actionable());

        let submitted = mgr.submit(&req.id).await.unwrap();
        assert_eq!(submitted.state, PermissionState::Pending);
        assert!(submitted.pending_at.is_some());
    }

    #[tokio::test]
    async fn test_approve_request() {
        let mgr = PermissionManager::new();

        let req = mgr
            .create(
                "session-1".to_string(),
                "Bash".to_string(),
                "tool-456".to_string(),
                RiskLevel::High,
                "Run command".to_string(),
                "cmd: npm test".to_string(),
                None,
                None,
                "Running shell command".to_string(),
            )
            .await;

        mgr.submit(&req.id).await.unwrap();
        let decided = mgr
            .decide(&req.id, PermissionDecision::Approve, None)
            .await
            .unwrap();

        assert_eq!(decided.state, PermissionState::Approved);
        assert!(decided.state.can_execute());
        assert!(decided.decided_at.is_some());
    }

    #[tokio::test]
    async fn test_deny_request() {
        let mgr = PermissionManager::new();

        let req = mgr
            .create(
                "session-1".to_string(),
                "Bash".to_string(),
                "tool-789".to_string(),
                RiskLevel::Critical,
                "Run dangerous command".to_string(),
                "cmd: rm -rf /".to_string(),
                None,
                None,
                "Dangerous command detected".to_string(),
            )
            .await;

        mgr.submit(&req.id).await.unwrap();
        let decided = mgr
            .decide(
                &req.id,
                PermissionDecision::Deny,
                Some("Too risky".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(decided.state, PermissionState::Denied);
        assert!(!decided.state.can_execute());
        assert_eq!(decided.feedback, Some("Too risky".to_string()));
    }

    #[tokio::test]
    async fn test_cancel_request() {
        let mgr = PermissionManager::new();

        let req = mgr
            .create(
                "session-1".to_string(),
                "Write".to_string(),
                "tool-cancel".to_string(),
                RiskLevel::Low,
                "Write".to_string(),
                "file".to_string(),
                None,
                None,
                "Writing".to_string(),
            )
            .await;

        mgr.submit(&req.id).await.unwrap();
        let canceled = mgr.cancel(&req.id).await.unwrap();

        assert_eq!(canceled.state, PermissionState::Canceled);
        assert!(canceled.feedback.is_none()); // cancel() passes None for feedback
    }

    #[tokio::test]
    async fn test_stale_decision_rejected() {
        let mgr = PermissionManager::new();

        let req = mgr
            .create(
                "session-1".to_string(),
                "Write".to_string(),
                "tool-stale".to_string(),
                RiskLevel::Low,
                "Write".to_string(),
                "file".to_string(),
                None,
                None,
                "Writing".to_string(),
            )
            .await;

        mgr.submit(&req.id).await.unwrap();
        mgr.decide(&req.id, PermissionDecision::Approve, None)
            .await
            .unwrap();

        // Try to decide again - should fail
        let result = mgr.decide(&req.id, PermissionDecision::Deny, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_transition() {
        let mgr = PermissionManager::new();

        let req = mgr
            .create(
                "session-1".to_string(),
                "Write".to_string(),
                "tool-invalid".to_string(),
                RiskLevel::Low,
                "Write".to_string(),
                "file".to_string(),
                None,
                None,
                "Writing".to_string(),
            )
            .await;

        // Try to approve without submitting - should fail
        let result = mgr.decide(&req.id, PermissionDecision::Approve, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pending_for_session() {
        let mgr = PermissionManager::new();

        // Create requests for different sessions
        let req1 = mgr
            .create(
                "session-A".to_string(),
                "Write".to_string(),
                "tool-A1".to_string(),
                RiskLevel::Low,
                "Write 1".to_string(),
                "file1".to_string(),
                None,
                None,
                "Write 1".to_string(),
            )
            .await;

        let _req2 = mgr
            .create(
                "session-A".to_string(),
                "Write".to_string(),
                "tool-A2".to_string(),
                RiskLevel::Low,
                "Write 2".to_string(),
                "file2".to_string(),
                None,
                None,
                "Write 2".to_string(),
            )
            .await;

        let _req3 = mgr
            .create(
                "session-B".to_string(),
                "Write".to_string(),
                "tool-B1".to_string(),
                RiskLevel::Low,
                "Write 3".to_string(),
                "file3".to_string(),
                None,
                None,
                "Write 3".to_string(),
            )
            .await;

        // Submit only req1
        mgr.submit(&req1.id).await.unwrap();

        let pending_a = mgr.pending_for_session("session-A").await;
        assert_eq!(pending_a.len(), 1);

        let pending_b = mgr.pending_for_session("session-B").await;
        assert_eq!(pending_b.len(), 0);
    }

    #[tokio::test]
    async fn test_cancel_session() {
        let mgr = PermissionManager::new();

        let _req1 = mgr
            .create(
                "session-X".to_string(),
                "Bash".to_string(),
                "tool-X1".to_string(),
                RiskLevel::Medium,
                "Cmd 1".to_string(),
                "cmd".to_string(),
                None,
                None,
                "Command".to_string(),
            )
            .await;

        let _req2 = mgr
            .create(
                "session-X".to_string(),
                "Bash".to_string(),
                "tool-X2".to_string(),
                RiskLevel::Medium,
                "Cmd 2".to_string(),
                "cmd".to_string(),
                None,
                None,
                "Command".to_string(),
            )
            .await;

        let canceled = mgr.cancel_session("session-X").await;
        assert_eq!(canceled, 2);
    }

    #[tokio::test]
    async fn test_stats() {
        let mgr = PermissionManager::new();

        let _req = mgr
            .create(
                "session-1".to_string(),
                "Write".to_string(),
                "tool-stats".to_string(),
                RiskLevel::High,
                "Write".to_string(),
                "file".to_string(),
                None,
                None,
                "Writing".to_string(),
            )
            .await;

        mgr.submit(&_req.id).await.unwrap();

        let stats = mgr.stats().await;
        assert_eq!(stats.total, 1);
        assert_eq!(stats.by_state.get("Pending"), Some(&1));
        assert_eq!(stats.by_risk.get("High"), Some(&1));
    }

    #[tokio::test]
    async fn test_decision_attempts_tracked() {
        let mgr = PermissionManager::new();

        let req = mgr
            .create(
                "session-1".to_string(),
                "Bash".to_string(),
                "tool-attempts".to_string(),
                RiskLevel::Medium,
                "Cmd".to_string(),
                "cmd".to_string(),
                None,
                None,
                "Command".to_string(),
            )
            .await;

        assert_eq!(req.decision_attempts, 0);

        mgr.submit(&req.id).await.unwrap();
        let decided = mgr
            .decide(&req.id, PermissionDecision::Approve, None)
            .await
            .unwrap();

        assert_eq!(decided.decision_attempts, 1);
    }
}
