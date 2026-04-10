//! Step Controller tests

use crate::agent::step_controller::{StepBuilder, StepControl, StepController};
use serde_json::json;

#[test]
fn test_step_builder() {
    let mut controller = StepBuilder::new().step().step().step_all().build();

    assert_eq!(controller.next(), Some(StepControl::Step));
    assert_eq!(controller.next(), Some(StepControl::Step));
    assert_eq!(controller.next(), Some(StepControl::StepAll));
    assert_eq!(controller.next(), None);
}

#[test]
fn test_tool_call_step() {
    let input = json!({"path": "test.txt"});
    let mut controller = StepBuilder::new().tool_call("Read", input.clone()).build();

    match controller.next() {
        Some(StepControl::ToolCall { tool, input: _inp }) => {
            assert_eq!(tool, "Read");
        }
        _ => panic!("Expected ToolCall step"),
    }
}

#[test]
fn test_generator_steps() {
    let mut counter = 0;
    let mut controller = StepController::with_generator(move || {
        counter += 1;
        if counter <= 3 {
            Some(StepControl::Step)
        } else {
            None
        }
    });

    assert_eq!(controller.next(), Some(StepControl::Step));
    assert_eq!(controller.next(), Some(StepControl::Step));
    assert_eq!(controller.next(), Some(StepControl::Step));
    assert_eq!(controller.next(), None);
}

#[test]
fn test_max_steps() {
    let mut controller = StepController::with_steps(vec![
        StepControl::Step,
        StepControl::Step,
        StepControl::Step,
    ])
    .with_max_steps(2);

    assert_eq!(controller.next(), Some(StepControl::Step));
    assert_eq!(controller.next(), Some(StepControl::Step));
    assert_eq!(controller.next(), Some(StepControl::End));
}

#[test]
fn test_default_controller() {
    let controller = StepController::default();
    assert!(!controller.has_next());
    assert_eq!(controller.step_count(), 0);
    assert_eq!(controller.peek(), None);
}

#[test]
fn test_with_steps_constructor() {
    let controller = StepController::with_steps(vec![StepControl::Continue, StepControl::End]);
    assert!(controller.has_next());
    assert_eq!(controller.step_count(), 0);
}

#[test]
fn test_add_steps_batch() {
    let mut controller = StepController::new();
    controller.add_steps(vec![StepControl::Step, StepControl::Continue]);
    assert_eq!(controller.step_count(), 0);
    assert_eq!(controller.next(), Some(StepControl::Step));
    assert_eq!(controller.next(), Some(StepControl::Continue));
}

#[test]
fn test_reset_controller() {
    let mut controller = StepBuilder::new().step().step_all().continue_exec().build();
    controller.next();
    controller.next();
    assert_eq!(controller.step_count(), 2);
    controller.reset();
    assert_eq!(controller.step_count(), 0);
    assert!(!controller.has_next());
}

#[test]
fn test_step_control_serialization() {
    let step = StepControl::Continue;
    let yaml = serde_yaml::to_string(&step).unwrap();
    assert!(yaml.contains("continue"));

    let parsed: StepControl = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(step, parsed);
}

#[test]
fn test_step_control_generate_n_serialization() {
    let step = StepControl::GenerateN {
        n: 3,
        prompt: Some("test prompt".to_string()),
        selector_prompt: None,
    };
    let yaml = serde_yaml::to_string(&step).unwrap();
    let parsed: StepControl = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(step, parsed);
}

#[test]
fn test_step_control_generate_n_skips_none() {
    let step = StepControl::GenerateN {
        n: 2,
        prompt: None,
        selector_prompt: None,
    };
    let yaml = serde_yaml::to_string(&step).unwrap();
    assert!(!yaml.contains("prompt"));
}

#[test]
fn test_generate_n_with_prompt_builder() {
    let mut controller = StepBuilder::new()
        .generate_n_with_prompt(3, "pick the best", Some("choose: ".to_string()))
        .build();

    match controller.next() {
        Some(StepControl::GenerateN {
            n,
            prompt,
            selector_prompt,
        }) => {
            assert_eq!(n, 3);
            assert_eq!(prompt, Some("pick the best".to_string()));
            assert_eq!(selector_prompt, Some("choose: ".to_string()));
        }
        _ => panic!("Expected GenerateN step"),
    }
}

#[test]
fn test_generate_n_builder() {
    let mut controller = StepBuilder::new().generate_n(5).build();

    match controller.next() {
        Some(StepControl::GenerateN { n, .. }) => assert_eq!(n, 5),
        _ => panic!("Expected GenerateN step"),
    }
}

#[test]
fn test_wait_for_input_builder() {
    let mut controller = StepBuilder::new().wait_for_input().build();
    assert_eq!(controller.next(), Some(StepControl::WaitForInput));
}

#[test]
fn test_end_builder() {
    let mut controller = StepBuilder::new().end().build();
    assert_eq!(controller.next(), Some(StepControl::End));
}

#[test]
fn test_peek_doesnt_advance() {
    let controller = StepController::with_steps(vec![StepControl::Step, StepControl::End]);
    assert_eq!(controller.peek(), Some(&StepControl::Step));
    assert_eq!(controller.peek(), Some(&StepControl::Step));
}
