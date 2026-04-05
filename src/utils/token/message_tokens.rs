//! Message Token Estimation
//!
//! Estimates tokens for provider message types.

use crate::providers::{ContentBlock, Message, MessageContent};

use super::estimator::estimate_tokens_for_text;

pub const CHARS_PER_TOKEN: usize = 4;

pub fn estimate_message_tokens(message: &Message) -> u64 {
    match &message.content {
        MessageContent::Text(text) => estimate_tokens_for_text(text) as u64,
        MessageContent::Blocks(blocks) => blocks.iter().map(estimate_block_tokens).sum(),
    }
}

pub fn estimate_block_tokens(block: &ContentBlock) -> u64 {
    match block {
        ContentBlock::Text { text } => estimate_tokens_for_text(text) as u64,
        ContentBlock::Thinking { thinking } => estimate_tokens_for_text(thinking) as u64,
        ContentBlock::ToolUse { input, .. } => estimate_tokens_for_text(&input.to_string()) as u64,
        ContentBlock::ToolResult { content, .. } => estimate_tokens_for_text(content) as u64,
        ContentBlock::Image { source } => (source.data.len() / 10_000) as u64 + 100,
    }
}

pub fn estimate_tokens(text: &str) -> usize {
    estimate_tokens_for_text(text)
}
