use super::types::*;
use super::traits::{HookCategory, HookContext, HookError, HookResult, PreCommitHook};
use super::security::SecurityHook;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

impl HookRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
            config: HookRegistryConfig::default(),
        }
    }

    /// Create a registry with configuration
    pub fn with_config(config: HookRegistryConfig) -> Self {
        Self {
            hooks: HashMap::new(),
            config,
        }
    }

    /// Create a registry with auto-detected hooks for the given path
    pub fn for_project(path: impl AsRef<Path>) -> Self {
        let detector = ProjectDetector::new(path.as_ref());
        let project_info = detector.detect();

        let mut registry = Self::new();
        registry.register_hooks_for_project(project_info);
        registry
    }

    /// Register hooks based on detected project info
    pub fn register_hooks_for_project(&mut self, project_info: super::detector::ProjectInfo) {
        // Register format hook if formatter detected
        if self.config.format {
            let hook = FormatHook::new(project_info.clone());
            self.register(Box::new(hook));
        }

        // Register lint hook if linter detected
        if self.config.lint {
            let hook = LintHook::new(project_info.clone());
            self.register(Box::new(hook));
        }

        // Register test hook if test framework detected
        if self.config.test {
            let hook = TestHook::new(project_info.clone());
            self.register(Box::new(hook));
        }

        // Register security hook (always available, language-agnostic)
        if self.config.security {
            let hook = SecurityHook::new();
            self.register(Box::new(hook));
        }
    }

    /// Register a hook
    pub fn register(&mut self, hook: Box<dyn PreCommitHook>) {
        let id = hook.id().to_string();
        self.hooks.insert(id, Arc::from(hook));
    }

    /// Set configuration
    pub fn set_config(&mut self, config: HookRegistryConfig) {
        self.config = config;
    }

    /// Check if hooks are enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if this is a WIP commit
    fn is_wip_commit(&self, message: &str) -> bool {
        let lower = message.to_lowercase();
        lower.contains("wip")
            || lower.contains("work in progress")
            || lower.contains("draft")
    }

    /// Get the list of hooks to run
    fn get_hooks_to_run(&self) -> Vec<Arc<dyn PreCommitHook>> {
        self.hooks.values().cloned().collect()
    }

    /// Run all applicable hooks
    pub fn run_all(&self, mut ctx: HookContext) -> Result<HooksRunResult, HookError> {
        let start = std::time::Instant::now();
        let mut results = HashMap::new();

        // Check if hooks are enabled
        if !self.config.enabled {
            info!("Pre-commit hooks are disabled");
            return Ok(HooksRunResult {
                success: true,
                results,
                total_duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Check for WIP commit
        if self.config.skip_if_wip && self.is_wip_commit(&ctx.commit_message) {
            info!("Skipping pre-commit hooks for WIP commit");
            return Ok(HooksRunResult {
                success: true,
                results,
                total_duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Set default timeout if not specified
        if ctx.timeout_seconds == 0 {
            ctx.timeout_seconds = self.config.timeout_seconds;
        }

        let hooks_to_run = self.get_hooks_to_run();
        let mut all_success = true;

        info!(hooks_count = hooks_to_run.len(), "Running pre-commit hooks");

        for hook in hooks_to_run {
            let id = hook.id().to_string();
            let name = hook.name().to_string();
            let category = hook.category();
            let hook_start = std::time::Instant::now();

            // Check if hook is applicable
            if !hook.is_applicable(&ctx) {
                debug!(hook = %id, "Hook not applicable, skipping");
                results.insert(id.clone(), HookRunInfo {
                    id: id.clone(),
                    name,
                    category,
                    result: HookResult::Skip("Not applicable".to_string()),
                    duration_ms: 0,
                    error: None,
                });
                continue;
            }

            debug!(hook = %id, "Running hook");

            match hook.run(&ctx) {
                Ok(result) => {
                    let duration_ms = hook_start.elapsed().as_millis() as u64;

                    if !result.is_success() {
                        all_success = false;
                        warn!(hook = %id, "Hook failed");
                    } else {
                        debug!(hook = %id, duration_ms, "Hook passed");
                    }

                    results.insert(id.clone(), HookRunInfo {
                        id: id.clone(),
                        name,
                        category,
                        result,
                        duration_ms,
                        error: None,
                    });
                }
                Err(e) => {
                    let duration_ms = hook_start.elapsed().as_millis() as u64;
                    all_success = false;

                    warn!(hook = %id, error = %e, "Hook errored");

                    results.insert(id.clone(), HookRunInfo {
                        id: id.clone(),
                        name,
                        category,
                        result: HookResult::Fail(e.to_string()),
                        duration_ms,
                        error: Some(e.to_string()),
                    });
                }
            }

            // Stop running hooks if one failed (fail-fast)
            if !all_success {
                break;
            }
        }

        let total_duration_ms = start.elapsed().as_millis() as u64;

        if all_success {
            info!(duration_ms = total_duration_ms, "All pre-commit hooks passed");
        } else {
            warn!(duration_ms = total_duration_ms, "Pre-commit hooks failed");
        }

        Ok(HooksRunResult {
            success: all_success,
            results,
            total_duration_ms,
        })
    }

    /// Run hooks and return a summary message
    pub fn run_and_summarize(&self, ctx: HookContext) -> Result<(bool, String), HookError> {
        let result = self.run_all(ctx)?;

        let mut message = if result.success {
            String::from("All pre-commit checks passed!\n")
        } else {
            String::from("Pre-commit checks failed:\n")
        };

        // Sort results by category order
        let category_order = [HookCategory::Format, HookCategory::Lint, HookCategory::Test, HookCategory::Security];

        for category in &category_order {
            for (id, info) in &result.results {
                if info.category == *category {
                    let status = match &info.result {
                        HookResult::Pass => "PASS",
                        HookResult::Fail(_) => "FAIL",
                        HookResult::Skip(_) => "SKIP",
                    };

                    message.push_str(&format!(
                        "  [{:>5}] {} ({}ms)\n",
                        status, info.name, info.duration_ms
                    ));

                    if let HookResult::Fail(reason) = &info.result {
                        for line in reason.lines().take(10) {
                            message.push_str(&format!("         {}\n", line));
                        }
                        if reason.lines().count() > 10 {
                            message.push_str("         ... (truncated)\n");
                        }
                    }

                    if let HookResult::Skip(reason) = &info.result {
                        message.push_str(&format!("         ({})\n", reason));
                    }
                }
            }
        }

        message.push_str(&format!("\nTotal time: {}ms\n", result.total_duration_ms));

        Ok((result.success, message))
    }

    /// Get list of registered hook IDs
    pub fn hook_ids(&self) -> Vec<String> {
        self.hooks.keys().cloned().collect()
    }

    /// Check if a hook is registered
    pub fn has_hook(&self, id: &str) -> bool {
        self.hooks.contains_key(id)
    }
}

#[cfg(test)]
