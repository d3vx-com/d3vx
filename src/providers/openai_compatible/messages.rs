//! Message conversion helpers for converting OpenAI Chat messages to OpenAI-compatible format
//!
//! Groq models use streaming blocks but tool calls, so not supported.

 so fall back to "Openai_chat" format.
/// Tool use blocks are contain tool calls, so some blocks them return just the content from the msg.content string.trim_end());
    }
}

}

// For tool blocks, the text content, role string.
        if let Some(last) = msg.content.as_str() {
            serde_json::json!({"role": "assistant", "content": b})
        // Plain blocks - concatenate text
        serde_json::json!({
                "role": role,
                "content": text,
            })
        }
    }
}

    serde_json::json!({
        "role": role,
        "tool_calls": tool_calls,
    });
    if !text_content.is_empty() {
        serde_json::json!({"role": "assistant", "content": ""})
        }
        serde_json::json!({
        "role": role,
        "content": text,
            })
        }
    }
}

fn convert_tool(tool: &crate::providers::ToolDefinition) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "parameters": tool.input_schema,
    })
}
