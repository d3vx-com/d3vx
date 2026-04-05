use std::any::Any;
use std::collections::HashMap;
use tracing::{info, warn};

use super::core::{Plugin, PluginContext, PluginError};
use super::slots::{
    AgentBackendAdapter, NotifierAdapter, PluginDescriptor, PluginSlot, RuntimeAdapter, ScmAdapter,
    TerminalAdapter, TrackerAdapter, WorkspaceAdapter,
};

/// Unified Plugin Registry for managing extensions and adapters.
pub struct PluginRegistry {
    extensions: HashMap<String, Box<dyn Plugin>>,
    adapters: HashMap<PluginSlot, (PluginDescriptor, Box<dyn Any + Send + Sync>)>,
    context: PluginContext,
}

impl PluginRegistry {
    /// Create a new empty registry with default context.
    pub fn new() -> Self {
        Self {
            extensions: HashMap::new(),
            adapters: HashMap::new(),
            context: PluginContext::new(),
        }
    }

    /// Set the plugin context.
    pub fn with_context(mut self, context: PluginContext) -> Self {
        self.context = context;
        self
    }

    /// Register a general extension plugin.
    pub async fn register_extension(&mut self, plugin: Box<dyn Plugin>) -> Result<(), PluginError> {
        let name = plugin.name().to_string();
        plugin.init(&self.context)?;
        self.extensions.insert(name, plugin);
        Ok(())
    }

    /// Register an adapter plugin into a specific slot.
    pub fn register_adapter(
        &mut self,
        slot: PluginSlot,
        desc: PluginDescriptor,
        plugin: Box<dyn Any + Send + Sync>,
    ) {
        info!("Registering {} plugin: {}", slot, desc.name);
        self.adapters.insert(slot, (desc, plugin));
    }

    /// Get a typed reference for a slot.
    pub fn get_adapter<T: 'static>(&self, slot: PluginSlot) -> Option<&T> {
        self.adapters.get(&slot)?.1.downcast_ref()
    }

    /// List all registered adapter plugins.
    pub fn list_plugins(&self) -> Vec<(PluginSlot, &PluginDescriptor)> {
        let mut result: Vec<_> = self.adapters.iter().map(|(s, (d, _))| (*s, d)).collect();
        result.sort_by_key(|(s, _)| format!("{}", s));
        result
    }

    /// Remove a plugin from the given slot.
    pub fn unregister(&mut self, slot: PluginSlot) {
        if self.adapters.remove(&slot).is_some() {
            warn!("Unregistering {} plugin", slot);
        }
    }

    // -- Typed Convenience Accessors --

    pub fn runtime(&self) -> Option<&dyn RuntimeAdapter> {
        self.get_adapter::<Box<dyn RuntimeAdapter>>(PluginSlot::Runtime)
            .map(|b| b.as_ref())
    }

    pub fn workspace(&self) -> Option<&dyn WorkspaceAdapter> {
        self.get_adapter::<Box<dyn WorkspaceAdapter>>(PluginSlot::Workspace)
            .map(|b| b.as_ref())
    }

    pub fn scm(&self) -> Option<&dyn ScmAdapter> {
        self.get_adapter::<Box<dyn ScmAdapter>>(PluginSlot::Scm)
            .map(|b| b.as_ref())
    }

    pub fn tracker(&self) -> Option<&dyn TrackerAdapter> {
        self.get_adapter::<Box<dyn TrackerAdapter>>(PluginSlot::Tracker)
            .map(|b| b.as_ref())
    }

    pub fn notifier(&self) -> Option<&dyn NotifierAdapter> {
        self.get_adapter::<Box<dyn NotifierAdapter>>(PluginSlot::Notifier)
            .map(|b| b.as_ref())
    }

    pub fn terminal(&self) -> Option<&dyn TerminalAdapter> {
        self.get_adapter::<Box<dyn TerminalAdapter>>(PluginSlot::Terminal)
            .map(|b| b.as_ref())
    }

    pub fn agent_backend(&self) -> Option<&dyn AgentBackendAdapter> {
        self.get_adapter::<Box<dyn AgentBackendAdapter>>(PluginSlot::AgentBackend)
            .map(|b| b.as_ref())
    }

    /// Shutdown all extensions.
    pub async fn shutdown(&mut self) {
        for (_, plugin) in self.extensions.drain() {
            if let Err(e) = plugin.shutdown() {
                warn!("Failed to shutdown plugin {}: {}", plugin.name(), e);
            }
        }
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
