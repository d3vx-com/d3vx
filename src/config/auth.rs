//! File-based API key storage.
//!
//! Stores provider API keys in `~/.d3vx/auth.json` with 0o600 permissions.
//! This follows the same pattern as opencode and other CLI tools —
//! simple, portable, no platform-specific keychain dependencies.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, warn};

/// Credential entry stored per provider.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Credential {
    /// The API key.
    pub key: String,
}

/// Path to the auth file: `~/.d3vx/auth.json`
fn auth_file_path() -> PathBuf {
    PathBuf::from(crate::config::defaults::get_global_config_dir()).join("auth.json")
}

/// Read all stored credentials from disk.
/// Returns an empty map if the file doesn't exist or is invalid.
fn read_all() -> HashMap<String, Credential> {
    let path = auth_file_path();
    if !path.exists() {
        return HashMap::new();
    }
    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(map) => map,
            Err(e) => {
                warn!("Failed to parse auth.json: {}", e);
                HashMap::new()
            }
        },
        Err(e) => {
            warn!("Failed to read auth.json: {}", e);
            HashMap::new()
        }
    }
}

/// Write all credentials to disk with restricted permissions.
fn write_all(data: &HashMap<String, Credential>) -> Result<(), String> {
    let path = auth_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Failed to serialize credentials: {e}"))?;

    // Write with 0o600 (owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(json.as_bytes())
            })
            .map_err(|e| format!("Failed to write auth.json: {e}"))?;
    }
    #[cfg(not(unix))]
    {
        fs::write(&path, &json).map_err(|e| format!("Failed to write auth.json: {e}"))?;
    }

    debug!("Wrote {} credentials to auth.json", data.len());
    Ok(())
}

/// Store an API key for a provider.
pub fn store_key(provider: &str, key: &str) -> Result<(), String> {
    let mut data = read_all();
    data.insert(
        provider.to_string(),
        Credential {
            key: key.to_string(),
        },
    );
    write_all(&data)?;
    debug!("Stored API key for {}", provider);
    Ok(())
}

/// Retrieve an API key for a provider.
///
/// Returns `None` if no key is stored.
pub fn get_key(provider: &str) -> Option<String> {
    let data = read_all();
    data.get(provider).map(|c| {
        debug!("Retrieved API key for {} from auth.json", provider);
        c.key.clone()
    })
}

/// Delete an API key for a provider.
pub fn delete_key(provider: &str) -> Result<(), String> {
    let mut data = read_all();
    if data.remove(provider).is_none() {
        return Err(format!("No key stored for {}", provider));
    }
    write_all(&data)?;
    debug!("Deleted API key for {}", provider);
    Ok(())
}

/// Check whether a key is stored for the given provider.
pub fn has_key(provider: &str) -> bool {
    get_key(provider).is_some()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    // All auth tests share ~/.d3vx/auth.json, so serialize them
    // to avoid parallel write races.
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap()
    }

    fn unique_provider(name: &str) -> String {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("__test_{name}_{id}")
    }

    #[test]
    fn test_store_and_retrieve() {
        let _guard = lock();
        let provider = unique_provider("store");
        assert!(store_key(&provider, "sk-test-123").is_ok());
        assert_eq!(get_key(&provider), Some("sk-test-123".to_string()));
        assert!(has_key(&provider));
        let _ = delete_key(&provider);
        assert!(!has_key(&provider));
    }

    #[test]
    fn test_missing_key_returns_none() {
        let provider = unique_provider("missing");
        assert_eq!(get_key(&provider), None);
        assert!(!has_key(&provider));
    }

    #[test]
    fn test_delete_removes_key() {
        let _guard = lock();
        let provider = unique_provider("delete");
        assert!(store_key(&provider, "sk-delete-me").is_ok());
        assert_eq!(get_key(&provider), Some("sk-delete-me".to_string()));
        assert!(delete_key(&provider).is_ok());
        assert_eq!(get_key(&provider), None);
    }

    #[test]
    fn test_overwrite_updates_key() {
        let _guard = lock();
        let provider = unique_provider("overwrite");
        assert!(store_key(&provider, "old-key").is_ok());
        assert!(store_key(&provider, "new-key").is_ok());
        assert_eq!(get_key(&provider), Some("new-key".to_string()));
        let _ = delete_key(&provider);
    }

    #[test]
    fn test_multiple_providers() {
        let _guard = lock();
        let p1 = unique_provider("multi_a");
        let p2 = unique_provider("multi_b");

        assert!(store_key(&p1, "key-a").is_ok());
        assert!(store_key(&p2, "key-b").is_ok());
        assert_eq!(get_key(&p1), Some("key-a".to_string()));
        assert_eq!(get_key(&p2), Some("key-b".to_string()));

        let _ = delete_key(&p1);
        let _ = delete_key(&p2);
    }
}
