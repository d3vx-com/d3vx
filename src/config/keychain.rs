//! OS keychain integration for secure API key storage.
//!
//! Stores provider API keys in the platform-native credential store:
//! - macOS: Keychain Access
//! - Linux: Secret Service (libsecret)
//! - Windows: Credential Manager
//!
//! Service name: "d3vx"
//! Account per key: "{provider}_api_key" (e.g. "anthropic_api_key")

use tracing::{debug, warn};

/// Service name used for all d3vx entries in the OS keychain.
const SERVICE: &str = "d3vx";

/// Store an API key in the OS keychain.
pub fn store_key(provider: &str, key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, &format!("{provider}_api_key"))
        .map_err(|e| format!("Failed to create keychain entry: {e}"))?;
    entry
        .set_password(key)
        .map_err(|e| format!("Failed to store API key: {e}"))?;
    debug!("Stored API key for {} in OS keychain", provider);
    Ok(())
}

/// Retrieve an API key from the OS keychain.
///
/// Returns `None` if no key is stored (first-time use or key was deleted).
pub fn get_key(provider: &str) -> Option<String> {
    let entry = match keyring::Entry::new(SERVICE, &format!("{provider}_api_key")) {
        Ok(e) => e,
        Err(e) => {
            warn!("Keychain entry creation failed for {provider}: {e}");
            return None;
        }
    };
    match entry.get_password() {
        Ok(key) => {
            debug!("Retrieved API key for {} from OS keychain", provider);
            Some(key)
        }
        Err(keyring::Error::NoEntry) => {
            debug!("No keychain entry for {}", provider);
            None
        }
        Err(e) => {
            warn!("Keychain read failed for {provider}: {e}");
            None
        }
    }
}

/// Delete an API key from the OS keychain.
pub fn delete_key(provider: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, &format!("{provider}_api_key"))
        .map_err(|e| format!("Failed to create keychain entry: {e}"))?;
    entry
        .delete_credential()
        .map_err(|e| format!("Failed to delete API key: {e}"))?;
    debug!("Deleted API key for {} from OS keychain", provider);
    Ok(())
}

/// Check whether a key is stored for the given provider.
pub fn has_key(provider: &str) -> bool {
    get_key(provider).is_some()
}
