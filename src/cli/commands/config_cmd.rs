//! Config and Pricing Command Implementations
//!
//! Reading/writing project config and model pricing management.

use anyhow::Result;
use std::fs;

pub(crate) async fn execute_config(
    action: &str,
    key: Option<&str>,
    value: Option<&str>,
) -> Result<()> {
    let config_path = std::env::current_dir()?.join(".d3vx").join("config.yml");

    match action {
        "list" => {
            if !config_path.exists() {
                println!("No .d3vx/config.yml found. Run `d3vx init` first.");
                return Ok(());
            }
            let content = fs::read_to_string(&config_path)?;
            println!("\nConfiguration:\n");
            println!("{}", content);
        }
        "get" => {
            let k = key.ok_or_else(|| anyhow::anyhow!("Key required for 'get' action"))?;
            if !config_path.exists() {
                println!("No .d3vx/config.yml found.");
                return Ok(());
            }
            let content = fs::read_to_string(&config_path)?;

            // Simple regex match for top-level keys
            let pattern = format!(r"(?m)^{}:\s*(.+)$", regex::escape(k));
            let re = regex::Regex::new(&pattern)?;

            if let Some(caps) = re.captures(&content) {
                println!("{}", caps[1].trim());
            } else {
                println!("Key \"{}\" not found.", k);
            }
        }
        "set" => {
            let k = key.ok_or_else(|| anyhow::anyhow!("Key required for 'set' action"))?;
            let v = value.ok_or_else(|| anyhow::anyhow!("Value required for 'set' action"))?;

            if !config_path.exists() {
                println!("No .d3vx/config.yml found. Run `d3vx init` first.");
                return Ok(());
            }

            let mut content = fs::read_to_string(&config_path)?;
            let pattern = format!(r"(?m)^({}:\s*)(.+)$", regex::escape(k));
            let re = regex::Regex::new(&pattern)?;

            if re.is_match(&content) {
                content = re.replace(&content, format!("${{1}}{}", v)).to_string();
            } else {
                if !content.ends_with('\n') && !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(&format!("{}: {}\n", k, v));
            }

            fs::write(&config_path, content)?;
            println!("Set {} = {}", k, v);
        }
        "delete" => {
            let k = key.ok_or_else(|| anyhow::anyhow!("Key required for 'delete' action"))?;

            if !config_path.exists() {
                println!("No .d3vx/config.yml found.");
                return Ok(());
            }

            let content = fs::read_to_string(&config_path)?;
            let pattern = format!(r"(?m)^{}:\s*.*\n?", regex::escape(k));
            let re = regex::Regex::new(&pattern)?;

            let new_content = re.replace_all(&content, "").to_string();
            fs::write(&config_path, new_content)?;
            println!("Deleted key \"{}\"", k);
        }
        _ => {
            anyhow::bail!(
                "Unknown config action: {}. Use: get, set, list, delete",
                action
            );
        }
    }

    Ok(())
}

pub(crate) async fn execute_pricing(action: &str, model: Option<&str>) -> Result<()> {
    match action {
        "list" => {
            println!("Loading model pricing from cache...");
            if let Some(manifest) = crate::providers::pricing_cache::load_manifest() {
                println!(
                    "\n{:<40} | {:<10} | {:<10} | {:<10}",
                    "Model ID", "Input ($)", "Output ($)", "Cache Read ($)"
                );
                println!("{:-<40}-+-{:-<10}-+-{:-<10}-+-{:-<10}", "", "", "", "");
                for (id, data) in manifest.iter().take(50) {
                    // Limit to 50 to avoid blasting terminal
                    if let Some(cost) = &data.cost {
                        println!(
                            "{:<40} | {:<10.2} | {:<10.2} | {:<10.2}",
                            id,
                            cost.input,
                            cost.output,
                            cost.cache_read.unwrap_or(0.0)
                        );
                    }
                }
                if manifest.len() > 50 {
                    println!("... and {} more models.", manifest.len() - 50);
                }
            } else {
                println!("No pricing cache found. Run `d3vx pricing refresh` first.");
            }
        }
        "refresh" => {
            println!("Refreshing pricing data from models.dev...");
            crate::providers::pricing_cache::fetch_and_cache_pricing().await?;
            println!("Pricing cache successfully updated.");
        }
        "get" => {
            if let Some(m) = model {
                if let Some(pricing) = crate::providers::pricing_cache::get_model_pricing(m) {
                    println!("Pricing for '{}':", m);
                    println!("  Input (per 1M): ${:.2}", pricing.input);
                    println!("  Output (per 1M): ${:.2}", pricing.output);
                    println!("  Cache Read (per 1M): ${:.2}", pricing.cache_read);
                } else {
                    println!("No pricing found for model '{}' in cache.", m);
                    println!("Run `d3vx pricing refresh` to ensure cache is up to date, or check the ID.");
                }
            } else {
                anyhow::bail!("Model required for 'get' action");
            }
        }
        _ => {
            anyhow::bail!(
                "Unknown pricing action: {}. Use: list, refresh, get",
                action
            );
        }
    }

    Ok(())
}
