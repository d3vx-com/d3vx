use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn};

use crate::providers::{ComplexityTier, ModelInfo, Provider, ProviderError};

/// Remote model definition from models.dev
#[derive(Debug, Clone, Deserialize, Serialize)]
struct RemoteModel {
    name: String,
    #[serde(default)]
    reasoning: bool,
    #[serde(default)]
    cost: Option<RemoteCost>,
    #[serde(default)]
    limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RemoteCost {
    input: f64,
    output: f64,
}

/// Remote provider definition from models.dev
#[derive(Debug, Clone, Deserialize, Serialize)]
struct RemoteProvider {
    api: String,
    name: String,
    models: HashMap<String, RemoteModel>,
}

/// The central source of truth for all models available to the system.
pub struct ModelRegistry {
    models: HashMap<String, ModelInfo>,
    providers: HashMap<String, Box<dyn Provider>>,
}

impl ModelRegistry {
    /// Create a new empty registry and fetch remote models.
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            providers: HashMap::new(),
        }
    }

    /// Register a provider and its static models.
    pub fn register_provider(&mut self, provider: Box<dyn Provider>) {
        let name = provider.name().to_string();
        info!("Registering provider: {}", name);

        for model in provider.models() {
            self.models.insert(model.id.clone(), model);
        }

        self.providers.insert(name, provider);
    }

    /// Dynamically discover models from all registered providers and remote registry.
    pub async fn discover_all(&mut self) -> Result<(), ProviderError> {
        // 1. Fetch from models.dev
        match self.fetch_remote_models().await {
            Ok(remote_models) => {
                info!(
                    "Fetched {} remote models from models.dev",
                    remote_models.len()
                );
                for model in remote_models {
                    self.models.insert(model.id.clone(), model);
                }
            }
            Err(e) => {
                warn!("Failed to fetch remote models: {}", e);
            }
        }

        // 2. Discover from local providers (Ollama, etc.)
        let mut new_models = Vec::new();
        for provider in self.providers.values() {
            match provider.discover_models().await {
                Ok(models) => {
                    info!(
                        "Discovered {} models from {}",
                        models.len(),
                        provider.name()
                    );
                    new_models.extend(models);
                }
                Err(e) => {
                    warn!("Failed to discover models from {}: {}", provider.name(), e);
                }
            }
        }

        for model in new_models {
            self.models.insert(model.id.clone(), model);
        }

        Ok(())
    }

    /// Fetch models from models.dev with 24h caching.
    pub async fn fetch_remote_models(&self) -> Result<Vec<ModelInfo>> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("d3vx");
        let cache_path = cache_dir.join("models_remote.json");

        // Check cache
        if cache_path.exists() {
            let metadata = fs::metadata(&cache_path)?;
            let modified = metadata.modified()?;
            let age = SystemTime::now()
                .duration_since(modified)
                .unwrap_or(Duration::MAX);

            if age < Duration::from_secs(24 * 3600) {
                debug!("Loading remote models from cache: {}", cache_path.display());
                let content = fs::read_to_string(&cache_path)?;
                return self.parse_remote_json(&content);
            }
        }

        // Fetch fresh
        info!("Fetching fresh model list from models.dev...");
        let response = reqwest::get("https://models.dev/api.json")
            .await?
            .text()
            .await?;

        // Save to cache
        fs::create_dir_all(&cache_dir)?;
        fs::write(&cache_path, &response)?;
        debug!("Cached remote models to {}", cache_path.display());

        self.parse_remote_json(&response)
    }

    fn parse_remote_json(&self, json: &str) -> Result<Vec<ModelInfo>> {
        let data: HashMap<String, RemoteProvider> =
            serde_json::from_str(json).context("Failed to parse models.dev API response")?;

        let mut models = Vec::new();
        for (provider_id, provider_data) in data {
            for (model_id, model_data) in provider_data.models {
                // Heuristic for tiering
                let tier = if model_data.reasoning {
                    ComplexityTier::Complex
                } else if model_data.cost.as_ref().map_or(false, |c| c.input > 10.0) {
                    ComplexityTier::Complex
                } else if model_data.cost.as_ref().map_or(false, |c| c.input < 0.2) {
                    ComplexityTier::Simple
                } else {
                    ComplexityTier::Standard
                };

                models.push(ModelInfo {
                    id: format!("{}/{}", provider_id, model_id),
                    name: model_data.name,
                    provider: provider_id.clone(),
                    tier,
                    context_window: model_data.limit.unwrap_or(32000),
                    max_output_tokens: 4096, // Fallback
                    supports_tool_use: true, // Most modern models do
                    supports_vision: false,  // Hard to detect from this API alone
                    supports_streaming: true,
                    supports_thinking: model_data.reasoning,
                    default_thinking_budget: if model_data.reasoning {
                        Some(4000)
                    } else {
                        None
                    },
                    cost_per_input_mtok: model_data.cost.as_ref().map(|c| c.input),
                    cost_per_output_mtok: model_data.cost.as_ref().map(|c| c.output),
                });
            }
        }

        Ok(models)
    }

    /// Get information about a specific model.
    pub fn get_model(&self, model_id: &str) -> Option<&ModelInfo> {
        self.models.get(model_id)
    }

    /// List all registered models.
    pub fn list_models(&self) -> Vec<&ModelInfo> {
        let mut list: Vec<_> = self.models.values().collect();
        list.sort_by_key(|m| &m.id);
        list
    }

    /// List all registered providers.
    pub fn list_providers(&self) -> Vec<&str> {
        let mut list: Vec<_> = self.providers.keys().map(|s| s.as_str()).collect();
        list.sort();
        list
    }

    /// Get a specific provider by name.
    pub fn get_provider(&self, name: &str) -> Option<&dyn Provider> {
        self.providers.get(name).map(|p| p.as_ref())
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}
