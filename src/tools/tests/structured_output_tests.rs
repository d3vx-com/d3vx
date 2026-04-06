//! Structured Output Tests
//!
//! Tests for SchemaValidator, ValidationError, and schema helpers.

use crate::tools::structured_output::schemas::*;
use crate::tools::structured_output::{SchemaValidator, ValidationError};
use serde_json::json;

// -- SchemaValidator tests --

#[test]
fn validator_new_defaults_to_strict() {
    let v = SchemaValidator::new(json!({}));
    // strict mode accepts additional fields
    let result = v.validate(&json!({"foo": "bar"}));
    assert!(result.is_ok());
}

#[test]
fn validator_with_strict_false() {
    let v = SchemaValidator::new(json!({})).with_strict(false);
    // non-strict mode accepts any value
    let result = v.validate(&json!(42));
    assert!(result.is_ok());
}

#[test]
fn validator_strict_accepts_any_value() {
    let v = SchemaValidator::new(json!({"type": "object"})).with_strict(true);
    let result = v.validate(&json!({"foo": "bar"}));
    assert!(result.is_ok());
}

#[test]
fn validator_non_strict_accepts_any_value() {
    let v = SchemaValidator::new(json!({"type": "object"})).with_strict(false);
    let result = v.validate(&json!(42));
    assert!(result.is_ok());
}

#[test]
fn validated_returns_clone_on_success() {
    let v = SchemaValidator::new(json!({}));
    let input = json!({"key": 1});
    let out = v.validated(&input).unwrap();
    assert_eq!(out, input);
}

// -- schemas::list_schema tests --

#[test]
fn list_schema_wraps_items() {
    let s = list_schema(json!({"type": "string"}));
    assert_eq!(s["type"], "array");
    assert_eq!(s["items"]["type"], "string");
}

#[test]
fn list_schema_passes_through_complex_item() {
    let item = json!({
        "type": "object",
        "properties": {"id": {"type": "integer"}},
        "required": ["id"]
    });
    let s = list_schema(item.clone());
    assert_eq!(s["items"], item);
}

// -- schemas::code_change_schema tests --

#[test]
fn code_change_schema_has_required_fields() {
    let s = code_change_schema();
    assert_eq!(s["type"], "object");

    let props = &s["properties"];
    assert_eq!(props["file_path"]["type"], "string");
    assert_eq!(props["action"]["type"], "string");

    let required = s["required"].as_array().unwrap();
    assert!(required.iter().any(|r| r == "file_path"));
    assert!(required.iter().any(|r| r == "action"));
}

#[test]
fn code_change_schema_action_enum_values() {
    let s = code_change_schema();
    let actions = s["properties"]["action"]["enum"].as_array().unwrap();
    assert_eq!(actions.len(), 3);
    assert!(actions.iter().any(|v| v == "create"));
    assert!(actions.iter().any(|v| v == "modify"));
    assert!(actions.iter().any(|v| v == "delete"));
}

#[test]
fn code_change_schema_has_optional_content() {
    let s = code_change_schema();
    assert_eq!(s["properties"]["content"]["type"], "string");
}

// -- schemas::task_breakdown_schema tests --

#[test]
fn task_breakdown_schema_has_tasks_array() {
    let s = task_breakdown_schema();
    assert_eq!(s["type"], "object");
    assert_eq!(s["properties"]["tasks"]["type"], "array");
}

#[test]
fn task_breakdown_schema_task_item_has_title() {
    let s = task_breakdown_schema();
    let item = &s["properties"]["tasks"]["items"];
    assert_eq!(item["properties"]["title"]["type"], "string");

    let req = item["required"].as_array().unwrap();
    assert!(req.iter().any(|r| r == "title"));
}

#[test]
fn task_breakdown_schema_priority_enum() {
    let s = task_breakdown_schema();
    let priorities = s["properties"]["tasks"]["items"]["properties"]["priority"]["enum"]
        .as_array()
        .unwrap();
    assert_eq!(priorities.len(), 3);
    assert!(priorities.iter().any(|v| v == "high"));
    assert!(priorities.iter().any(|v| v == "medium"));
    assert!(priorities.iter().any(|v| v == "low"));
}

// -- schemas::decision_schema tests --

#[test]
fn decision_schema_has_required_fields() {
    let s = decision_schema();
    let req = s["required"].as_array().unwrap();
    assert!(req.iter().any(|r| r == "decision"));
    assert!(req.iter().any(|r| r == "reasoning"));
}

#[test]
fn decision_schema_alternatives_is_array() {
    let s = decision_schema();
    assert_eq!(s["properties"]["alternatives_considered"]["type"], "array");
}

// -- ValidationError tests --

#[test]
fn validation_error_parse_error_display() {
    let e = ValidationError::ParseError("bad json".to_string());
    assert!(e.to_string().contains("Parse error"));
    assert!(e.to_string().contains("bad json"));
}

#[test]
fn validation_error_schema_mismatch_display() {
    let e = ValidationError::SchemaMismatch(vec!["foo".into(), "bar".into()]);
    let msg = e.to_string();
    assert!(msg.contains("Schema mismatch"));
    assert!(msg.contains("foo"));
}

#[test]
fn validation_error_schema_error_display() {
    let e = ValidationError::SchemaError("missing type".to_string());
    assert!(e.to_string().contains("Schema error"));
}
