//! Channel-based real-time message routing between team members.
//!
//! Each member registers an `mpsc` receiver handle.  The bus routes
//! point-to-point messages to the correct channel and broadcasts `"*"`
//! messages via a Tokio `broadcast` sender.

use anyhow::{bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::debug;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A message routed between team members.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmMessage {
    pub id: String,
    pub from_call_sign: String,
    pub to_call_sign: String, // "*" for broadcast
    pub body: String,
    pub timestamp: String, // ISO 3339
    pub message_type: SwarmMessageType,
}

/// Types of messages that can be sent between team members.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SwarmMessageType {
    Text,
    TaskClaim,
    TaskComplete,
    ShutdownRequest,
    ShutdownResponse { approve: bool },
    StatusUpdate,
}

// ---------------------------------------------------------------------------
// SwarmMessage helpers
// ---------------------------------------------------------------------------

impl SwarmMessage {
    /// Create a plain text message.
    pub fn new_text(from: &str, to: &str, body: &str) -> Self {
        Self::new(from, to, body.to_string(), SwarmMessageType::Text)
    }

    /// Create a shutdown-request message.
    pub fn new_shutdown_request(from: &str, to: &str) -> Self {
        Self::new(from, to, String::new(), SwarmMessageType::ShutdownRequest)
    }

    /// Create a shutdown-response message.
    pub fn new_shutdown_response(from: &str, to: &str, approve: bool) -> Self {
        Self::new(
            from,
            to,
            String::new(),
            SwarmMessageType::ShutdownResponse { approve },
        )
    }

    /// Shared constructor.
    pub fn new(from: &str, to: &str, body: String, message_type: SwarmMessageType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from_call_sign: from.to_string(),
            to_call_sign: to.to_string(),
            body,
            timestamp: Utc::now().to_rfc3339(),
            message_type,
        }
    }
}

// ---------------------------------------------------------------------------
// MessageBus
// ---------------------------------------------------------------------------

const BROADCAST_CAPACITY: usize = 256;

/// Routes messages between registered team members.
#[derive(Clone)]
pub struct MessageBus {
    channels: Arc<RwLock<HashMap<String, mpsc::Sender<SwarmMessage>>>>,
    broadcast_tx: broadcast::Sender<SwarmMessage>,
}

impl MessageBus {
    /// Create an empty bus with no registered members.
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
        }
    }

    /// Register a member's receive channel under its call sign.
    pub async fn register(&self, call_sign: &str, tx: mpsc::Sender<SwarmMessage>) {
        debug!(call_sign = call_sign, "registering member on message bus");
        self.channels
            .write()
            .await
            .insert(call_sign.to_string(), tx);
    }

    /// Remove a member's channel from the bus.
    pub async fn unregister(&self, call_sign: &str) {
        debug!(
            call_sign = call_sign,
            "unregistering member from message bus"
        );
        self.channels.write().await.remove(call_sign);
    }

    /// Route a message to its recipient.
    ///
    /// If `to_call_sign == "*"` the message is broadcast to all subscribers.
    /// Otherwise it is sent to the specific member's channel.
    pub async fn send(&self, message: SwarmMessage) -> Result<()> {
        debug!(
            id = %message.id,
            from = %message.from_call_sign,
            to = %message.to_call_sign,
            ty = ?message.message_type,
            "routing message"
        );

        if message.to_call_sign == "*" {
            // Broadcast — ignore the result; zero subscribers is fine.
            let _ = self.broadcast_tx.send(message);
            return Ok(());
        }

        let channels = self.channels.read().await;
        match channels.get(&message.to_call_sign) {
            Some(tx) => {
                tx.send(message).await?;
                Ok(())
            }
            None => bail!("recipient '{}' not registered", message.to_call_sign),
        }
    }

    /// Obtain a new broadcast receiver (used by the lead to hear all traffic).
    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<SwarmMessage> {
        self.broadcast_tx.subscribe()
    }

    /// Return all currently registered call signs.
    pub async fn list_registered(&self) -> Vec<String> {
        let channels = self.channels.read().await;
        channels.keys().cloned().collect()
    }

    /// Check whether a call sign is registered.
    pub async fn is_registered(&self, call_sign: &str) -> bool {
        self.channels.read().await.contains_key(call_sign)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_and_send_to_specific_member() {
        let bus = MessageBus::new();
        let (tx, mut rx) = mpsc::channel::<SwarmMessage>(16);
        bus.register("bravo", tx).await;

        let msg = SwarmMessage::new_text("alpha", "bravo", "hello");
        bus.send(msg.clone()).await.expect("send");

        let received = rx.recv().await.expect("recv");
        assert_eq!(received.id, msg.id);
        assert_eq!(received.body, "hello");
        assert_eq!(received.from_call_sign, "alpha");
        assert_eq!(received.to_call_sign, "bravo");
    }

    #[tokio::test]
    async fn broadcast_sends_to_subscribers() {
        let bus = MessageBus::new();
        let mut sub = bus.subscribe_broadcast();

        let msg = SwarmMessage::new_text("alpha", "*", "everyone");
        bus.send(msg.clone()).await.expect("send broadcast");

        let received = sub.recv().await.expect("recv broadcast");
        assert_eq!(received.id, msg.id);
        assert_eq!(received.body, "everyone");
        assert_eq!(received.to_call_sign, "*");
    }

    #[tokio::test]
    async fn unregister_removes_member() {
        let bus = MessageBus::new();
        let (tx, _rx) = mpsc::channel::<SwarmMessage>(16);
        bus.register("charlie", tx).await;
        assert!(bus.is_registered("charlie").await);

        bus.unregister("charlie").await;
        assert!(!bus.is_registered("charlie").await);
    }

    #[tokio::test]
    async fn send_to_nonexistent_member_returns_error() {
        let bus = MessageBus::new();
        let msg = SwarmMessage::new_text("alpha", "ghost", "hello?");
        let err = bus.send(msg).await.unwrap_err();
        assert!(err.to_string().contains("recipient 'ghost' not registered"));
    }

    #[tokio::test]
    async fn list_registered_returns_correct_names() {
        let bus = MessageBus::new();
        let (tx1, _rx1) = mpsc::channel::<SwarmMessage>(16);
        let (tx2, _rx2) = mpsc::channel::<SwarmMessage>(16);

        bus.register("alpha", tx1).await;
        bus.register("bravo", tx2).await;

        let mut names = bus.list_registered().await;
        names.sort();
        assert_eq!(names, vec!["alpha", "bravo"]);
    }

    #[tokio::test]
    async fn shutdown_helpers_construct_correct_types() {
        let req = SwarmMessage::new_shutdown_request("alpha", "bravo");
        assert_eq!(req.message_type, SwarmMessageType::ShutdownRequest);
        assert_eq!(req.from_call_sign, "alpha");
        assert_eq!(req.to_call_sign, "bravo");

        let resp = SwarmMessage::new_shutdown_response("bravo", "alpha", true);
        assert_eq!(
            resp.message_type,
            SwarmMessageType::ShutdownResponse { approve: true }
        );
    }
}
