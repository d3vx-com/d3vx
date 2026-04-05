//! Telegram Bot API notification dispatcher.

use reqwest::Client;
use serde_json::json;

use super::types::{DispatchResult, Notification, NotificationPriority};

/// Sends notifications via the Telegram Bot API.
pub struct TelegramNotifier {
    bot_token: String,
    chat_id: String,
    client: Client,
}

impl TelegramNotifier {
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            bot_token,
            chat_id,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Create from environment variable names.
    pub fn from_env(token_env: &str, chat_id_env: &str) -> Option<Self> {
        let token = std::env::var(token_env).ok()?;
        let chat_id = std::env::var(chat_id_env).ok()?;
        if token.is_empty() || chat_id.is_empty() {
            return None;
        }
        Some(Self::new(token, chat_id))
    }

    /// Dispatch a notification via Telegram.
    pub async fn send(&self, notification: &Notification) -> DispatchResult {
        let text = self.format_message(notification);
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);

        let body = json!({
            "chat_id": self.chat_id,
            "text": text,
            "parse_mode": "MarkdownV2",
        });

        match self.client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => DispatchResult {
                channel: "telegram".to_string(),
                success: true,
                error: None,
            },
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let msg = format!("HTTP {} – {}", status, body);
                tracing::warn!(%msg, "Telegram API error");
                DispatchResult {
                    channel: "telegram".to_string(),
                    success: false,
                    error: Some(msg),
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Telegram request failed");
                DispatchResult {
                    channel: "telegram".to_string(),
                    success: false,
                    error: Some(e.to_string()),
                }
            }
        }
    }

    /// Format notification as Telegram MarkdownV2.
    fn format_message(&self, n: &Notification) -> String {
        let emoji = priority_emoji(n.priority);
        // MarkdownV2 requires escaping special chars: _ * [ ] ( ) ~ ` > # + - = | { } . !
        let title = escape_mdv2(&n.title);
        let body = escape_mdv2(&n.message);
        let mut parts = vec![format!("{} *{}*", emoji, title), body];

        if !n.metadata.is_null() && !n.metadata.as_object().map_or(true, |o| o.is_empty()) {
            parts.push(escape_mdv2(&format!("```{}", n.metadata)));
        }
        parts.join("\n\n")
    }
}

fn priority_emoji(p: NotificationPriority) -> &'static str {
    match p {
        NotificationPriority::Urgent => "\u{1f534}",
        NotificationPriority::Action => "\u{1f7e1}",
        NotificationPriority::Warning => "\u{1f535}",
        NotificationPriority::Info => "\u{2139}\u{fe0f}",
    }
}

/// Escape text for Telegram MarkdownV2.
fn escape_mdv2(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('_', "\\_")
        .replace('*', "\\*")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace('~', "\\~")
        .replace('`', "\\`")
        .replace('>', "\\>")
        .replace('#', "\\#")
        .replace('+', "\\+")
        .replace('-', "\\-")
        .replace('=', "\\=")
        .replace('|', "\\|")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('.', "\\.")
        .replace('!', "\\!")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_message_contains_emoji() {
        let notifier = TelegramNotifier::new("fake".into(), "123".into());
        let n = Notification::info("Test Title", "Hello world");
        let msg = notifier.format_message(&n);
        assert!(msg.contains("Test Title"));
        assert!(msg.contains("Hello world"));
    }

    #[test]
    fn escape_mdv2_handles_specials() {
        let escaped = escape_mdv2("hello_world [test].");
        assert_eq!(escaped, "hello\\_world \\[test\\]\\.");
    }

    #[test]
    fn priority_emoji_variants() {
        assert!(!priority_emoji(NotificationPriority::Urgent).is_empty());
        assert!(!priority_emoji(NotificationPriority::Info).is_empty());
    }

    #[test]
    fn from_env_returns_none_when_missing() {
        std::env::remove_var("__D3VX_TEST_TOKEN__");
        std::env::remove_var("__D3VX_TEST_CHAT__");
        assert!(TelegramNotifier::from_env("__D3VX_TEST_TOKEN__", "__D3VX_TEST_CHAT__").is_none());
    }

    #[test]
    fn notification_builder_helpers() {
        let n = Notification::urgent("Alert", "Something broke");
        assert_eq!(n.priority, NotificationPriority::Urgent);

        let n =
            Notification::info("FYI", "All good").with_metadata("task", serde_json::json!("T-1"));
        assert!(n.metadata["task"].is_string());
    }
}
