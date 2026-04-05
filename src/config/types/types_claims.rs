//! Claims-based authorization types
//!
//! Provides wildcard-based permission patterns for fine-grained access control,
//! inspired by Ruflo's claims system. Claims use patterns like:
//! - `"tools:read"` -- exact match
//! - `"tools:*"` -- prefix wildcard (all actions under "tools")
//! - `"*:read"` -- suffix wildcard (read action for any category)
//! - `"*"` -- full wildcard (everything)

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A single permission claim that either grants or denies access.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claim {
    /// The claim pattern (e.g., "tools:read", "tools:*", "*:read", "*")
    pub pattern: String,
    /// Whether this claim grants (true) or denies (false) access
    #[serde(default = "default_granted")]
    pub granted: bool,
}

fn default_granted() -> bool {
    true
}

/// Claims-based role configuration mapping role names to claim lists.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ClaimsConfig {
    /// Map of role name to list of claims
    #[serde(default)]
    pub roles: HashMap<String, Vec<Claim>>,
}

/// Specificity level of a claim match (higher = more specific).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Specificity {
    /// Full wildcard: `"*"`
    FullWildcard = 0,
    /// Suffix wildcard: `"*:read"`
    SuffixWildcard = 1,
    /// Prefix wildcard: `"tools:*"`
    PrefixWildcard = 2,
    /// Exact match: `"tools:read"`
    Exact = 3,
}

/// Result of matching a claim pattern against an action.
struct MatchResult {
    specificity: Specificity,
    granted: bool,
}

/// Evaluate if a claim pattern matches an action string.
///
/// Supports:
/// - Exact match: `"tools:read"` matches `"tools:read"`
/// - Prefix wildcard: `"tools:*"` matches `"tools:read"`, `"tools:write"`
/// - Suffix wildcard: `"*:read"` matches `"tools:read"`, `"files:read"`
/// - Full wildcard: `"*"` matches everything
pub fn matches_claim(pattern: &str, action: &str) -> bool {
    let pattern = pattern.trim();
    let action = action.trim();

    if pattern == "*" {
        return true;
    }

    if pattern.contains(':') {
        let mut parts = pattern.splitn(2, ':');
        let prefix = parts.next().unwrap_or("");
        let suffix = parts.next().unwrap_or("");

        let mut action_parts = action.splitn(2, ':');
        let action_prefix = action_parts.next().unwrap_or("");
        let action_suffix = action_parts.next().unwrap_or("");

        // Prefix wildcard: "*:read"
        if prefix == "*" && suffix == action_suffix && !action_suffix.is_empty() {
            return true;
        }

        // Suffix wildcard: "tools:*"
        if suffix == "*" && prefix == action_prefix && !action_prefix.is_empty() {
            return true;
        }

        // Exact match: "tools:read"
        if prefix == action_prefix && suffix == action_suffix {
            return true;
        }
    } else {
        // No colon: treat as exact match on the whole string
        if pattern == action {
            return true;
        }
    }

    false
}

/// Classify the specificity of a claim pattern.
fn classify_specificity(pattern: &str) -> Specificity {
    let pattern = pattern.trim();

    if pattern == "*" {
        return Specificity::FullWildcard;
    }

    if !pattern.contains(':') {
        return Specificity::Exact;
    }

    let mut parts = pattern.splitn(2, ':');
    let prefix = parts.next().unwrap_or("");
    let suffix = parts.next().unwrap_or("");

    match (prefix, suffix) {
        ("*", "*") => Specificity::FullWildcard,
        ("*", _) => Specificity::SuffixWildcard,
        (_, "*") => Specificity::PrefixWildcard,
        (_, _) => Specificity::Exact,
    }
}

/// Evaluate all claims for a role and determine if an action is allowed.
///
/// Returns:
/// - `Some(true)` if a matching claim grants access
/// - `Some(false)` if a matching claim denies access
/// - `None` if no claim matches the action
///
/// When multiple claims match, the most specific one wins.
pub fn evaluate_claims(claims: &[Claim], action: &str) -> Option<bool> {
    let best = claims
        .iter()
        .filter_map(|claim| {
            if matches_claim(&claim.pattern, action) {
                Some(MatchResult {
                    specificity: classify_specificity(&claim.pattern),
                    granted: claim.granted,
                })
            } else {
                None
            }
        })
        .max_by_key(|m| m.specificity);

    best.map(|m| m.granted)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // matches_claim tests
    // =========================================================================

    #[test]
    fn test_exact_match() {
        assert!(matches_claim("tools:read", "tools:read"));
        assert!(!matches_claim("tools:read", "tools:write"));
    }

    #[test]
    fn test_prefix_wildcard() {
        assert!(matches_claim("tools:*", "tools:read"));
        assert!(matches_claim("tools:*", "tools:write"));
        assert!(matches_claim("tools:*", "tools:execute"));
        assert!(!matches_claim("tools:*", "files:read"));
    }

    #[test]
    fn test_suffix_wildcard() {
        assert!(matches_claim("*:read", "tools:read"));
        assert!(matches_claim("*:read", "files:read"));
        assert!(!matches_claim("*:read", "tools:write"));
    }

    #[test]
    fn test_full_wildcard() {
        assert!(matches_claim("*", "tools:read"));
        assert!(matches_claim("*", "anything:everything"));
        assert!(matches_claim("*", "solo"));
    }

    #[test]
    fn test_no_colon_exact() {
        assert!(matches_claim("read", "read"));
        assert!(!matches_claim("read", "write"));
    }

    #[test]
    fn test_empty_action() {
        assert!(!matches_claim("tools:read", ""));
        assert!(!matches_claim("*:read", ""));
    }

    #[test]
    fn test_empty_suffix_wildside() {
        // "*:" should not match "tools:read" because the suffix is empty
        assert!(!matches_claim("*:", "tools:read"));
        // "tools:" should not match "tools:read" because the suffix is empty
        assert!(!matches_claim("tools:", "tools:read"));
    }

    // =========================================================================
    // evaluate_claims specificity tests
    // =========================================================================

    #[test]
    fn test_specificity_exact_over_prefix_wildcard() {
        let claims = vec![
            Claim {
                pattern: "tools:*".to_string(),
                granted: true,
            },
            Claim {
                pattern: "tools:read".to_string(),
                granted: false,
            },
        ];

        // Exact match deny wins over prefix wildcard grant
        assert_eq!(evaluate_claims(&claims, "tools:read"), Some(false));
        // Prefix wildcard grant still applies to other actions
        assert_eq!(evaluate_claims(&claims, "tools:write"), Some(true));
    }

    #[test]
    fn test_specificity_prefix_over_full_wildcard() {
        let claims = vec![
            Claim {
                pattern: "*".to_string(),
                granted: true,
            },
            Claim {
                pattern: "tools:*".to_string(),
                granted: false,
            },
        ];

        // Prefix wildcard deny wins over full wildcard grant
        assert_eq!(evaluate_claims(&claims, "tools:read"), Some(false));
        // Full wildcard grant applies to other categories
        assert_eq!(evaluate_claims(&claims, "files:read"), Some(true));
    }

    #[test]
    fn test_deny_overrides_grant_when_more_specific() {
        let claims = vec![
            Claim {
                pattern: "*".to_string(),
                granted: true,
            },
            Claim {
                pattern: "tools:*".to_string(),
                granted: false,
            },
            Claim {
                pattern: "tools:read".to_string(),
                granted: true,
            },
        ];

        // Exact grant wins over prefix deny
        assert_eq!(evaluate_claims(&claims, "tools:read"), Some(true));
        // Prefix deny wins over full wildcard grant
        assert_eq!(evaluate_claims(&claims, "tools:write"), Some(false));
        // Full wildcard grant for other categories
        assert_eq!(evaluate_claims(&claims, "files:read"), Some(true));
    }

    #[test]
    fn test_no_matching_claims_returns_none() {
        let claims = vec![Claim {
            pattern: "tools:*".to_string(),
            granted: true,
        }];

        assert_eq!(evaluate_claims(&claims, "files:read"), None);
    }

    #[test]
    fn test_empty_claims_returns_none() {
        assert_eq!(evaluate_claims(&[], "tools:read"), None);
    }

    // =========================================================================
    // ClaimsConfig role lookup tests
    // =========================================================================

    #[test]
    fn test_missing_role_returns_none() {
        let config = ClaimsConfig {
            roles: HashMap::new(),
        };

        assert!(config.roles.get("nonexistent").is_none());
    }

    #[test]
    fn test_role_with_claims() {
        let mut config = ClaimsConfig::default();
        config.roles.insert(
            "executor".to_string(),
            vec![
                Claim {
                    pattern: "tools:*".to_string(),
                    granted: true,
                },
                Claim {
                    pattern: "tools:destructive".to_string(),
                    granted: false,
                },
            ],
        );

        let claims = config.roles.get("executor").unwrap();
        assert_eq!(evaluate_claims(claims, "tools:read"), Some(true));
        assert_eq!(evaluate_claims(claims, "tools:destructive"), Some(false));
    }

    // =========================================================================
    // Claim serde tests
    // =========================================================================

    #[test]
    fn test_claim_serde_roundtrip() {
        let claim = Claim {
            pattern: "tools:read".to_string(),
            granted: false,
        };
        let json = serde_json::to_string(&claim).unwrap();
        let parsed: Claim = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, claim);
    }

    #[test]
    fn test_claims_config_serde_roundtrip() {
        let mut config = ClaimsConfig::default();
        config.roles.insert(
            "tech_lead".to_string(),
            vec![Claim {
                pattern: "*".to_string(),
                granted: true,
            }],
        );

        let json = serde_json::to_string(&config).unwrap();
        let parsed: ClaimsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, config);
    }

    // =========================================================================
    // classify_specificity tests
    // =========================================================================

    #[test]
    fn test_specificity_ordering() {
        assert!(Specificity::Exact > Specificity::PrefixWildcard);
        assert!(Specificity::PrefixWildcard > Specificity::SuffixWildcard);
        assert!(Specificity::SuffixWildcard > Specificity::FullWildcard);
    }
}
