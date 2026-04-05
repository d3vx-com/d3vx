//! E2E Integration Tests
//!
//! Tests the full agent loop with real provider integration.
//! Run with: cargo run --bin e2e_test

use anyhow::Result;
use d3vx::providers::anthropic::AnthropicProvider;
use d3vx::providers::traits::StreamResult;
use d3vx::providers::{Message, MessageContent, MessagesRequest, Provider, Role, StreamEvent};
use futures::StreamExt;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,d3vx=debug")
        .init();

    let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set");
    println!("Starting E2E test...\n");

    // Test 1: Basic completion
    println!("=== Test 1: Basic Completion ===");
    test_basic_completion(&api_key).await?;

    // Test 2: With thinking
    println!("\n=== Test 2: Extended Thinking ===");
    test_thinking(&api_key).await?;

    println!("\n=== All E2E tests passed! ===");
    Ok(())
}

async fn test_basic_completion(api_key: &str) -> Result<()> {
    let provider = AnthropicProvider::new(api_key.to_string());

    let request = MessagesRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text("Say exactly: 'Hello, World!'".to_string()),
        }],
        system_prompt: None,
        tools: vec![],
        max_tokens: Some(100),
        temperature: None,
        thinking: None,
        prompt_caching: false,
    };

    let mut stream: StreamResult = provider.send(request).await?;
    let mut got_text = false;

    while let Some(event_result) = stream.next().await {
        match event_result? {
            StreamEvent::TextDelta { text } => {
                print!("{}", text);
                if text.contains("Hello") || text.contains("World") {
                    got_text = true;
                }
            }
            StreamEvent::MessageEnd { usage, stop_reason } => {
                println!(
                    "\n[Usage: {} input, {} output, {} reasoning tokens]",
                    usage.input_tokens, usage.output_tokens, usage.reasoning_tokens
                );
                println!("[Stop reason: {:?}]", stop_reason);
                assert!(got_text, "Should have received 'Hello, World!'");
                return Ok(());
            }
            _ => {}
        }
    }
    Err(anyhow::anyhow!("Stream ended without MessageEnd"))
}

async fn test_thinking(api_key: &str) -> Result<()> {
    let provider = AnthropicProvider::new(api_key.to_string());

    let request = MessagesRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(
                "Explain why the sky is blue in one short sentence.".to_string(),
            ),
        }],
        system_prompt: None,
        tools: vec![],
        max_tokens: Some(256),
        temperature: None,
        thinking: Some(d3vx::providers::ThinkingConfig {
            enabled: true,
            budget_tokens: Some(1024),
            reasoning_effort: None,
        }),
        prompt_caching: false,
    };

    let mut stream: StreamResult = provider.send(request).await?;
    let mut got_thinking = false;

    while let Some(event_result) = stream.next().await {
        match event_result? {
            StreamEvent::ThinkingDelta { text } => {
                print!("[thinking: {}...]", &text[..text.len().min(40)]);
                got_thinking = true;
            }
            StreamEvent::TextDelta { text } => {
                print!("{}", text);
            }
            StreamEvent::MessageEnd { usage, .. } => {
                println!("\n[Total tokens: {}]", usage.total());
                println!("[Reasoning tokens: {}]", usage.reasoning_tokens);
                assert!(got_thinking, "Should have received thinking content");
                return Ok(());
            }
            _ => {}
        }
    }
    Err(anyhow::anyhow!("Stream ended without MessageEnd"))
}
