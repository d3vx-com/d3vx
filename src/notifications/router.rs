//! Notification Routing System
//!
//! Routes notifications to appropriate channels based on priority.
//! Supports rate limiting, deduplication, and cooldown windows.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::types::{Notification, NotificationChannel, NotificationPriority};

/// Error type for routing operations.
#[derive(Debug, thiserror::Error)]
pub enum RoutingError {
    #[error("No channels configured for priority {priority:?}")]
    NoChannelsForPriority { priority: NotificationPriority },
    #[error("Rate limit exceeded for channel {channel:?}")]
    RateLimitExceeded { channel: String },
    #[error("Channel {channel} unavailable: {reason}")]
    ChannelUnavailable { channel: String, reason: String },
}

/// Configuration for a single route rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRule {
    /// Which priorities this rule handles.
    pub priorities: Vec<NotificationPriority>,
    /// Which channels to dispatch to.
    pub channels: Vec<NotificationChannel>,
    /// Max notifications per minute per channel (rate limit).
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_minute: u32,
    /// Minimum seconds between duplicate notifications (dedup window).
    #[serde(default = "default_cooldown_secs")]
    pub cooldown_secs: u64,
}

fn default_rate_limit() -> u32 {
    10
}

fn default_cooldown_secs() -> u64 {
    60
}

/// Full routing table configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Ordered list of routing rules (first match wins).
    pub rules: Vec<RouteRule>,
    /// Default channels if no rule matches.
    pub default_channels: Vec<NotificationChannel>,
    /// Global rate limit across all channels.
    #[serde(default = "default_rate_limit")]
    pub global_rate_limit: u32,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            rules: vec![
                RouteRule {
                    priorities: vec![NotificationPriority::Urgent],
                    channels: vec![NotificationChannel::Telegram {
                        bot_token_env: "D3VX_TELEGRAM_BOT_TOKEN".to_string(),
                        chat_id_env: "D3VX_TELEGRAM_CHAT_ID".to_string(),
                    }],
                    rate_limit_per_minute: 20,
                    cooldown_secs: 30,
                },
                RouteRule {
                    priorities: vec![NotificationPriority::Action, NotificationPriority::Warning],
                    channels: vec![NotificationChannel::Telegram {
                        bot_token_env: "D3VX_TELEGRAM_BOT_TOKEN".to_string(),
                        chat_id_env: "D3VX_TELEGRAM_CHAT_ID".to_string(),
                    }],
                    rate_limit_per_minute: 10,
                    cooldown_secs: 60,
                },
                RouteRule {
                    priorities: vec![NotificationPriority::Info],
                    channels: vec![NotificationChannel::Webhook {
                        url: "http://localhost:8080/notify".to_string(),
                        secret: None,
                    }],
                    rate_limit_per_minute: 5,
                    cooldown_secs: 300,
                },
            ],
            default_channels: vec![NotificationChannel::Telegram {
                bot_token_env: "D3VX_TELEGRAM_BOT_TOKEN".to_string(),
                chat_id_env: "D3VX_TELEGRAM_CHAT_ID".to_string(),
            }],
            global_rate_limit: 30,
        }
    }
}

/// Tracks per-channel state for rate limiting and deduplication.
#[derive(Debug, Clone)]
struct ChannelState {
    /// Timestamps of recent dispatches (for rate limiting, last minute only).
    dispatch_times: Vec<Instant>,
    /// Hash of notification title + priority -> last dispatch time (for dedup).
    event_digests: HashMap<String, Instant>,
}

impl ChannelState {
    fn new() -> Self {
        Self {
            dispatch_times: Vec::new(),
            event_digests: HashMap::new(),
        }
    }
}

/// Routes notifications to channels based on priority rules.
pub struct NotificationRouter {
    channels: Arc<RwLock<HashMap<NotificationChannel, ChannelState>>>,
    config: RoutingConfig,
}

impl NotificationRouter {
    /// Create a new router with the given configuration.
    pub fn new(config: RoutingConfig) -> Self {
        let mut channels = HashMap::new();
        for rule in &config.rules {
            for ch in &rule.channels {
                channels.entry(ch.clone()).or_insert_with(ChannelState::new);
            }
        }
        for ch in &config.default_channels {
            channels.entry(ch.clone()).or_insert_with(ChannelState::new);
        }

        Self {
            channels: Arc::new(RwLock::new(channels)),
            config,
        }
    }

    /// Resolve which channels should handle a given priority level.
    pub fn resolve_channels(&self, priority: &NotificationPriority) -> Vec<NotificationChannel> {
        for rule in &self.config.rules {
            if rule.priorities.contains(priority) {
                return rule.channels.clone();
            }
        }
        self.config.default_channels.clone()
    }

    /// Route a notification, returning which channels it should go to.
    ///
    /// Checks rate limits and dedup before returning channels.
    pub async fn route(
        &self,
        notification: Notification,
    ) -> Result<Vec<NotificationChannel>, RoutingError> {
        let channels = self.resolve_channels(&notification.priority);
        if channels.is_empty() {
            return Err(RoutingError::NoChannelsForPriority {
                priority: notification.priority,
            });
        }

        // Check dedup first (skip if it is a duplicate)
        if !self.check_dedup(&notification).await {
            debug!(
                title = %notification.title,
                "Notification deduplicated, skipping"
            );
            return Ok(vec![]);
        }

        // Check rate limits per channel
        let mut allowed = Vec::new();
        for ch in &channels {
            if self.check_rate_limit(ch).await {
                allowed.push(ch.clone());
            } else {
                warn!(channel = ?ch, "Rate limit exceeded, skipping channel");
            }
        }

        if allowed.is_empty() {
            return Err(RoutingError::RateLimitExceeded {
                channel: format!("{:?}", channels.first()),
            });
        }

        Ok(allowed)
    }

    /// Dispatch a notification: resolve channels, check limits, and send.
    pub async fn dispatch(&self, notification: Notification) -> Result<(), RoutingError> {
        let channels = self.route(notification.clone()).await?;

        for ch in &channels {
            debug!(
                channel = ?ch,
                title = %notification.title,
                priority = ?notification.priority,
                "Dispatching notification"
            );
            self.record_dispatch(ch.clone(), &notification).await;
            // Actual sending is future work; log dispatch for now.
        }

        Ok(())
    }

    /// Check if a channel is within its rate limit.
    async fn check_rate_limit(&self, channel: &NotificationChannel) -> bool {
        let state_map = self.channels.read().await;
        let Some(state) = state_map.get(channel) else {
            return true;
        };

        let now = Instant::now();
        let one_minute_ago = now - Duration::from_secs(60);
        let recent_count = state
            .dispatch_times
            .iter()
            .filter(|t| **t > one_minute_ago)
            .count();

        // Find the applicable rate limit from the rule or global
        let limit = self.rate_limit_for(channel) as usize;
        recent_count < limit
    }

    /// Check if a notification is not a duplicate (true = ok to send).
    async fn check_dedup(&self, notification: &Notification) -> bool {
        let digest = Self::notification_digest(notification);
        let state_map = self.channels.read().await;

        for state in state_map.values() {
            if let Some(last_sent) = state.event_digests.get(&digest) {
                let cooldown = self.cooldown_for(&notification.priority);
                if last_sent.elapsed() < Duration::from_secs(cooldown) {
                    return false;
                }
            }
        }
        true
    }

    /// Record a dispatch for rate limit and dedup tracking.
    async fn record_dispatch(&self, channel: NotificationChannel, notification: &Notification) {
        let digest = Self::notification_digest(notification);
        let mut state_map = self.channels.write().await;

        if let Some(state) = state_map.get_mut(&channel) {
            state.dispatch_times.push(Instant::now());
            state.event_digests.insert(digest, Instant::now());
        }
    }

    /// Remove stale entries: dispatch times older than 1 minute, digests older than 1 hour.
    pub async fn cleanup(&self) {
        let mut state_map = self.channels.write().await;
        let now = Instant::now();
        let one_minute_ago = now - Duration::from_secs(60);
        let one_hour_ago = now - Duration::from_secs(3600);

        for state in state_map.values_mut() {
            state.dispatch_times.retain(|t| *t > one_minute_ago);
            state.event_digests.retain(|_, t| *t > one_hour_ago);
        }
    }

    /// Compute a dedup digest key from notification title and priority.
    fn notification_digest(notification: &Notification) -> String {
        format!("{:?}:{}", notification.priority, notification.title)
    }

    /// Find the applicable rate limit for a channel.
    fn rate_limit_for(&self, channel: &NotificationChannel) -> u32 {
        for rule in &self.config.rules {
            if rule.channels.contains(channel) {
                return rule.rate_limit_per_minute;
            }
        }
        self.config.global_rate_limit
    }

    /// Find the applicable cooldown for a priority level.
    fn cooldown_for(&self, priority: &NotificationPriority) -> u64 {
        for rule in &self.config.rules {
            if rule.priorities.contains(priority) {
                return rule.cooldown_secs;
            }
        }
        default_cooldown_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_channel() -> NotificationChannel {
        NotificationChannel::Telegram {
            bot_token_env: "D3VX_TEST_TOKEN".to_string(),
            chat_id_env: "D3VX_TEST_CHAT".to_string(),
        }
    }

    #[test]
    fn test_default_config_has_three_rules() {
        let config = RoutingConfig::default();
        assert_eq!(config.rules.len(), 3);
    }

    #[test]
    fn test_resolve_channels_urgent() {
        let router = NotificationRouter::new(RoutingConfig::default());
        let channels = router.resolve_channels(&NotificationPriority::Urgent);
        assert_eq!(channels.len(), 1);
        assert!(matches!(channels[0], NotificationChannel::Telegram { .. }));
    }

    #[test]
    fn test_resolve_channels_info() {
        let router = NotificationRouter::new(RoutingConfig::default());
        let channels = router.resolve_channels(&NotificationPriority::Info);
        assert_eq!(channels.len(), 1);
        assert!(matches!(channels[0], NotificationChannel::Webhook { .. }));
    }

    #[tokio::test]
    async fn test_rate_limit_enforcement() {
        let ch = make_channel();
        let config = RoutingConfig {
            rules: vec![RouteRule {
                priorities: vec![NotificationPriority::Urgent],
                channels: vec![ch.clone()],
                rate_limit_per_minute: 2,
                cooldown_secs: 0,
            }],
            default_channels: vec![ch.clone()],
            global_rate_limit: 100,
        };
        let router = NotificationRouter::new(config);

        // Send 3 notifications; the 3rd should be rate-limited
        for _ in 0..3 {
            let n = Notification::urgent("Rate test", "body");
            let _ = router.dispatch(n).await;
        }

        // After 3 dispatches with rate_limit_per_minute=2, the 3rd was still
        // recorded but rate limiting kicks in on the *next* check.
        // Let us verify by calling route directly:
        let n = Notification::urgent("Rate test 4", "body");
        let result = router.route(n).await;
        // Should fail because rate limit (2/min) was exceeded by dispatches above
        assert!(result.is_err(), "Expected rate limit error");
    }

    #[tokio::test]
    async fn test_dedup_prevents_duplicate() {
        let ch = make_channel();
        let config = RoutingConfig {
            rules: vec![RouteRule {
                priorities: vec![NotificationPriority::Urgent],
                channels: vec![ch.clone()],
                rate_limit_per_minute: 100,
                cooldown_secs: 600,
            }],
            default_channels: vec![ch],
            global_rate_limit: 100,
        };
        let router = NotificationRouter::new(config);

        // First dispatch should succeed
        let n1 = Notification::urgent("Dedup test", "body");
        let result1 = router.route(n1.clone()).await;
        assert!(result1.is_ok());
        router
            .record_dispatch(result1.unwrap().into_iter().next().unwrap(), &n1)
            .await;

        // Second dispatch of same title/priority within cooldown should be deduped
        let n2 = Notification::urgent("Dedup test", "body");
        let result2 = router.route(n2).await;
        assert!(result2.is_ok());
        // Dedup returns empty channels
        assert!(
            result2.unwrap().is_empty(),
            "Expected dedup to return no channels"
        );
    }
}
