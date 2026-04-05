//! Model Pricing Cache
//!
//! Fetches and caches dynamic pricing from models.dev

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use tracing::{debug, info, warn};

const MODELS_DEV_URL: &str = "https://models.dev/api.json";
const CACHE_FILE_NAME: &str = "models.json";
const MAX_CACHE_AGE_SECS: u64 = 60 * 60; // 60 minutes (OpenCode parity)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostData {
    pub input: f64,
    pub output: f64,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelData {
    pub id: String,
    pub name: String,
    pub cost: Option<CostData>,
}

pub type ModelsManifest = HashMap<String, ModelData>;

/// Get the path to the current user's models.json cache
pub fn get_cache_path() -> PathBuf {
    PathBuf::from(crate::config::get_global_config_dir()).join(CACHE_FILE_NAME)
}

/// Checks if the cache exists and is fresh
pub fn is_cache_fresh() -> bool {
    let path = get_cache_path();
    if !path.exists() {
        return false;
    }

    if let Ok(metadata) = fs::metadata(&path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = SystemTime::now().duration_since(modified) {
                return duration.as_secs() < MAX_CACHE_AGE_SECS;
            }
        }
    }
    false
}

/// Fetches the latest models JSON from models.dev and writes to cache
pub async fn fetch_and_cache_pricing() -> Result<()> {
    info!("Fetching dynamic model pricing from {}", MODELS_DEV_URL);
    let client = reqwest::Client::builder()
        .user_agent("d3vx-terminal")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let response = client.get(MODELS_DEV_URL).send().await?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch models.dev API: HTTP {}", response.status());
    }

    let raw_text = response.text().await?;

    // Create config dir if not exists
    let cache_path = get_cache_path();
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Attempt to parse just to ensure it's valid JSON before wiping the old cache
    let _parsed: serde_json::Value =
        serde_json::from_str(&raw_text).context("Invalid JSON received from models.dev")?;

    fs::write(&cache_path, raw_text)?;
    debug!(
        "Successfully cached models.json to {}",
        cache_path.display()
    );

    Ok(())
}

/// Loads the manifest from the specified cache unconditionally
pub fn load_manifest() -> Option<ModelsManifest> {
    let path = get_cache_path();
    if !path.exists() {
        return None;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to read models cache at {}: {}", path.display(), e);
            return None;
        }
    };

    match serde_json::from_str::<ModelsManifest>(&content) {
        Ok(manifest) => Some(manifest),
        Err(e) => {
            warn!("Failed to parse models cache: {}", e);
            None
        }
    }
}

/// Reads the pricing for a specific model from the cache
pub fn get_model_pricing(model_id: &str) -> Option<crate::agent::cost::ModelPricing> {
    let manifest = load_manifest()?;

    let data = manifest.get(model_id)?;
    let cost = data.cost.as_ref()?;

    Some(crate::agent::cost::ModelPricing {
        input: cost.input,
        output: cost.output,
        cache_read: cost.cache_read.unwrap_or(0.0),
    })
}
