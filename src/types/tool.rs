//! Tool definition types for LLM function calling
//!
//! Tools define capabilities that the LLM can invoke during conversation.
//! These types follow Anthropic's tool format for maximum expressiveness.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Parameter definition within a tool's input schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    /// The JSON type of the parameter (e.g., "string", "number", "boolean")
    #[serde(rename = "type")]
    pub param_type: String,
    /// Description of what this parameter does
    pub description: String,
    /// Optional enum values if this is an enum type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    /// For array types, the type of items
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<ToolParameter>>,
    /// Whether this parameter is required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// Default value for this parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

impl ToolParameter {
    /// Create a new string parameter
    pub fn string(description: impl Into<String>) -> Self {
        Self {
            param_type: "string".to_string(),
            description: description.into(),
            enum_values: None,
            items: None,
            required: None,
            default: None,
        }
    }

    /// Create a new number parameter
    pub fn number(description: impl Into<String>) -> Self {
        Self {
            param_type: "number".to_string(),
            description: description.into(),
            enum_values: None,
            items: None,
            required: None,
            default: None,
        }
    }

    /// Create a new boolean parameter
    pub fn boolean(description: impl Into<String>) -> Self {
        Self {
            param_type: "boolean".to_string(),
            description: description.into(),
            enum_values: None,
            items: None,
            required: None,
            default: None,
        }
    }

    /// Create a new array parameter
    pub fn array(description: impl Into<String>, item_type: ToolParameter) -> Self {
        Self {
            param_type: "array".to_string(),
            description: description.into(),
            enum_values: None,
            items: Some(Box::new(item_type)),
            required: None,
            default: None,
        }
    }

    /// Create a new enum parameter
    pub fn enum_param(description: impl Into<String>, values: Vec<String>) -> Self {
        Self {
            param_type: "string".to_string(),
            description: description.into(),
            enum_values: Some(values),
            items: None,
            required: None,
            default: None,
        }
    }

    /// Mark this parameter as required
    pub fn required(mut self) -> Self {
        self.required = Some(true);
        self
    }

    /// Set a default value for this parameter
    pub fn default(mut self, value: Value) -> Self {
        self.default = Some(value);
        self
    }
}

/// Input schema for a tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    /// Always "object" for tool input schemas
    #[serde(rename = "type")]
    pub schema_type: String,
    /// Parameter definitions
    pub properties: HashMap<String, ToolParameter>,
    /// Required parameter names
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
}

impl ToolSchema {
    /// Create a new tool schema with the given properties
    pub fn new(properties: HashMap<String, ToolParameter>) -> Self {
        let required = properties
            .iter()
            .filter(|(_, p)| p.required.unwrap_or(false))
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();

        Self {
            schema_type: "object".to_string(),
            properties,
            required: if required.is_empty() {
                None
            } else {
                Some(required)
            },
        }
    }

    /// Create an empty schema
    pub fn empty() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: HashMap::new(),
            required: None,
        }
    }
}

/// Definition of a tool that can be invoked by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique name of the tool
    pub name: String,
    /// Description of what the tool does
    pub description: String,
    /// JSON schema for the tool's input parameters
    pub input_schema: ToolSchema,
}

impl ToolDefinition {
    /// Create a new tool definition
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: ToolSchema,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }

    /// Create a simple tool with no parameters
    pub fn simple(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: ToolSchema::empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_parameter_string() {
        let param = ToolParameter::string("A file path").required();
        assert_eq!(param.param_type, "string");
        assert_eq!(param.description, "A file path");
        assert_eq!(param.required, Some(true));
    }

    #[test]
    fn test_tool_parameter_enum() {
        let param =
            ToolParameter::enum_param("Mode", vec!["read".to_string(), "write".to_string()]);
        assert_eq!(
            param.enum_values,
            Some(vec!["read".to_string(), "write".to_string()])
        );
    }

    #[test]
    fn test_tool_schema_serialization() {
        let mut properties = HashMap::new();
        properties.insert(
            "path".to_string(),
            ToolParameter::string("File path").required(),
        );
        properties.insert(
            "mode".to_string(),
            ToolParameter::enum_param("Access mode", vec!["read".to_string(), "write".to_string()]),
        );

        let schema = ToolSchema::new(properties);
        let json = serde_json::to_string(&schema).unwrap();

        assert!(json.contains(r#""type":"object""#));
        assert!(json.contains(r#""required":["path"]"#));
    }

    #[test]
    fn test_tool_definition_serialization() {
        let tool = ToolDefinition::simple("ping", "Check connectivity");
        let json = serde_json::to_string(&tool).unwrap();

        assert!(json.contains(r#""name":"ping""#));
        assert!(json.contains(r#""description":"Check connectivity""#));
        assert!(json.contains(r#""input_schema""#));
    }

    #[test]
    fn test_tool_definition_deserialization() {
        let json = r#"{
            "name": "read_file",
            "description": "Read a file",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path"
                    }
                },
                "required": ["path"]
            }
        }"#;

        let tool: ToolDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(tool.name, "read_file");
        assert_eq!(tool.description, "Read a file");
        assert!(tool.input_schema.properties.contains_key("path"));
    }
}
