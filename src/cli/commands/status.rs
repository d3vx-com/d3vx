//! Status Command Implementation
//!
//! Display project and provider status information.

use anyhow::{Context, Result};

use crate::config::{get_provider_config, load_config, LoadConfigOptions};
use crate::utils::project::detect_project;

pub(crate) async fn execute_status() -> Result<()> {
    let project_root = std::env::current_dir().context("Failed to get current directory")?;
    let d3vx_dir = project_root.join(".d3vx");

    println!("d3vx Status\n");
    println!("Project: {}", project_root.display());
    println!(
        "Initialized: {}",
        if d3vx_dir.exists() { "Yes" } else { "No" }
    );

    // Detect project info
    let detected = detect_project(&project_root);
    println!(
        "Detected Stack: {} / {}",
        detected.language, detected.framework
    );

    // Load config
    let config_result = load_config(LoadConfigOptions {
        project_root: Some(project_root.to_string_lossy().to_string()),
        ..Default::default()
    });

    if let Ok(config) = config_result {
        let (model, api_key, base_url) = get_provider_config(&config);
        println!("Provider: {}", config.provider);
        println!("Model: {}", model);
        println!(
            "API Key configured: {}",
            if api_key.is_some() { "Yes" } else { "No" }
        );
        if let Some(url) = base_url {
            println!("Base URL: {}", url);
        }
    } else {
        println!("Configuration: Not loaded or invalid.");
    }

    Ok(())
}
