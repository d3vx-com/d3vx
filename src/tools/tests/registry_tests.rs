//! Tests for Tool Registry
//!
//! Covers tool registration and retrieval.

#[cfg(test)]
mod tests {
    use crate::tools::types::Tool;
    use crate::tools::{
        get_tool, list_tools, register_core_tools, register_tool, BashTool, EditTool, ReadTool,
        WriteTool,
    };

    // =========================================================================
    // Registration Tests
    // =========================================================================

    #[test]
    fn test_register_and_retrieve_tool() {
        let tool = BashTool::new();
        let name = tool.definition().name.clone();
        register_tool(tool);

        let retrieved = get_tool(&name);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_register_multiple_tools() {
        register_tool(ReadTool::new());
        register_tool(WriteTool::new());
        register_tool(EditTool::new());

        assert!(get_tool("Read").is_some());
        assert!(get_tool("Write").is_some());
        assert!(get_tool("Edit").is_some());
    }

    #[test]
    fn test_get_nonexistent_tool() {
        let result = get_tool("NonexistentTool123");
        assert!(result.is_none());
    }

    #[test]
    fn test_list_tools() {
        register_core_tools();
        let tools = list_tools();

        assert!(!tools.is_empty());

        let tool_names: Vec<String> = tools.iter().map(|t| t.name()).collect();
        assert!(tool_names.contains(&"Bash".to_string()));
        assert!(tool_names.contains(&"Read".to_string()));
        assert!(tool_names.contains(&"Write".to_string()));
    }

    #[test]
    fn test_core_tools_registered() {
        register_core_tools();

        let expected_tools = vec![
            "Bash",
            "Read",
            "Write",
            "Edit",
            "Glob",
            "Grep",
            "Think",
            "Question",
            "TodoWrite",
            "WebFetch",
            "MultiEdit",
            "DelegateReview",
            "send_inbox_message",
            "read_inbox",
            "complete_task",
            "draft_change",
            "skill",
            "structured_output",
            "best_of_n",
        ];

        for tool_name in expected_tools {
            assert!(
                get_tool(tool_name).is_some(),
                "Tool {} should be registered",
                tool_name
            );
        }
    }

    // =========================================================================
    // Tool Definition Tests
    // =========================================================================

    #[test]
    fn test_tool_definition_schema() {
        let tool = BashTool::new();
        let def = tool.definition();

        assert!(!def.name.is_empty());
        assert!(!def.description.is_empty());
        assert!(def.input_schema.is_object());

        let schema = &def.input_schema;
        assert!(schema["type"].is_string());
        assert!(schema["properties"].is_object());
    }

    #[test]
    fn test_all_tools_have_valid_definitions() {
        register_core_tools();
        let tools = list_tools();

        for tool in tools {
            assert!(!tool.name().is_empty(), "Tool name should not be empty");
            assert!(
                !tool.definition().description.is_empty(),
                "Tool {} description should not be empty",
                tool.name()
            );
            assert!(
                tool.definition().input_schema.is_object(),
                "Tool {} should have object schema",
                tool.name()
            );
        }
    }
}
