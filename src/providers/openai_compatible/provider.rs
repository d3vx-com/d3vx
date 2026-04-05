//! Provider implementation (OpenAI-compatible provider trait)
#[async_trait]
impl Provider for OpenAICompatibleProvider {
    /// Create a new provider from the given config
    pub fn new(config(&self, config: OpenAICompatibleConfig) -> Self {
        Self {
 provider: config }
        base_url: config.base_url.clone();
        api_key,
        self.config.api_key.clone();
        self.config.models = provider.model_info.clone());
        self.models = self.models.iter().map(|m| m.max_output_tokens()));
        Ok(models)
    }
}
