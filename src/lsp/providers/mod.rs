//! Built-in LSP Server Configurations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    pub command: Vec<String>,
    pub args: Vec<String>,
    pub extensions: Vec<String>,
    pub initialization_options: Option<serde_json::Value>,
}

impl LspServerConfig {
    pub fn new(command: Vec<String>, extensions: Vec<String>) -> Self {
        Self {
            command,
            args: vec![],
            extensions,
            initialization_options: None,
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_init_options(mut self, opts: serde_json::Value) -> Self {
        self.initialization_options = Some(opts);
        self
    }
}

/// Built-in LSP server configurations
pub struct BuiltInServers;

impl BuiltInServers {
    /// Get all built-in server configurations
    pub fn all() -> HashMap<String, LspServerConfig> {
        let mut servers = HashMap::new();
        
        servers.insert("typescript".to_string(), Self::typescript());
        servers.insert("rust".to_string(), Self::rust_analyzer());
        servers.insert("python".to_string(), Self::python());
        servers.insert("go".to_string(), Self::go());
        servers.insert("java".to_string(), Self::java());
        servers.insert("csharp".to_string(), Self::csharp());
        servers.insert("cpp".to_string(), Self::cpp());
        servers.insert("clangd".to_string(), Self::clangd());

        servers
    }

    /// TypeScript/JavaScript
    pub fn typescript() -> LspServerConfig {
        LspServerConfig::new(
            vec!["typescript-language-server".to_string(), "--stdio".to_string()],
            vec![".ts".to_string(), ".tsx".to_string(), ".js".to_string(), ".jsx".to_string(), ".json".to_string()],
        )
    }

    /// Rust Analyzer
    pub fn rust_analyzer() -> LspServerConfig {
        LspServerConfig::new(
            vec!["rust-analyzer".to_string()],
            vec![".rs".to_string()],
        )
    }

    /// Python (Pyright)
    pub fn python() -> LspServerConfig {
        LspServerConfig::new(
            vec!["pyright-langserver".to_string(), "--stdio".to_string()],
            vec![".py".to_string()],
        )
    }

    /// Go (gopls)
    pub fn go() -> LspServerConfig {
        LspServerConfig::new(
            vec!["gopls".to_string()],
            vec![".go".to_string()],
        )
    }

    /// Java (jdtls)
    pub fn java() -> LspServerConfig {
        LspServerConfig::new(
            vec!["jdtls".to_string()],
            vec![".java".to_string()],
        )
    }

    /// C# (OmniSharp)
    pub fn csharp() -> LspServerConfig {
        LspServerConfig::new(
            vec!["omnisharp".to_string(), "--languageserver".to_string()],
            vec![".cs".to_string()],
        )
    }

    /// C/C++ (clangd)
    pub fn cpp() -> LspServerConfig {
        LspServerConfig::new(
            vec!["clangd".to_string()],
            vec![".c".to_string(), ".cpp".to_string(), ".cc".to_string(), ".h".to_string(), ".hpp".to_string()],
        )
    }

    /// Clangd (alternative for C/C++)
    pub fn clangd() -> LspServerConfig {
        Self::cpp()
    }

    /// Get server for a file extension
    pub fn for_extension(ext: &str) -> Option<String> {
        let servers = Self::all();
        for (name, config) in servers {
            if config.extensions.iter().any(|e| e == ext) {
                return Some(name);
            }
        }
        None
    }

    /// Get server for a file path
    pub fn for_file(path: &str) -> Option<String> {
        let path = std::path::Path::new(path);
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e))?;
        
        Self::for_extension(&ext)
    }
}
