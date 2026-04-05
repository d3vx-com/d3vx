//! Notification utility
//!
//! Handles sending notifications via macOS desktop and Telegram bot.

use crate::config::NotificationsConfig;
use anyhow::Result;
use serde::Serialize;
use tracing::{debug, error};

#[derive(Debug, Clone, Serialize)]
pub struct NotificationOptions {
    pub title: String,
    pub body: String,
    pub type_name: String, // "success", "error", "info"
}

/// Send a notification based on configuration
pub async fn notify(opts: NotificationOptions, config: &NotificationsConfig) -> Result<()> {
    debug!("Sending notification: {} - {}", opts.title, opts.body);

    // Desktop notification (macOS)
    if config.desktop {
        if let Err(e) = notify_macos(&opts) {
            error!("Failed to send macOS notification: {}", e);
        }
    }

    // Telegram notification
    if let Some(ref telegram) = config.telegram {
        if !telegram.bot_token.is_empty() && !telegram.chat_id.is_empty() {
            if let Err(e) = notify_telegram(&opts, &telegram.bot_token, &telegram.chat_id).await {
                error!("Failed to send Telegram notification: {}", e);
            }
        }
    }

    Ok(())
}

/// Send a macOS desktop notification using 'osascript'
fn notify_macos(opts: &NotificationOptions) -> Result<()> {
    let script = format!(
        "display notification \"{}\" with title \"{}\" subtitle \"d3vx terminal\"",
        opts.body.replace("\"", "\\\""),
        opts.title.replace("\"", "\\\"")
    );

    std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .spawn()?;

    Ok(())
}

/// Send a Telegram notification via Bot API
async fn notify_telegram(opts: &NotificationOptions, bot_token: &str, chat_id: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);

    let text = format!("<b>{}</b>\n\n{}", opts.title, opts.body);

    let payload = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "HTML"
    });

    let resp = client.post(url).json(&payload).send().await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await?;
        return Err(anyhow::anyhow!("Telegram API error: {}", error_text));
    }

    Ok(())
}
