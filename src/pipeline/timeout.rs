//! Pipeline Timeout Handler
//!
//! Provides timeout management with graceful cancellation for pipeline phases.
//! Follows Single Responsibility Principle - only handles timeout logic.

use std::time::{Duration, Instant};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::handlers::PhaseError;
use super::phases::Phase;

/// Timeout configuration for pipeline phases
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Default timeout for each phase
    pub default_phase_timeout: Duration,
    /// Phase-specific timeouts
    pub phase_timeouts: std::collections::HashMap<Phase, Duration>,
    /// Enable graceful cancellation
    pub enable_graceful_cancellation: bool,
    /// Grace period for cleanup after timeout
    pub cleanup_grace_period: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        let mut phase_timeouts = std::collections::HashMap::new();

        // Research can take longer (reading files, analyzing codebase)
        phase_timeouts.insert(Phase::Research, Duration::from_secs(300)); // 5 minutes

        // Planning is relatively quick
        phase_timeouts.insert(Phase::Plan, Duration::from_secs(180)); // 3 minutes

        // Implementation can take the longest
        phase_timeouts.insert(Phase::Implement, Duration::from_secs(600)); // 10 minutes

        // Review is moderate
        phase_timeouts.insert(Phase::Review, Duration::from_secs(240)); // 4 minutes

        // Documentation is relatively quick
        phase_timeouts.insert(Phase::Docs, Duration::from_secs(180)); // 3 minutes

        Self {
            default_phase_timeout: Duration::from_secs(300), // 5 minutes default
            phase_timeouts,
            enable_graceful_cancellation: true,
            cleanup_grace_period: Duration::from_secs(10),
        }
    }
}

impl TimeoutConfig {
    /// Create a new timeout config with custom default
    pub fn with_default_timeout(default: Duration) -> Self {
        Self {
            default_phase_timeout: default,
            ..Default::default()
        }
    }

    /// Set timeout for a specific phase
    pub fn with_phase_timeout(mut self, phase: Phase, duration: Duration) -> Self {
        self.phase_timeouts.insert(phase, duration);
        self
    }

    /// Get timeout for a specific phase
    pub fn get_timeout(&self, phase: Phase) -> Duration {
        self.phase_timeouts
            .get(&phase)
            .copied()
            .unwrap_or(self.default_phase_timeout)
    }
}

/// Manages timeouts for pipeline execution
pub struct TimeoutManager {
    /// Configuration
    config: TimeoutConfig,
    /// Cancellation token for graceful shutdown
    cancellation_token: CancellationToken,
    /// Start time of current operation
    start_time: Option<Instant>,
}

impl TimeoutManager {
    /// Create a new timeout manager with default configuration
    pub fn new() -> Self {
        Self::with_config(TimeoutConfig::default())
    }

    /// Create a new timeout manager with custom configuration
    pub fn with_config(config: TimeoutConfig) -> Self {
        Self {
            config,
            cancellation_token: CancellationToken::new(),
            start_time: None,
        }
    }

    /// Get the cancellation token for child tasks
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Request graceful cancellation
    pub fn cancel(&self) {
        if !self.cancellation_token.is_cancelled() {
            info!("Timeout cancellation requested");
            self.cancellation_token.cancel();
        }
    }

    /// Start timing an operation
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        debug!("Started timeout timer");
    }

    /// Get elapsed time since start
    pub fn elapsed(&self) -> Option<Duration> {
        self.start_time.map(|t| t.elapsed())
    }

    /// Check if the current operation has exceeded its timeout
    pub fn has_timed_out(&self, phase: Phase) -> bool {
        if let Some(elapsed) = self.elapsed() {
            let timeout_duration = self.config.get_timeout(phase);
            elapsed > timeout_duration
        } else {
            false
        }
    }

    /// Get remaining time for a phase
    pub fn remaining_time(&self, phase: Phase) -> Option<Duration> {
        if let Some(elapsed) = self.elapsed() {
            let timeout_duration = self.config.get_timeout(phase);
            if elapsed < timeout_duration {
                Some(timeout_duration - elapsed)
            } else {
                Some(Duration::from_secs(0))
            }
        } else {
            None
        }
    }

    /// Reset the timer for a new operation
    pub fn reset(&mut self) {
        self.start_time = None;
        // Create a new cancellation token
        self.cancellation_token = CancellationToken::new();
        debug!("Reset timeout timer");
    }

    /// Execute a future with timeout and graceful cancellation
    pub async fn execute_with_timeout<F, T>(
        &mut self,
        phase: Phase,
        future: F,
    ) -> Result<T, PhaseError>
    where
        F: std::future::Future<Output = Result<T, PhaseError>>,
    {
        self.start();

        let timeout_duration = self.config.get_timeout(phase);
        let token = self.cancellation_token.clone();

        debug!(
            "Executing phase {} with timeout of {:?}",
            phase, timeout_duration
        );

        // Wrap the future with timeout
        let result = timeout(timeout_duration, async {
            // Check for cancellation periodically
            tokio::select! {
                result = future => {
                    result
                }
                _ = token.cancelled() => {
                    warn!("Phase {} cancelled", phase);
                    Err(PhaseError::Cancelled)
                }
            }
        })
        .await;

        match result {
            Ok(Ok(value)) => {
                debug!("Phase {} completed successfully", phase);
                Ok(value)
            }
            Ok(Err(e)) => {
                error!("Phase {} failed: {}", phase, e);
                Err(e)
            }
            Err(_) => {
                // Timeout occurred
                error!("Phase {} timed out after {:?}", phase, timeout_duration);

                if self.config.enable_graceful_cancellation {
                    self.cancel();

                    // Give cleanup operations time to complete
                    tokio::time::sleep(self.config.cleanup_grace_period).await;
                }

                Err(PhaseError::Timeout {
                    timeout_ms: timeout_duration.as_millis() as u64,
                })
            }
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &TimeoutConfig {
        &self.config
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: TimeoutConfig) {
        self.config = config;
    }
}

impl Default for TimeoutManager {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard for tracking operation duration
pub struct DurationGuard {
    phase: Phase,
    start: Instant,
    on_complete: Option<Box<dyn FnOnce(Phase, Duration) + Send>>,
}

impl DurationGuard {
    /// Create a new duration guard
    pub fn new(phase: Phase) -> Self {
        Self {
            phase,
            start: Instant::now(),
            on_complete: None,
        }
    }

    /// Set callback for when the guard is dropped
    pub fn on_complete<F>(mut self, callback: F) -> Self
    where
        F: FnOnce(Phase, Duration) + Send + 'static,
    {
        self.on_complete = Some(Box::new(callback));
        self
    }
}

impl Drop for DurationGuard {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        debug!("Phase {} completed in {:?}", self.phase, duration);

        if let Some(callback) = self.on_complete.take() {
            callback(self.phase, duration);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[test]
    fn test_timeout_config_default() {
        let config = TimeoutConfig::default();

        assert_eq!(
            config.get_timeout(Phase::Research),
            Duration::from_secs(300)
        );
        assert_eq!(config.get_timeout(Phase::Plan), Duration::from_secs(180));
        assert_eq!(
            config.get_timeout(Phase::Implement),
            Duration::from_secs(600)
        );
    }

    #[test]
    fn test_timeout_config_custom() {
        let config =
            TimeoutConfig::default().with_phase_timeout(Phase::Research, Duration::from_secs(120));

        assert_eq!(
            config.get_timeout(Phase::Research),
            Duration::from_secs(120)
        );
        assert_eq!(config.get_timeout(Phase::Plan), Duration::from_secs(180)); // Unchanged
    }

    #[tokio::test]
    async fn test_timeout_manager_success() {
        let mut manager = TimeoutManager::new();

        let future = async {
            sleep(Duration::from_millis(10)).await;
            Ok::<i32, PhaseError>(42)
        };

        let result = manager.execute_with_timeout(Phase::Research, future).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_timeout_manager_timeout() {
        let mut phase_timeouts = std::collections::HashMap::new();
        phase_timeouts.insert(Phase::Research, Duration::from_millis(50));
        let config = TimeoutConfig {
            default_phase_timeout: Duration::from_millis(50),
            phase_timeouts,
            cleanup_grace_period: Duration::from_millis(10),
            ..Default::default()
        };
        let mut manager = TimeoutManager::with_config(config);

        let future = async {
            sleep(Duration::from_secs(10)).await; // Way too long
            Ok::<i32, PhaseError>(42)
        };

        let result = manager.execute_with_timeout(Phase::Research, future).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, PhaseError::Timeout { .. }));
    }

    #[tokio::test]
    async fn test_timeout_manager_cancellation() {
        let mut manager = TimeoutManager::new();
        let token = manager.cancellation_token();

        // Cancel immediately
        tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            token.cancel();
        });

        let future = async {
            loop {
                sleep(Duration::from_millis(100)).await;
            }
            #[allow(unreachable_code)]
            Ok::<i32, PhaseError>(42)
        };

        let result = manager.execute_with_timeout(Phase::Research, future).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PhaseError::Cancelled));
    }

    #[test]
    fn test_timeout_manager_elapsed() {
        let mut manager = TimeoutManager::new();
        assert!(manager.elapsed().is_none());

        manager.start();
        std::thread::sleep(Duration::from_millis(50));

        let elapsed = manager.elapsed().unwrap();
        assert!(elapsed >= Duration::from_millis(50));
    }

    #[test]
    fn test_timeout_manager_remaining_time() {
        let mut manager = TimeoutManager::new();
        manager.start();

        std::thread::sleep(Duration::from_millis(100));

        let remaining = manager.remaining_time(Phase::Research).unwrap();
        assert!(remaining < Duration::from_secs(300));
        assert!(remaining > Duration::from_secs(0));
    }

    #[tokio::test]
    async fn test_duration_guard() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;

        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        {
            let guard = DurationGuard::new(Phase::Research).on_complete(move |phase, duration| {
                assert_eq!(phase, Phase::Research);
                counter_clone.fetch_add(duration.as_millis() as u64, Ordering::SeqCst);
            });

            sleep(Duration::from_millis(50)).await;
            drop(guard);
        }

        let value = counter.load(Ordering::SeqCst);
        assert!(value >= 50);
    }

    #[test]
    fn test_timeout_manager_reset() {
        let mut manager = TimeoutManager::new();
        manager.start();

        let token1 = manager.cancellation_token();
        assert!(!token1.is_cancelled());

        manager.reset();

        let token2 = manager.cancellation_token();
        assert!(!token2.is_cancelled());

        // Old token should be different from new token
        token1.cancel();
        assert!(token1.is_cancelled());
        assert!(!token2.is_cancelled());
    }
}
