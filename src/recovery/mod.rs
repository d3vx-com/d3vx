//! Error recovery and resilience module
//!
//! Provides crash detection, session restoration, and escalation strategies.

pub mod crash_detector;
pub mod escalation;
pub mod idempotency;
pub mod session_restorer;
pub mod status;

pub use crash_detector::{CrashDetector, CrashStatus};
pub use escalation::{EscalationLevel, EscalationStrategy};
pub use idempotency::{IdempotencyError, IdempotencyGuard, OperationId, PerformedOperations};
pub use session_restorer::SessionRestorer;
pub use status::{HealthConfig, HealthIndicator, HealthIssue, RecoveryAction, RecoveryStatus};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_indicator_labels() {
        assert_eq!(HealthIndicator::Healthy.label(), "healthy");
        assert_eq!(HealthIndicator::Stuck.label(), "stuck");
        assert_eq!(HealthIndicator::Crashed.label(), "crashed");
        assert_eq!(HealthIndicator::Unknown.label(), "unknown");
    }

    #[test]
    fn test_health_indicator_needs_intervention() {
        assert!(!HealthIndicator::Healthy.needs_intervention());
        assert!(HealthIndicator::Stuck.needs_intervention());
        assert!(HealthIndicator::Crashed.needs_intervention());
        assert!(!HealthIndicator::Unknown.needs_intervention());
    }

    #[test]
    fn test_recovery_status_healthy() {
        let status = RecoveryStatus::healthy("test-session".to_string());
        assert_eq!(status.health, HealthIndicator::Healthy);
        assert!(!status.recovery_recommended);
        assert!(status.allows_merge());
    }

    #[test]
    fn test_operation_id() {
        let id = idempotency::OperationId::new("sess-1", "create_pr");
        assert_eq!(id.as_str(), "sess-1:create_pr");

        let id_with_key = id.with_key("pr-123");
        assert_eq!(id_with_key.as_str(), "sess-1:create_pr:pr-123");
    }

    #[tokio::test]
    async fn test_idempotency_guard() {
        let guard = IdempotencyGuard::new();

        // Can perform initially
        assert!(guard.can_perform("test_op").await);

        // Mark as completed
        guard.mark_completed("test_op").await;

        // Can't perform again
        assert!(!guard.can_perform("test_op").await);
    }

    #[tokio::test]
    async fn test_idempotency_failed_can_retry() {
        let guard = IdempotencyGuard::new();

        guard.mark_failed("test_op").await;
        assert!(guard.can_perform("test_op").await); // Can retry
    }
}
