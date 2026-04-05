//! Test real API call to GLM-5 with spawn_parallel_agents tool

use anyhow::Result;
use d3vx::providers::anthropic::AnthropicProvider;
use d3vx::providers::{
    Message, MessageContent, MessagesRequest, Provider, ProviderOptions, Role, StreamEvent,
    ToolDefinition,
};
use futures::StreamExt;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,d3vx=trace")
        .init();

    let _ = dotenvy::dotenv();

    // GLM uses Anthropic-compatible API
    let api_key = env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set");
    let base_url = env::var("ANTHROPIC_BASE_URL")
        .ok()
        .unwrap_or_else(|| "https://open.bigmodel.cn/api/paas/v4".to_string());

    let mut actual_base = base_url.clone();
    if !actual_base.ends_with("/v1") {
        actual_base = format!("{}/v1", actual_base);
    }

    println!("Using base_url: {}", actual_base);
    println!("Model: glm-5");

    let options = ProviderOptions {
        base_url: Some(actual_base),
        ..Default::default()
    };

    let provider = AnthropicProvider::with_options(api_key, options);

    // Define spawn_parallel_agents tool using serde_json
    let tool_json = serde_json::json!({
        "name": "spawn_parallel_agents",
        "description": "SPAWN PARALLEL AGENTS - This is the ONLY way to run multiple agents. Use this tool to spawn 2-5 parallel agent loops for independent tasks.",
        "input_schema": {
            "type": "object",
            "properties": {
                "subtasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "description": {"type": "string"},
                            "task": {"type": "string"},
                            "agent_type": {"type": "string"}
                        },
                        "required": ["description", "task"]
                    }
                },
                "reasoning": {"type": "string"}
            },
            "required": ["subtasks", "reasoning"]
        }
    });

    let spawn_parallel_tool: ToolDefinition =
        serde_json::from_value(tool_json).expect("Failed to parse tool");

    let request = MessagesRequest {
        model: "glm-5".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: MessageContent::Text(
                "You are a senior developer. Analyze this codebase using parallel agents. Use the spawn_parallel_agents tool to launch agents that will analyze: Backend API code, Frontend code, and Database code. Call spawn_parallel_agents with these 3 analysis tasks.".to_string(),
            ),
        }],
        system_prompt: Some(
            "You are a helpful assistant. You MUST call the spawn_parallel_agents tool when asked to use parallel agents. Do NOT write bash scripts.".to_string()
        ),
        tools: vec![spawn_parallel_tool],
        max_tokens: Some(2048),
        temperature: Some(0.7),
        thinking: None,
        prompt_caching: true,
    };

    println!("\n=== Sending request to GLM-5 ===\n");
    let mut stream = provider.send(request).await?;

    let mut tool_calls_found = Vec::new();

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                match event {
                    StreamEvent::ToolUseStart { id: _, name } => {
                        println!("\n*** TOOL USE START: {} ***", name);
                    }
                    StreamEvent::ToolUseDelta { input_json } => {
                        let preview = if input_json.len() > 300 {
                            format!("{}...", &input_json[..300])
                        } else {
                            input_json.clone()
                        };
                        println!("Input: {}", preview);
                    }
                    StreamEvent::ToolUseEnd { id: _, name, input } => {
                        println!("\n*** TOOL USE END: {} ***", name);
                        println!("Full input: {}", input);
                        tool_calls_found.push(name);
                    }
                    StreamEvent::TextDelta { text } => {
                        print!("{}", text);
                    }
                    StreamEvent::ThinkingDelta { text: _ } => {
                        // Skip thinking output
                    }
                    StreamEvent::MessageEnd {
                        usage: _,
                        stop_reason,
                    } => {
                        println!("\n*** MESSAGE END: {:?} ***", stop_reason);
                    }
                    _ => {}
                }
            }
            Err(e) => println!("\nError: {:?}", e),
        }
    }

    println!("\n\n=== RESULTS ===");
    if tool_calls_found.is_empty() {
        println!("NO TOOL CALLS FOUND");
    } else {
        println!("Tool calls found: {:?}", tool_calls_found);
        if tool_calls_found
            .iter()
            .any(|n| n == "spawn_parallel_agents")
        {
            println!("\n*** SUCCESS: spawn_parallel_agents was called! ***");
        } else {
            println!("\n*** WARNING: spawn_parallel_agents was NOT called ***");
            println!("Other tools called: {:?}", tool_calls_found);
        }
    }

    Ok(())
}
