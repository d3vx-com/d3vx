use crate::agent::StepControl;
use crate::tools::{best_of_n_tool::BestOfNTool, Tool, ToolContext};

#[tokio::test]
async fn best_of_n_tool_returns_step_control_metadata() {
    let tool = BestOfNTool::new();
    let input = serde_json::json!({
        "prompt": "Implement the strongest variant",
        "n": 4,
        "selector_prompt": "Prefer the variant with the strongest tests"
    });

    let result = tool.execute(input, &ToolContext::default()).await;

    assert!(!result.is_error);
    let step_control = result
        .metadata
        .get("step_control")
        .cloned()
        .expect("expected step_control metadata");
    let parsed: StepControl =
        serde_json::from_value(step_control).expect("step_control should deserialize");

    match parsed {
        StepControl::GenerateN {
            n,
            prompt,
            selector_prompt,
        } => {
            assert_eq!(n, 4);
            assert_eq!(prompt.as_deref(), Some("Implement the strongest variant"));
            assert_eq!(
                selector_prompt.as_deref(),
                Some("Prefer the variant with the strongest tests")
            );
        }
        other => panic!("unexpected step control: {:?}", other),
    }
}
