//! Formatting utilities

use chrono::{DateTime, Utc};

/// Format a timestamp as relative time
pub fn format_relative_time(time: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(time);

    if diff.num_seconds() < 60 {
        "just now".to_string()
    } else if diff.num_seconds() < 3600 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_seconds() < 86400 {
        format!("{}h ago", diff.num_hours())
    } else {
        format!("{}d ago", diff.num_days())
    }
}

/// Format a timestamp as HH:MM
pub fn format_time(time: DateTime<Utc>) -> String {
    time.format("%H:%M").to_string()
}

/// Format elapsed milliseconds
pub fn format_elapsed(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{}s", ms / 1000)
    } else if ms < 3600000 {
        format!("{}m", ms / 60000)
    } else {
        format!("{}h", ms / 3600000)
    }
}

/// Format token count
pub fn format_tokens(tokens: u64) -> String {
    if tokens < 1000 {
        format!("{}", tokens)
    } else if tokens < 1_000_000 {
        format!("{:.1}K", tokens as f64 / 1000.0)
    } else {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    }
}

/// Format cost
pub fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("${:.4}", cost)
    } else if cost < 1.0 {
        format!("${:.3}", cost)
    } else {
        format!("${:.2}", cost)
    }
}

/// Format file size
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes < KB {
        format!("{}B", bytes)
    } else if bytes < MB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_elapsed() {
        assert_eq!(format_elapsed(50), "50ms");
        assert_eq!(format_elapsed(1500), "1s");
        assert_eq!(format_elapsed(90000), "1m");
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(100), "100");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1500000), "1.5M");
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.0001), "$0.0001");
        assert!(format_cost(0.01).starts_with("$0.01"));
        assert!(format_cost(1.234).starts_with("$1.2"));
    }
}
