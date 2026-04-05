use super::{HookContext, HookError, HookResult, PreCommitHook};

pub struct HookRegistry {
    hooks: Vec<Box<dyn PreCommitHook>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    pub fn register(&mut self, hook: Box<dyn PreCommitHook>) {
        self.hooks.push(hook);
    }

    pub fn run_all(&self, ctx: &HookContext) -> Result<Vec<(String, HookResult)>, HookError> {
        let mut results = Vec::new();

        for hook in &self.hooks {
            tracing::debug!("Running pre-commit hook: {}", hook.name());
            let result = hook.run(ctx)?;
            results.push((hook.name().to_string(), result));
        }

        Ok(results)
    }
}
