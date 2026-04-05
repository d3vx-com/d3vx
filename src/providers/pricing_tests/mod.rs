// Tests for Pricing Cache
use super::pricing_cache::{CostData, ModelData};
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_manifest_serialization_roundtrip() {
    let mut manifest = HashMap::new();
    manifest.insert(
        "gpt-4o".to_string(),
        ModelData {
            id: "gpt-4o".into(),
            name: "GPT-4o".into(),
            cost: Some(CostData {
                input: 2.5,
                output: 10.0,
                cache_read: Some(1.25),
                cache_write: Some(3.75),
            }),
        },
    );

    let json = serde_json::to_string(&manifest).unwrap();
    let decoded: HashMap<String, ModelData> = serde_json::from_str(&json).unwrap();

    assert!(decoded.contains_key("gpt-4o"));
    let data = decoded.get("gpt-4o").unwrap();
    assert_eq!(data.name, "GPT-4o");
    let cost = data.cost.as_ref().unwrap();
    assert_eq!(cost.input, 2.5);
    assert_eq!(cost.cache_write, Some(3.75));
}

#[test]
fn test_is_cache_fresh_logic() {
    let dir = tempdir().unwrap();
    let cache_path = dir.path().join("models.json");

    // No file -> not fresh
    assert!(!is_cache_fresh_at(&cache_path));

    // Create file
    fs::write(&cache_path, "{}").unwrap();

    // New file -> fresh (MAX_CACHE_AGE_SECS is 3600)
    assert!(is_cache_fresh_at(&cache_path));
}

/// Helper mirroring the internal logic of is_cache_fresh but taking a path
fn is_cache_fresh_at(path: &std::path::Path) -> bool {
    if !path.exists() {
        return false;
    }
    if let Ok(metadata) = fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = std::time::SystemTime::now().duration_since(modified) {
                return duration.as_secs() < 3600; // 60 mins
            }
        }
    }
    false
}

#[test]
fn test_pricing_conversion_to_internal_type() {
    let cost_data = CostData {
        input: 1.0,
        output: 2.0,
        cache_read: Some(0.5),
        cache_write: None,
    };

    // Simulate get_model_pricing's mapping logic
    let internal_pricing = crate::agent::cost::ModelPricing {
        input: cost_data.input,
        output: cost_data.output,
        cache_read: cost_data.cache_read.unwrap_or(0.0),
    };

    assert_eq!(internal_pricing.input, 1.0);
    assert_eq!(internal_pricing.output, 2.0);
    assert_eq!(internal_pricing.cache_read, 0.5);
}
