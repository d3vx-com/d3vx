//! Tests for GitHub Configuration
//!
//! Covers configuration options and defaults.

#[cfg(test)]
mod tests {
    use crate::pipeline::github::GitHubConfig;

    // =========================================================================
    // Default Configuration Tests
    // =========================================================================

    #[test]
    fn test_default_config() {
        let config = GitHubConfig::default();

        assert!(config.repositories.is_empty());
        assert!(!config.trigger_labels.is_empty());
        assert!(config.trigger_labels.contains(&"d3vx".to_string()));
        assert!(config.auto_process_labels.contains(&"d3vx-auto".to_string()));
        assert_eq!(config.poll_interval_secs, 300);
        assert!(config.sync_status);
        assert_eq!(config.token_env, "GITHUB_TOKEN");
        assert_eq!(config.api_base_url, "https://api.github.com");
    }

    #[test]
    fn test_default_trigger_labels() {
        let config = GitHubConfig::default();

        assert!(config.trigger_labels.contains(&"d3vx".to_string()));
        assert!(config.trigger_labels.contains(&"ai-assist".to_string()));
    }

    #[test]
    fn test_default_auto_process_labels() {
        let config = GitHubConfig::default();

        assert!(config.auto_process_labels.contains(&"d3vx-auto".to_string()));
    }

    // =========================================================================
    // Custom Configuration Tests
    // =========================================================================

    #[test]
    fn test_custom_repositories() {
        let config = GitHubConfig {
            repositories: vec![
                "owner/repo1".to_string(),
                "owner/repo2".to_string(),
            ],
            ..Default::default()
        };

        assert_eq!(config.repositories.len(), 2);
    }

    #[test]
    fn test_custom_poll_interval() {
        let config = GitHubConfig {
            poll_interval_secs: 60,
            ..Default::default()
        };

        assert_eq!(config.poll_interval_secs, 60);
    }

    #[test]
    fn test_webhook_secret() {
        let config = GitHubConfig {
            webhook_secret: Some("my-secret-key".to_string()),
            ..Default::default()
        };

        assert!(config.webhook_secret.is_some());
        assert_eq!(config.webhook_secret.unwrap(), "my-secret-key");
    }

    #[test]
    fn test_custom_api_url() {
        let config = GitHubConfig {
            api_base_url: "https://github.enterprise.com/api/v3".to_string(),
            ..Default::default()
        };

        assert_eq!(config.api_base_url, "https://github.enterprise.com/api/v3");
    }

    #[test]
    fn test_sync_status_disabled() {
        let config = GitHubConfig {
            sync_status: false,
            ..Default::default()
        };

        assert!(!config.sync_status);
    }

    // =========================================================================
    // Configuration Validation Tests
    // =========================================================================

    #[test]
    fn test_empty_repositories_is_valid() {
        let config = GitHubConfig {
            repositories: vec![],
            ..Default::default()
        };

        // Empty repositories is valid (may be populated by discovery)
        assert!(config.repositories.is_empty());
    }

    #[test]
    fn test_custom_token_env() {
        let config = GitHubConfig {
            token_env: "GITHUB_API_TOKEN".to_string(),
            ..Default::default()
        };

        assert_eq!(config.token_env, "GITHUB_API_TOKEN");
    }
}
