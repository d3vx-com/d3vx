//! Helper utilities for Best-of-N execution

/// Strip thinking/reasoning tags from content
pub fn strip_thinking_tags(content: &str) -> String {
    let mut result = content.to_string();

    // Strip common thinking tags
    for (open, close) in &[
        ("<think>", "</think>"),
        ("<thinking>", "</thinking>"),
        ("<reasoning>", "</reasoning>"),
    ] {
        while let Some(start) = result.find(open) {
            if let Some(end) = result.find(close) {
                result = format!("{}{}", &result[..start], &result[end + close.len()..]);
            } else {
                break;
            }
        }
    }

    result.trim().to_string()
}

/// Truncate content for preview
pub fn truncate_preview(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        format!("{}...[truncated]", &content[..max_len])
    }
}
