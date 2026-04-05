use anyhow::Result;
use clap::Parser;
use d3vx::cli::{execute, Cli};
use tracing::info;
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env if present
    let _ = dotenvy::dotenv();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging to a file in ~/.d3vx/d3vx.log
    let log_dir = d3vx::config::get_global_config_dir();
    let log_path = std::path::Path::new(&log_dir).join("d3vx.log");

    // Ensure log directory exists
    let _ = std::fs::create_dir_all(&log_dir);

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    let log_level = if cli.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(log_level.into())
                .from_env_lossy(),
        )
        .with_writer(file)
        .init();

    info!("Logging initialized to {}", log_path.display());

    // Start background model pricing refresh (matches opencode's 60-minute fetch)
    tokio::spawn(async move {
        // Fetch immediately on startup if the existing cache is missing or stale (> 60m)
        if !d3vx::providers::pricing_cache::is_cache_fresh() {
            let _ = d3vx::providers::pricing_cache::fetch_and_cache_pricing().await;
        }

        // Then continuously poll every 60 minutes implicitly
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60 * 60)).await;
            let _ = d3vx::providers::pricing_cache::fetch_and_cache_pricing().await;
        }
    });

    // Execute the CLI
    execute(cli).await
}
