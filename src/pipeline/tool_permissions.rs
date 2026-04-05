//! Context-Aware Tool Permissions
//!
//! Auto-approves tool requests based on previously granted permissions:
//!
//! - **Same resource**: If `Write src/main.rs` was approved <N> mins ago, auto-approve
//! - **Same directory**: If `Write src/auth/` was approved, `Write src/auth/tokens.rs` auto-approves
//! - **Plan pre-approval**: Files modified during plan phase are pre-approved for implement phase
//! - **Session scope**: Approvals persist across restarts in the persistent SQLite cache
//!
//! ## How It Works
//!
//! ```text
//! Tool request arrives
//!   → Check in-memory cache (fast path)
//!   → Check SQLite persistent cache (restart-safe)
//!   → If matched & not expired → auto-approve
//!   → If no match → fall through to user prompt
//! ```
//!
//! ## Expiry
//!
//! Approvals have a TTL (default 30 min). After expiry, the question is
//! re-asked to the user. Directory-level approvals have a shorter TTL
//! (15 min) since they cover more files.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// The reason an auto-approval matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoApproveReason {
    /// Same tool + exact resource approved recently
    ExactMatch,
    /// Same tool + parent directory approved recently
    DirectoryMatch,
    /// Resource was pre-approved during plan phase
    PlanPreApproved,
    /// Tool is on the permanent allowlist
    Allowlist,
}

/// Result of checking context-aware permissions.
#[derive(Debug, Clone)]
pub struct ContextPermissionResult {
    pub approved: bool,
    pub reason: Option<AutoApproveReason>,
    pub cached_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ContextPermissionResult {
    pub fn auto_approved(
        reason: AutoApproveReason,
        cached_at: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            approved: true,
            reason: Some(reason),
            cached_at: Some(cached_at),
        }
    }

    pub fn not_matched() -> Self {
        Self {
            approved: false,
            reason: None,
            cached_at: None,
        }
    }
}

/// Configuration for the context permission cache.
#[derive(Debug, Clone)]
pub struct ContextPermissionConfig {
    /// TTL for exact-match approvals
    pub exact_match_ttl: Duration,
    /// TTL for directory-match approvals
    pub directory_match_ttl: Duration,
    /// Max cache entries before cleanup
    pub max_entries: usize,
    /// Tools that are permanently allowed (no questions ever)
    pub permanent_allowlist: Vec<String>,
}

impl Default for ContextPermissionConfig {
    fn default() -> Self {
        Self {
            exact_match_ttl: Duration::from_secs(30 * 60),
            directory_match_ttl: Duration::from_secs(15 * 60),
            max_entries: 1000,
            permanent_allowlist: vec!["Read".to_string(), "Glob".to_string(), "Grep".to_string()],
        }
    }
}

/// A cached approval entry (for SQLite persistence).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedApproval {
    /// Tool name (e.g., "Write", "Bash")
    pub tool_name: String,
    /// Resource path or prefix (e.g., "src/main.rs", "src/auth/")
    pub resource: String,
    /// Whether this is a directory-level approval
    pub is_directory: bool,
    /// Risk level that was approved
    pub risk_level: String,
    /// When the user approved this
    pub approved_at: chrono::DateTime<chrono::Utc>,
    /// When this cache entry expires
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Project this applies to (empty = all projects)
    pub project_path: Option<String>,
}

/// In-memory cache for fast permission lookups.
pub struct ContextPermissionCache {
    config: ContextPermissionConfig,
    in_memory: Vec<CachedApproval>,
}

impl ContextPermissionCache {
    pub fn new(config: ContextPermissionConfig) -> Self {
        Self {
            config,
            in_memory: Vec::new(),
        }
    }

    /// Check if a tool request should be auto-approved.
    pub fn check(
        &self,
        tool_name: &str,
        resource: Option<&str>,
        _project_path: Option<&str>,
    ) -> ContextPermissionResult {
        // 1. Check permanent allowlist
        if self
            .config
            .permanent_allowlist
            .contains(&tool_name.to_string())
        {
            return ContextPermissionResult::auto_approved(
                AutoApproveReason::Allowlist,
                chrono::Utc::now(),
            );
        }

        let Some(r) = resource else {
            return ContextPermissionResult::not_matched();
        };

        let now = chrono::Utc::now();

        // 2. Check exact match
        for entry in &self.in_memory {
            if entry.tool_name != tool_name {
                continue;
            }
            if !entry.is_directory && entry.resource == r {
                if now < entry.expires_at {
                    return ContextPermissionResult::auto_approved(
                        AutoApproveReason::ExactMatch,
                        entry.approved_at,
                    );
                }
            }
        }

        // 3. Check directory match
        for entry in &self.in_memory {
            if entry.tool_name != tool_name || !entry.is_directory {
                continue;
            }
            if now >= entry.expires_at {
                continue;
            }
            if r.starts_with(&entry.resource) {
                return ContextPermissionResult::auto_approved(
                    AutoApproveReason::DirectoryMatch,
                    entry.approved_at,
                );
            }
        }

        ContextPermissionResult::not_matched()
    }

    /// Record a new approval in the cache.
    pub fn record_approval(&mut self, approval: CachedApproval) {
        // Evict expired
        let now = chrono::Utc::now();
        self.in_memory.retain(|e| e.expires_at > now);

        // Avoid duplicates
        self.in_memory.retain(|e| {
            !(e.tool_name == approval.tool_name
                && e.resource == approval.resource
                && e.is_directory == approval.is_directory)
        });

        self.in_memory.push(approval);

        // Cap size
        if self.in_memory.len() > self.config.max_entries {
            self.in_memory
                .drain(0..self.in_memory.len() - self.config.max_entries);
        }
    }

    /// Get all cached entries for persistence to SQLite.
    pub fn entries(&self) -> &[CachedApproval] {
        &self.in_memory
    }

    /// Load entries from SQLite (e.g., after restart).
    pub fn load_entries(&mut self, entries: Vec<CachedApproval>) {
        let now = chrono::Utc::now();
        self.in_memory = entries.into_iter().filter(|e| e.expires_at > now).collect();
    }

    /// Clear all cache entries (e.g., on session end).
    pub fn clear(&mut self) {
        self.in_memory.clear();
    }

    /// Count active entries.
    pub fn len(&self) -> usize {
        self.in_memory.len()
    }
}

/// Compute the directory prefix for a resource path.
/// e.g., "src/auth/tokens.rs" → "src/auth/"
pub fn directory_prefix(resource: &str) -> String {
    let path = Path::new(resource);
    if let Some(parent) = path.parent() {
        let parent_str = parent.to_string_lossy();
        if parent_str.is_empty() {
            String::new()
        } else {
            format!("{}/", parent_str)
        }
    } else {
        String::new()
    }
}

/// Build a cache entry for an approval.
pub fn build_approval(
    tool_name: &str,
    resource: Option<&str>,
    risk_level: &str,
    project_path: Option<&str>,
    ttl: Duration,
) -> CachedApproval {
    let resource_str = resource.unwrap_or("").to_string();
    let (resource, is_directory) = if resource_str.ends_with('/') {
        (resource_str.clone(), true)
    } else {
        (resource_str.clone(), false)
    };

    let now = chrono::Utc::now();
    let expires = now + chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::minutes(30));

    CachedApproval {
        tool_name: tool_name.to_string(),
        resource,
        is_directory,
        risk_level: risk_level.to_string(),
        approved_at: now,
        expires_at: expires,
        project_path: project_path.map(|s| s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cache() -> ContextPermissionCache {
        ContextPermissionCache::new(ContextPermissionConfig::default())
    }

    #[test]
    fn test_permanent_allowlist() {
        let cache = make_cache();
        let result = cache.check("Read", Some("secret.env"), None);
        assert!(result.approved);
        assert_eq!(result.reason, Some(AutoApproveReason::Allowlist));
    }

    #[test]
    fn test_exact_match_auto_approve() {
        let mut cache = make_cache();
        let approval = build_approval(
            "Write",
            Some("src/main.rs"),
            "Medium",
            None,
            Duration::from_secs(30 * 60),
        );
        cache.record_approval(approval);

        let result = cache.check("Write", Some("src/main.rs"), None);
        assert!(result.approved);
        assert_eq!(result.reason, Some(AutoApproveReason::ExactMatch));
    }

    #[test]
    fn test_directory_match_auto_approve() {
        let mut cache = make_cache();
        let approval = build_approval(
            "Write",
            Some("src/auth/"),
            "High",
            None,
            Duration::from_secs(15 * 60),
        );
        cache.record_approval(approval);

        let result = cache.check("Write", Some("src/auth/tokens.rs"), None);
        assert!(result.approved);
        assert_eq!(result.reason, Some(AutoApproveReason::DirectoryMatch));
    }

    #[test]
    fn test_no_match_returns_not_matched() {
        let cache = make_cache();
        let result = cache.check("Bash", Some("rm -rf /"), None);
        assert!(!result.approved);
        assert!(result.reason.is_none());
    }

    #[test]
    fn test_expired_entry_not_matched() {
        let mut cache = make_cache();
        let mut approval = build_approval(
            "Write",
            Some("src/main.rs"),
            "Medium",
            None,
            Duration::from_secs(1),
        );
        approval.approved_at = chrono::Utc::now() - chrono::Duration::minutes(31);
        approval.expires_at = chrono::Utc::now() - chrono::Duration::minutes(1);
        cache.record_approval(approval);

        let result = cache.check("Write", Some("src/main.rs"), None);
        assert!(!result.approved);
    }

    #[test]
    fn test_directory_prefix() {
        assert_eq!(directory_prefix("src/auth/tokens.rs"), "src/auth/");
        assert_eq!(directory_prefix("Cargo.toml"), "");
        assert_eq!(directory_prefix("src/lib.rs"), "src/");
    }

    #[test]
    fn test_cache_deduplication() {
        let mut cache = make_cache();
        let a1 = build_approval(
            "Write",
            Some("src/main.rs"),
            "Medium",
            None,
            Duration::from_secs(30 * 60),
        );
        let a2 = build_approval(
            "Write",
            Some("src/main.rs"),
            "High",
            None,
            Duration::from_secs(60 * 60),
        );
        cache.record_approval(a1);
        cache.record_approval(a2);

        assert_eq!(cache.len(), 1);
    }
}
