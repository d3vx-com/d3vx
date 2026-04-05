use super::types::*;
use super::traits::HookContext;


    #[test]
    fn test_hook_registry_new() {
        let registry = HookRegistry::new();
        assert!(registry.hooks.is_empty());
        assert!(registry.is_enabled());
    }

    #[test]
    fn test_hook_registry_disabled() {
        let config = HookRegistryConfig {
            enabled: false,
            ..Default::default()
        };
        let registry = HookRegistry::with_config(config);
        assert!(!registry.is_enabled());
    }

    #[test]
    fn test_hook_registry_is_wip() {
        let registry = HookRegistry::new();

        assert!(registry.is_wip_commit("WIP: working on feature"));
        assert!(registry.is_wip_commit("wip commit"));
        assert!(registry.is_wip_commit("DRAFT: initial implementation"));
        assert!(registry.is_wip_commit("Work in progress"));
        assert!(!registry.is_wip_commit("feat: add new feature"));
        assert!(!registry.is_wip_commit("fix: resolve bug"));
    }

    #[test]
    fn test_hook_registry_run_all_disabled() {
        let config = HookRegistryConfig {
            enabled: false,
            ..Default::default()
        };
        let registry = HookRegistry::with_config(config);
        let ctx = HookContext::default();

        let result = registry.run_all(ctx).unwrap();
        assert!(result.success);
        assert!(result.results.is_empty());
    }

    #[test]
    fn test_hook_registry_run_all_wip() {
        let registry = HookRegistry::new();
        let ctx = HookContext {
            commit_message: "WIP: work in progress".to_string(),
            ..Default::default()
        };

        let result = registry.run_all(ctx).unwrap();
        assert!(result.success);
        assert!(result.results.is_empty()); // No hooks run for WIP
    }

    #[test]
    fn test_hooks_run_result_success() {
        let result = HooksRunResult {
            success: true,
            results: HashMap::new(),
            total_duration_ms: 100,
        };
        assert!(result.success);
    }

    #[test]
    fn test_hook_run_info_is_success() {
        let info = HookRunInfo {
            id: "test".to_string(),
            name: "Test".to_string(),
            category: HookCategory::Format,
            result: HookResult::Pass,
            duration_ms: 10,
            error: None,
        };
        assert!(info.is_success());

        let info = HookRunInfo {
            id: "test".to_string(),
            name: "Test".to_string(),
            category: HookCategory::Format,
            result: HookResult::Skip("reason".to_string()),
            duration_ms: 10,
            error: None,
        };
        assert!(info.is_success());

        let info = HookRunInfo {
            id: "test".to_string(),
            name: "Test".to_string(),
            category: HookCategory::Format,
            result: HookResult::Fail("error".to_string()),
            duration_ms: 10,
            error: None,
        };
        assert!(!info.is_success());

        let info = HookRunInfo {
            id: "test".to_string(),
            name: "Test".to_string(),
            category: HookCategory::Format,
            result: HookResult::Pass,
            duration_ms: 10,
            error: Some("error".to_string()),
        };
        assert!(!info.is_success());
    }
}

