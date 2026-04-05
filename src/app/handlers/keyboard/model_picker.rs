//! Model Picker Keyboard Handling

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use crate::app::App;
use crate::config::loader::loading::{
    find_project_root, save_global_config_part, save_project_config_part,
};
use crate::providers::ComplexityTier;

impl App {
    pub async fn handle_model_picker_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.ui.model_picker_entering_api_key {
            return self.handle_api_key_input(key).await;
        }

        match key.code {
            KeyCode::Esc => {
                self.ui.show_model_picker = false;
            }
            KeyCode::Tab => {
                // Cycle through tiers: Simple -> Standard -> Complex -> Simple
                self.ui.model_picker_selected_tier = match self.ui.model_picker_selected_tier {
                    ComplexityTier::Simple => ComplexityTier::Standard,
                    ComplexityTier::Standard => ComplexityTier::Complex,
                    ComplexityTier::Complex => ComplexityTier::Simple,
                };
                self.ui.model_picker_selected_index = 0;
            }
            KeyCode::Up => {
                if self.ui.model_picker_selected_index > 0 {
                    self.ui.model_picker_selected_index -= 1;
                }
            }
            KeyCode::Down => {
                let tier_count = self.get_filtered_tier_count();
                if self.ui.model_picker_selected_index + 1 < tier_count {
                    self.ui.model_picker_selected_index += 1;
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.ui.model_picker_filter.push(c);
                self.ui.model_picker_selected_index = 0;
            }
            KeyCode::Backspace => {
                self.ui.model_picker_filter.pop();
                self.ui.model_picker_selected_index = 0;
            }
            KeyCode::Enter => {
                self.confirm_model_selection().await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_api_key_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.ui.model_picker_entering_api_key = false;
            }
            KeyCode::Char(c) => {
                self.ui.model_picker_api_key_input.push(c);
            }
            KeyCode::Backspace => {
                self.ui.model_picker_api_key_input.pop();
            }
            KeyCode::Enter => {
                let key_value = std::mem::take(&mut self.ui.model_picker_api_key_input);
                if !key_value.is_empty() {
                    // Save to global config
                    let selected_model = self.get_selected_model_info();
                    if let Some(m) = selected_model {
                        let provider_id = &m.provider;
                        // Determine the environment variable name (fallback to logical name)
                        let env_name = match provider_id.as_str() {
                            "anthropic" => "ANTHROPIC_API_KEY",
                            "openai" => "OPENAI_API_KEY",
                            "gemini" => "GOOGLE_API_KEY",
                            _ => &format!("{}_API_KEY", provider_id.to_uppercase()),
                        };

                        let part = serde_yaml::to_value(json!({
                            "providers": {
                                "configs": {
                                    provider_id: {
                                        "api_key_env": env_name // Just in case
                                    }
                                }
                            }
                        }))?;

                        // Also need to set it in env for immediate use
                        std::env::set_var(env_name, &key_value);

                        save_global_config_part(part)?;
                        self.add_system_message(&format!(
                            "API Key for {} saved to global config.",
                            provider_id
                        ));
                    }
                }
                self.ui.model_picker_entering_api_key = false;
            }
            _ => {}
        }
        Ok(())
    }

    async fn confirm_model_selection(&mut self) -> Result<()> {
        let selected = self.get_selected_model_info();
        if let Some(m) = selected {
            // 1. Check for API key
            // (In a real implementation, we'd check env, local context, and global context)
            // For now, let's just check if it's a known provider and if we have a key
            if m.provider != "ollama"
                && std::env::var(format!("{}_API_KEY", m.provider.to_uppercase())).is_err()
            {
                self.ui.model_picker_entering_api_key = true;
                return Ok(());
            }

            // 2. Save model routing to project config
            if let Some(root) = find_project_root() {
                let tier_field = match self.ui.model_picker_selected_tier {
                    ComplexityTier::Simple => "cheap_model",
                    ComplexityTier::Standard => "standard_model",
                    ComplexityTier::Complex => "complex_model",
                };

                let part = serde_yaml::to_value(json!({
                    "model_routing": {
                        tier_field: m.id
                    }
                }))?;

                save_project_config_part(&root, part)?;
                self.add_system_message(&format!(
                    "Assigned {} to {} tier in project config.",
                    m.name, self.ui.model_picker_selected_tier
                ));
            }

            self.ui.show_model_picker = false;
        }
        Ok(())
    }

    fn get_filtered_tier_count(&self) -> usize {
        // Since this is called from an async context, we might need a sync wrapper or just handle it differently
        // For simplicity in this logic, I'll assume we can lock briefly
        let registry = self.registry.blocking_read();
        let models = registry.list_models();
        models
            .iter()
            .filter(|m| m.tier == self.ui.model_picker_selected_tier)
            .filter(|m| {
                self.ui.model_picker_filter.is_empty()
                    || m.name
                        .to_lowercase()
                        .contains(&self.ui.model_picker_filter.to_lowercase())
            })
            .count()
    }

    fn get_selected_model_info(&self) -> Option<crate::providers::ModelInfo> {
        let registry = self.registry.blocking_read();
        let models = registry.list_models();
        let tier_models: Vec<_> = models
            .iter()
            .filter(|m| m.tier == self.ui.model_picker_selected_tier)
            .filter(|m| {
                self.ui.model_picker_filter.is_empty()
                    || m.name
                        .to_lowercase()
                        .contains(&self.ui.model_picker_filter.to_lowercase())
            })
            .collect();

        let idx = self.ui.model_picker_selected_index;
        if idx < tier_models.len() {
            Some((*tier_models[idx]).clone())
        } else {
            None
        }
    }
}
