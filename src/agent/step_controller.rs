//! Step Controller for Programmatic Agent Execution

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepControl {
    Continue,
    Step,
    StepAll,
    GenerateN {
        n: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        selector_prompt: Option<String>,
    },
    ToolCall {
        tool: String,
        input: Value,
    },
    WaitForInput,
    End,
}

pub type StepGenerator = Box<dyn FnMut() -> Option<StepControl> + Send + Sync>;

pub struct StepController {
    steps: VecDeque<StepControl>,
    generator: Option<StepGenerator>,
    current_index: usize,
    max_steps: usize,
}

impl Default for StepController {
    fn default() -> Self {
        Self::new()
    }
}

impl StepController {
    pub fn new() -> Self {
        Self {
            steps: VecDeque::new(),
            generator: None,
            current_index: 0,
            max_steps: 1000,
        }
    }

    pub fn with_steps(steps: Vec<StepControl>) -> Self {
        Self {
            steps: VecDeque::from(steps),
            generator: None,
            current_index: 0,
            max_steps: 1000,
        }
    }

    pub fn with_generator<G>(generator: G) -> Self
    where
        G: FnMut() -> Option<StepControl> + Send + Sync + 'static,
    {
        Self {
            steps: VecDeque::new(),
            generator: Some(Box::new(generator)),
            current_index: 0,
            max_steps: 1000,
        }
    }

    pub fn with_max_steps(mut self, max: usize) -> Self {
        self.max_steps = max;
        self
    }

    pub fn add_step(&mut self, step: StepControl) {
        self.steps.push_back(step);
    }

    pub fn add_steps(&mut self, steps: impl IntoIterator<Item = StepControl>) {
        self.steps.extend(steps);
    }

    pub fn next(&mut self) -> Option<StepControl> {
        if self.current_index >= self.max_steps {
            return Some(StepControl::End);
        }

        if let Some(step) = self.steps.pop_front() {
            self.current_index += 1;
            return Some(step);
        }

        if let Some(ref mut gen) = self.generator {
            let result = gen();
            if result.is_some() {
                self.current_index += 1;
            }
            return result;
        }

        None
    }

    pub fn peek(&self) -> Option<&StepControl> {
        self.steps.front()
    }

    pub fn has_next(&self) -> bool {
        !self.steps.is_empty() || self.generator.is_some()
    }

    pub fn step_count(&self) -> usize {
        self.current_index
    }

    pub fn reset(&mut self) {
        self.current_index = 0;
        self.steps.clear();
    }
}

pub struct StepBuilder {
    steps: Vec<StepControl>,
}

impl StepBuilder {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn step(mut self) -> Self {
        self.steps.push(StepControl::Step);
        self
    }

    pub fn step_all(mut self) -> Self {
        self.steps.push(StepControl::StepAll);
        self
    }

    pub fn continue_exec(mut self) -> Self {
        self.steps.push(StepControl::Continue);
        self
    }

    pub fn tool_call(mut self, tool: impl Into<String>, input: Value) -> Self {
        self.steps.push(StepControl::ToolCall {
            tool: tool.into(),
            input,
        });
        self
    }

    pub fn generate_n(mut self, n: usize) -> Self {
        self.steps.push(StepControl::GenerateN {
            n,
            prompt: None,
            selector_prompt: None,
        });
        self
    }

    pub fn generate_n_with_prompt(
        mut self,
        n: usize,
        prompt: impl Into<String>,
        selector_prompt: Option<String>,
    ) -> Self {
        self.steps.push(StepControl::GenerateN {
            n,
            prompt: Some(prompt.into()),
            selector_prompt,
        });
        self
    }

    pub fn wait_for_input(mut self) -> Self {
        self.steps.push(StepControl::WaitForInput);
        self
    }

    pub fn end(mut self) -> Self {
        self.steps.push(StepControl::End);
        self
    }

    pub fn build(self) -> StepController {
        StepController::with_steps(self.steps)
    }
}

impl Default for StepBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Iterator for StepController {
    type Item = StepControl;

    fn next(&mut self) -> Option<Self::Item> {
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.steps.len();
        (remaining, Some(remaining))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let input = serde_json::json!({"path": "test.txt"});
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
}
