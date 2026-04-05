//! Per-agent mailbox system for inter-agent direct messaging.
//!
//! Provides a global `MailboxRegistry` where each agent gets a dedicated
//! mailbox. Messages can be sent between agents by ID and are stored
//! in-memory with read/unread tracking.

use chrono::Utc;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

// -- Types ------------------------------------------------------------------

/// A single message in an agent's mailbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub timestamp: String,
    pub read: bool,
}

/// Per-agent mailbox holding all messages for one agent.
#[derive(Debug, Clone)]
pub struct AgentMailbox {
    #[allow(dead_code)]
    agent_id: String,
    messages: Vec<AgentMessage>,
}

impl AgentMailbox {
    fn new(agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            messages: Vec::new(),
        }
    }

    fn unread_count(&self) -> usize {
        self.messages.iter().filter(|m| !m.read).count()
    }
}

/// Global registry of all agent mailboxes.
pub struct MailboxRegistry {
    mailboxes: HashMap<String, AgentMailbox>,
}

impl MailboxRegistry {
    fn new() -> Self {
        Self {
            mailboxes: HashMap::new(),
        }
    }

    /// Create a mailbox for a new agent.
    ///
    /// If the agent already has a mailbox this is a no-op.
    pub fn register_agent(&mut self, agent_id: &str) {
        if self.mailboxes.contains_key(agent_id) {
            debug!(agent_id = %agent_id, "mailbox already registered, skipping");
            return;
        }
        debug!(agent_id = %agent_id, "registered agent mailbox");
        self.mailboxes
            .insert(agent_id.to_string(), AgentMailbox::new(agent_id));
    }

    /// Remove an agent's mailbox, discarding any undelivered messages.
    pub fn unregister_agent(&mut self, agent_id: &str) {
        if self.mailboxes.remove(agent_id).is_some() {
            debug!(agent_id = %agent_id, "unregistered agent mailbox");
        } else {
            warn!(agent_id = %agent_id, "unregister called for unknown agent");
        }
    }

    /// Deliver a message from one agent to another.
    ///
    /// Returns the generated message ID on success.
    /// Returns an error string if the recipient does not exist.
    pub fn send_message(
        &mut self,
        from: &str,
        to: &str,
        subject: &str,
        body: &str,
    ) -> Result<String, String> {
        let mailbox = self
            .mailboxes
            .get_mut(to)
            .ok_or_else(|| format!("Recipient '{}' has no mailbox", to))?;

        let msg_id = format!("msg-{}", Uuid::new_v4().as_simple());
        let message = AgentMessage {
            id: msg_id.clone(),
            from: from.to_string(),
            to: to.to_string(),
            subject: subject.to_string(),
            body: body.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            read: false,
        };

        debug!(
            from = %from,
            to = %to,
            msg_id = %msg_id,
            "delivering inter-agent message"
        );

        mailbox.messages.push(message);
        Ok(msg_id)
    }

    /// Read messages for an agent, optionally filtering to unread only.
    ///
    /// All returned messages are marked as read.
    /// Returns an empty vec if the agent has no mailbox.
    pub fn read_messages(&mut self, agent_id: &str, unread_only: bool) -> Vec<AgentMessage> {
        let mailbox = match self.mailboxes.get_mut(agent_id) {
            Some(m) => m,
            None => return Vec::new(),
        };

        let results: Vec<AgentMessage> = mailbox
            .messages
            .iter()
            .filter(|m| !unread_only || !m.read)
            .cloned()
            .collect();

        // Mark all returned messages as read
        for msg in &mut mailbox.messages {
            if results.iter().any(|r| r.id == msg.id) {
                msg.read = true;
            }
        }

        results
    }

    /// Count unread messages for an agent.
    ///
    /// Returns 0 if the agent has no mailbox.
    pub fn check_mail(&self, agent_id: &str) -> usize {
        self.mailboxes
            .get(agent_id)
            .map(|m| m.unread_count())
            .unwrap_or(0)
    }

    /// Check whether an agent is registered.
    pub fn is_registered(&self, agent_id: &str) -> bool {
        self.mailboxes.contains_key(agent_id)
    }
}

// -- Global singleton -------------------------------------------------------

static MAILBOX_REGISTRY: Lazy<RwLock<MailboxRegistry>> =
    Lazy::new(|| RwLock::new(MailboxRegistry::new()));

/// Register an agent in the global mailbox registry.
pub fn register_agent(agent_id: &str) {
    MAILBOX_REGISTRY.write().unwrap().register_agent(agent_id);
}

/// Unregister an agent from the global mailbox registry.
pub fn unregister_agent(agent_id: &str) {
    MAILBOX_REGISTRY.write().unwrap().unregister_agent(agent_id);
}

/// Send a message between agents via the global mailbox registry.
pub fn send_message(from: &str, to: &str, subject: &str, body: &str) -> Result<String, String> {
    MAILBOX_REGISTRY
        .write()
        .unwrap()
        .send_message(from, to, subject, body)
}

/// Read messages for an agent, optionally filtering to unread only.
pub fn read_messages(agent_id: &str, unread_only: bool) -> Vec<AgentMessage> {
    MAILBOX_REGISTRY
        .write()
        .unwrap()
        .read_messages(agent_id, unread_only)
}

/// Count unread messages for an agent.
pub fn check_mail(agent_id: &str) -> usize {
    MAILBOX_REGISTRY.read().unwrap().check_mail(agent_id)
}

/// Check whether an agent has a registered mailbox.
pub fn is_registered(agent_id: &str) -> bool {
    MAILBOX_REGISTRY.read().unwrap().is_registered(agent_id)
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Per-test unique ID prefix to avoid collisions when tests run in parallel
    /// against the global singleton.
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn uid(label: &str) -> String {
        let n = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("test-{}-{}", n, label)
    }

    #[test]
    fn register_and_unregister_lifecycle() {
        let a = uid("agent-a");
        let b = uid("agent-b");

        register_agent(&a);
        assert!(is_registered(&a));
        assert!(!is_registered(&b));

        unregister_agent(&a);
        assert!(!is_registered(&a));
    }

    #[test]
    fn double_register_is_noop() {
        let x = uid("agent-x");

        register_agent(&x);
        register_agent(&x);
        assert!(is_registered(&x));

        // Should still have exactly one mailbox
        let reg = MAILBOX_REGISTRY.read().unwrap();
        assert!(reg.mailboxes.contains_key(&x));
    }

    #[test]
    fn send_and_receive_between_two_agents() {
        let alice = uid("alice");
        let bob = uid("bob");

        register_agent(&alice);
        register_agent(&bob);

        let msg_id =
            send_message(&alice, &bob, "Hello", "Hi from Alice").expect("send should succeed");
        assert!(!msg_id.is_empty());

        // Bob should have 1 unread
        assert_eq!(check_mail(&bob), 1);
        assert_eq!(check_mail(&alice), 0);

        // Bob reads all messages
        let messages = read_messages(&bob, false);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].from, alice);
        assert_eq!(messages[0].to, bob);
        assert_eq!(messages[0].subject, "Hello");
        assert_eq!(messages[0].body, "Hi from Alice");
        assert!(!messages[0].id.is_empty());
        assert!(!messages[0].timestamp.is_empty());
    }

    #[test]
    fn unread_count_tracking() {
        let producer = uid("producer");
        let consumer = uid("consumer");

        register_agent(&producer);
        register_agent(&consumer);

        assert_eq!(check_mail(&consumer), 0);

        send_message(&producer, &consumer, "Msg1", "body1").unwrap();
        assert_eq!(check_mail(&consumer), 1);

        send_message(&producer, &consumer, "Msg2", "body2").unwrap();
        assert_eq!(check_mail(&consumer), 2);
    }

    #[test]
    fn reading_marks_as_read() {
        let sender = uid("sender");
        let receiver = uid("receiver");

        register_agent(&sender);
        register_agent(&receiver);

        send_message(&sender, &receiver, "Sub", "Body").unwrap();
        assert_eq!(check_mail(&receiver), 1);

        // Read all messages (marks as read)
        let msgs = read_messages(&receiver, false);
        assert_eq!(msgs.len(), 1);

        // Unread count should now be 0
        assert_eq!(check_mail(&receiver), 0);

        // Reading unread-only should return nothing
        let again = read_messages(&receiver, true);
        assert!(again.is_empty());

        // But reading all should still return the message (just already read)
        let all = read_messages(&receiver, false);
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn send_to_nonexistent_agent_returns_error() {
        let only = uid("only-agent");
        let ghost = uid("ghost");

        register_agent(&only);

        let result = send_message(&only, &ghost, "Hi", "No one home");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(&ghost));
    }

    #[test]
    fn read_messages_unread_only_filter() {
        let a = uid("a");
        let b = uid("b");

        register_agent(&a);
        register_agent(&b);

        send_message(&a, &b, "First", "1").unwrap();
        send_message(&a, &b, "Second", "2").unwrap();
        send_message(&a, &b, "Third", "3").unwrap();

        // Read only unread -- should get all 3 and mark as read
        let unread = read_messages(&b, true);
        assert_eq!(unread.len(), 3);

        // Now unread count is 0
        assert_eq!(check_mail(&b), 0);

        // Send one more
        send_message(&a, &b, "Fourth", "4").unwrap();
        assert_eq!(check_mail(&b), 1);

        // Read unread only -- should get just the new one
        let new_unread = read_messages(&b, true);
        assert_eq!(new_unread.len(), 1);
        assert_eq!(new_unread[0].subject, "Fourth");
    }

    #[test]
    fn read_messages_for_unknown_agent_returns_empty() {
        let unknown = uid("nonexistent");
        let msgs = read_messages(&unknown, false);
        assert!(msgs.is_empty());
    }

    #[test]
    fn check_mail_for_unknown_agent_returns_zero() {
        let unknown = uid("unknown");
        assert_eq!(check_mail(&unknown), 0);
    }

    #[test]
    fn unregister_drops_messages() {
        let x = uid("x");
        let y = uid("y");

        register_agent(&x);
        register_agent(&y);

        send_message(&x, &y, "Lost", "This will be discarded").unwrap();
        assert_eq!(check_mail(&y), 1);

        unregister_agent(&y);

        // Mail is gone
        assert_eq!(check_mail(&y), 0);
        let msgs = read_messages(&y, false);
        assert!(msgs.is_empty());
    }

    #[test]
    fn message_timestamp_is_iso8601() {
        let ts_sender = uid("ts-sender");
        let ts_receiver = uid("ts-receiver");

        register_agent(&ts_sender);
        register_agent(&ts_receiver);

        send_message(&ts_sender, &ts_receiver, "timecheck", "body").unwrap();
        let msgs = read_messages(&ts_receiver, false);
        assert_eq!(msgs.len(), 1);

        // Verify the timestamp parses as ISO 8601
        let ts = &msgs[0].timestamp;
        assert!(
            chrono::DateTime::parse_from_rfc3339(ts).is_ok(),
            "timestamp '{}' should be valid ISO 8601",
            ts
        );
    }
}
