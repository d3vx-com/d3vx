//! Tests for MCP Protocol Types
//!
//! Covers JSON-RPC message structures and MCP-specific types.

#[cfg(test)]
mod tests {
    use crate::mcp::protocol::{
        JsonRpcRequest, JsonRpcResponse, JsonRpcError, JsonRpcNotification,
        InitializeParams, InitializeResult, ClientInfo, ServerInfo,
        ListToolsResult, McpToolDefinition, CallToolParams, CallToolResult,
        McpContent,
    };
    use serde_json::json;

    // =========================================================================
    // JSON-RPC Request Tests
    // =========================================================================

    #[test]
    fn test_json_rpc_request_creation() {
        let req = JsonRpcRequest::new(
            json!(1),
            "test_method",
            json!({"key": "value"}),
        );

        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, json!(1));
        assert_eq!(req.method, "test_method");
    }

    #[test]
    fn test_json_rpc_request_serialization() {
        let req = JsonRpcRequest::new(
            json!("req-123"),
            "initialize",
            json!({"protocol_version": "2024-11-05"}),
        );

        let json_str = serde_json::to_string(&req).unwrap();

        assert!(json_str.contains("\"jsonrpc\":\"2.0\""));
        assert!(json_str.contains("\"id\":\"req-123\""));
        assert!(json_str.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn test_json_rpc_request_deserialization() {
        let json_str = r#"{
            "jsonrpc": "2.0",
            "id": 42,
            "method": "tools/call",
            "params": {"name": "test_tool"}
        }"#;

        let req: JsonRpcRequest = serde_json::from_str(json_str).unwrap();

        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, json!(42));
        assert_eq!(req.method, "tools/call");
    }

    // =========================================================================
    // JSON-RPC Response Tests
    // =========================================================================

    #[test]
    fn test_json_rpc_response_success() {
        let json_str = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"status": "ok"}
        }"#;

        let res: JsonRpcResponse = serde_json::from_str(json_str).unwrap();

        assert_eq!(res.jsonrpc, "2.0");
        assert_eq!(res.id, json!(1));
        assert!(res.result.is_some());
        assert!(res.error.is_none());
    }

    #[test]
    fn test_json_rpc_response_error() {
        let json_str = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32600, "message": "Invalid Request"}
        }"#;

        let res: JsonRpcResponse = serde_json::from_str(json_str).unwrap();

        assert!(res.result.is_none());
        assert!(res.error.is_some());

        let error = res.error.unwrap();
        assert_eq!(error.code, -32600);
        assert_eq!(error.message, "Invalid Request");
    }

    #[test]
    fn test_json_rpc_error() {
        let error = JsonRpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: Some(json!({"method": "unknown"})),
        };

        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
        assert!(error.data.is_some());
    }

    // =========================================================================
    // JSON-RPC Notification Tests
    // =========================================================================

    #[test]
    fn test_json_rpc_notification_creation() {
        let notif = JsonRpcNotification::new(
            "notifications/initialized",
            json!({}),
        );

        assert_eq!(notif.jsonrpc, "2.0");
        assert_eq!(notif.method, "notifications/initialized");
    }

    #[test]
    fn test_json_rpc_notification_no_id() {
        let json_str = r#"{
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": {"uri": "file:///test.txt"}
        }"#;

        let notif: JsonRpcNotification = serde_json::from_str(json_str).unwrap();

        assert_eq!(notif.jsonrpc, "2.0");
        assert_eq!(notif.method, "notifications/resources/updated");
    }

    // =========================================================================
    // MCP Initialize Tests
    // =========================================================================

    #[test]
    fn test_initialize_params() {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: json!({"tools": {}}),
            client_info: ClientInfo {
                name: "d3vx".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        assert_eq!(params.protocol_version, "2024-11-05");
        assert_eq!(params.client_info.name, "d3vx");
    }

    #[test]
    fn test_initialize_params_serialization() {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: json!({"tools": {}, "resources": {}}),
            client_info: ClientInfo {
                name: "d3vx".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let json = serde_json::to_string(&params).unwrap();

        assert!(json.contains("\"protocol_version\""));
        assert!(json.contains("\"client_info\""));
    }

    #[test]
    fn test_initialize_result() {
        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: json!({"tools": {}}),
            server_info: ServerInfo {
                name: "mcp-server".to_string(),
                version: "1.0.0".to_string(),
            },
        };

        assert_eq!(result.protocol_version, "2024-11-05");
        assert_eq!(result.server_info.name, "mcp-server");
    }

    // =========================================================================
    // MCP Tool Definition Tests
    // =========================================================================

    #[test]
    fn test_mcp_tool_definition() {
        let tool = McpToolDefinition {
            name: "read_file".to_string(),
            description: Some("Read a file from disk".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        };

        assert_eq!(tool.name, "read_file");
        assert!(tool.description.is_some());
    }

    #[test]
    fn test_list_tools_result() {
        let result = ListToolsResult {
            tools: vec![
                McpToolDefinition {
                    name: "tool1".to_string(),
                    description: Some("First tool".to_string()),
                    input_schema: json!({"type": "object"}),
                },
                McpToolDefinition {
                    name: "tool2".to_string(),
                    description: None,
                    input_schema: json!({"type": "object"}),
                },
            ],
        };

        assert_eq!(result.tools.len(), 2);
    }

    // =========================================================================
    // Call Tool Tests
    // =========================================================================

    #[test]
    fn test_call_tool_params() {
        let params = CallToolParams {
            name: "bash".to_string(),
            arguments: json!({"command": "ls -la"}),
        };

        assert_eq!(params.name, "bash");
        assert_eq!(params.arguments["command"], "ls -la");
    }

    #[test]
    fn test_call_tool_params_serialization() {
        let params = CallToolParams {
            name: "read_file".to_string(),
            arguments: json!({"path": "/tmp/test.txt"}),
        };

        let json = serde_json::to_value(&params).unwrap();

        assert_eq!(json["name"], "read_file");
        assert_eq!(json["arguments"]["path"], "/tmp/test.txt");
    }

    #[test]
    fn test_call_tool_result_success() {
        let result = CallToolResult {
            content: vec![McpContent::Text {
                text: "File contents here".to_string(),
            }],
            is_error: false,
        };

        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
    }

    #[test]
    fn test_call_tool_result_error() {
        let result = CallToolResult {
            content: vec![McpContent::Text {
                text: "Error: File not found".to_string(),
            }],
            is_error: true,
        };

        assert!(result.is_error);
    }

    // =========================================================================
    // McpContent Tests
    // =========================================================================

    #[test]
    fn test_mcp_content_text() {
        let content = McpContent::Text {
            text: "Hello".to_string(),
        };

        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");
    }

    #[test]
    fn test_mcp_content_image() {
        let content = McpContent::Image {
            data: "base64imagedata".to_string(),
            mime_type: "image/png".to_string(),
        };

        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "image");
        assert_eq!(json["mime_type"], "image/png");
    }

    #[test]
    fn test_mcp_content_resource() {
        let content = McpContent::Resource {
            resource: json!({"uri": "file:///test.txt"}),
        };

        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "resource");
    }

    // =========================================================================
    // Round-Trip Tests
    // =========================================================================

    #[test]
    fn test_request_round_trip() {
        let original = JsonRpcRequest::new(
            json!("abc-123"),
            "tools/list",
            json!({"cursor": null}),
        );

        let json_str = serde_json::to_string(&original).unwrap();
        let parsed: JsonRpcRequest = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.jsonrpc, original.jsonrpc);
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.method, original.method);
    }

    #[test]
    fn test_call_tool_result_round_trip() {
        let original = CallToolResult {
            content: vec![
                McpContent::Text { text: "Line 1".to_string() },
                McpContent::Text { text: "Line 2".to_string() },
            ],
            is_error: false,
        };

        let json_str = serde_json::to_string(&original).unwrap();
        let parsed: CallToolResult = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.content.len(), 2);
        assert_eq!(parsed.is_error, false);
    }
}
