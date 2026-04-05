use anyhow::Result;
use d3vx::providers::anthropic::AnthropicProvider;
use d3vx::providers::{Message, MessageContent, MessagesRequest, Provider, ProviderOptions, Role};
use futures::StreamExt;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("debug,d3vx=trace")
        .init();

    let _ = dotenvy::dotenv();

    let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set");
    let base_url = env::var("ANTHROPIC_BASE_URL").ok();

    // Try with /v1 explicitly appended since the rust provider might not append it correctly
    let mut actual_base = base_url.clone().unwrap();
    if !actual_base.ends_with("/v1") {
        actual_base = format!("{}/v1", actual_base);
    }

    println!("Using base_url: {:?}", actual_base);
    println!("Which means provider will use: {}/messages", actual_base);

    let mut options = ProviderOptions::default();
    options.base_url = Some(actual_base);

    let provider = AnthropicProvider::with_options(api_key, options);

    let request = MessagesRequest {
        model: "glm-5".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text("Say hello world!".to_string()),
        }],
        system_prompt: None,
        tools: vec![],
        max_tokens: Some(1024),
        temperature: None,
        thinking: None,
        prompt_caching: true,
    };

    println!("Sending request to glm-5...");
    let mut stream = provider.send(request).await?;

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => println!("Event: {:?}", event),
            Err(e) => println!("Error: {:?}", e),
        }
    }

    println!("Done!");
    Ok(())
}
