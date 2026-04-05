//! Notification System
//!
//! Priority-based notification dispatch via Telegram and webhooks.

pub mod router;
pub mod telegram;
pub mod types;

pub use router::{NotificationRouter, RouteRule, RoutingConfig, RoutingError};
pub use types::{Notification, NotificationChannel, NotificationPriority};
