//! Setup and Init Command Implementations
//!
//! Project initialization, interactive setup wizard, and status display.

use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use crate::config::defaults::get_global_config_path;
use crate::utils::project::{detect_project, generate_project_md};

use crate::cli::commands::helpers::{prompt_input, prompt_yes_no, provider_default_models};

pub(crate) async fn execute_init(path: Option<&PathBuf>) -> Result<()> {
    let project_root = path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let d3vx_dir = project_root.join(".d3vx");

    if d3vx_dir.exists() {
        println!("Warning: .d3vx already exists.");
        return Ok(());
    }

    println!("Initializing d3vx...\n");

    // Detect project info
    let detected = detect_project(&project_root);

    // Create directories
    fs::create_dir_all(d3vx_dir.join("memory"))?;
    fs::create_dir_all(d3vx_dir.join("sessions"))?;
    fs::create_dir_all(d3vx_dir.join("hooks"))?;

    // Create config.yml
    let config_path = d3vx_dir.join("config.yml");
    let mut config_file = File::create(config_path)?;
    config_file.write_all(
        b"# d3vx Configuration
# See docs for full options

provider: anthropic
model: claude-sonnet-4-20250514

# Permission patterns
permissions:
  allow: []
  deny:
    - \"BashTool(cmd:sudo *)\"
    - \"BashTool(cmd:rm -rf /)\"

# Git Integration & Pre-Commit Hooks
git:
  auto_commit: true
  auto_push: false
  pre_commit_hooks:
    format: true
    clippy: true
    test: true
    security: true
    skip_if_wip: true
    timeout_seconds: 60
",
    )?;

    // Create project.md with detected info
    let project_md_content = generate_project_md(&detected);
    let project_md_path = d3vx_dir.join("project.md");
    fs::write(project_md_path, project_md_content)?;

    // Create todo.md
    let todo_path = d3vx_dir.join("todo.md");
    let mut todo_file = File::create(todo_path)?;
    todo_file.write_all(b"# Todo\n\n")?;

    // Update .gitignore
    if project_root.join(".git").exists() {
        let gitignore_path = project_root.join(".gitignore");
        let mut gitignore_content = if gitignore_path.exists() {
            fs::read_to_string(&gitignore_path)?
        } else {
            String::new()
        };

        if !gitignore_content.contains(".d3vx-worktrees") {
            if !gitignore_content.is_empty() && !gitignore_content.ends_with('\n') {
                gitignore_content.push('\n');
            }
            gitignore_content.push_str("\n# d3vx worktrees\n.d3vx-worktrees/\n");
            fs::write(&gitignore_path, gitignore_content)?;
            println!("  Added .d3vx-worktrees/ to .gitignore");
        }
    }

    println!("  Created .d3vx/config.yml");
    println!("  Created .d3vx/project.md");
    println!("  Created .d3vx/memory/");
    println!("  Created .d3vx/sessions/");

    let detected_label = if detected.framework != "Unknown" {
        format!("{} ({})", detected.framework, detected.language)
    } else {
        detected.language
    };

    println!("\nd3vx initialized! Detected: {}", detected_label);
    if !detected.build_command.is_empty() {
        println!("  Build: {}", detected.build_command);
    }
    if !detected.test_command.is_empty() {
        println!("  Test: {}", detected.test_command);
    }
    println!("Edit .d3vx/project.md to describe your project.\n");
    Ok(())
}

pub(crate) async fn execute_setup(provider: Option<&str>) -> Result<()> {
    use crate::providers::SUPPORTED_PROVIDERS;

    println!("\n  d3vx Interactive Setup\n");
    println!("{}", "=".repeat(50));

    let all_providers: Vec<_> = SUPPORTED_PROVIDERS.all().collect();

    // Select provider
    let selected_provider = if let Some(p) = provider {
        if !SUPPORTED_PROVIDERS.is_supported(p) {
            anyhow::bail!(
                "Unknown provider '{}'. Run `d3vx doctor` to see supported providers.",
                p
            );
        }
        p.to_string()
    } else {
        println!("\nSelect your LLM provider:\n");
        for (i, provider_info) in all_providers.iter().enumerate() {
            let marker = if provider_info.id == "anthropic" {
                " (default)"
            } else {
                ""
            };
            println!(
                "  {}. {:<12} - {}{}",
                i + 1,
                provider_info.id,
                provider_info.name,
                marker
            );
        }
        println!("\n  0. Exit setup");
        println!("\nEnter number or provider name: ");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input == "0" {
            println!("\nSetup cancelled.");
            return Ok(());
        }

        // Try to parse as number
        if let Ok(num) = input.parse::<usize>() {
            if num == 0 || num > all_providers.len() {
                anyhow::bail!("Invalid selection");
            }
            all_providers[num - 1].id.to_string()
        } else {
            // Try as provider name
            if !SUPPORTED_PROVIDERS.is_supported(input) {
                anyhow::bail!(
                    "Unknown provider '{}'. Run `d3vx doctor` to see supported providers.",
                    input
                );
            }
            input.to_string()
        }
    };

    let provider_info = SUPPORTED_PROVIDERS
        .get(&selected_provider)
        .ok_or_else(|| anyhow::anyhow!("Provider not found"))?;

    println!("\n{:=<50}", "");
    println!("\n  Configuring: {}", provider_info.name);
    println!("\n{:=<50}", "");

    let (default_cheap, default_standard, default_premium) =
        provider_default_models(&selected_provider);

    println!("\nProvider config target: {}", get_global_config_path());
    println!(
        "This global file applies across all your projects unless a repo overrides it with .d3vx/config.yml.\n"
    );

    let standard_model = prompt_input("Standard model", Some(&default_standard))?;
    let routing_enabled = prompt_yes_no("Enable 3-tier model routing?", true)?;
    let cheap_model = if routing_enabled {
        prompt_input("Cheap model", Some(&default_cheap))?
    } else {
        standard_model.clone()
    };
    let premium_model = if routing_enabled {
        prompt_input("Premium model", Some(&default_premium))?
    } else {
        standard_model.clone()
    };

    let yaml = render_setup_config_yaml(
        &selected_provider,
        &standard_model,
        routing_enabled,
        &cheap_model,
        &premium_model,
    )?;

    println!("\nPlanned config:\n");
    println!("{}", yaml);

    if !prompt_yes_no("Write this to your global config?", true)? {
        println!("\nSetup cancelled.");
        return Ok(());
    }

    let config_path = PathBuf::from(get_global_config_path());
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&config_path, yaml)?;
    println!("\nWrote {}", config_path.display());

    if selected_provider == "ollama" {
        println!("\nOllama next steps:");
        println!("  1. Install Ollama");
        println!("  2. Pull your chosen models");
        println!("  3. Start the server with `ollama serve`");
        println!("  4. Run `d3vx doctor`");
        return Ok(());
    }

    println!("\nAPI key setup:");
    match selected_provider.as_str() {
        "anthropic" => println!("  1. Visit: https://console.anthropic.com/settings/keys"),
        "openai" => println!("  1. Visit: https://platform.openai.com/api-keys"),
        "groq" => println!("  1. Visit: https://console.groq.com/keys"),
        "openrouter" => println!("  1. Visit: https://openrouter.ai/keys"),
        _ => println!("  1. Visit the provider's site and generate an API key"),
    }
    println!("  2. Add this to your shell profile:");
    if provider_info.api_key_env.is_empty() {
        println!("     # No API key required for {}", provider_info.name);
    } else {
        println!(
            "     export {}=\"your-api-key-here\"",
            provider_info.api_key_env
        );
    }
    println!("  3. Restart your shell or run: source ~/.zshrc");
    println!("  4. Verify with: d3vx doctor");

    Ok(())
}

fn render_setup_config_yaml(
    provider: &str,
    standard_model: &str,
    routing_enabled: bool,
    cheap_model: &str,
    premium_model: &str,
) -> Result<String> {
    let mut root = serde_yaml::Mapping::new();
    root.insert(
        serde_yaml::Value::String("provider".to_string()),
        serde_yaml::Value::String(provider.to_string()),
    );
    root.insert(
        serde_yaml::Value::String("model".to_string()),
        serde_yaml::Value::String(standard_model.to_string()),
    );

    if routing_enabled {
        let mut routing = serde_yaml::Mapping::new();
        routing.insert(
            serde_yaml::Value::String("enabled".to_string()),
            serde_yaml::Value::Bool(true),
        );
        routing.insert(
            serde_yaml::Value::String("complexity_routing".to_string()),
            serde_yaml::Value::Bool(true),
        );
        routing.insert(
            serde_yaml::Value::String("cheap_model".to_string()),
            serde_yaml::Value::String(cheap_model.to_string()),
        );
        routing.insert(
            serde_yaml::Value::String("standard_model".to_string()),
            serde_yaml::Value::String(standard_model.to_string()),
        );
        routing.insert(
            serde_yaml::Value::String("premium_model".to_string()),
            serde_yaml::Value::String(premium_model.to_string()),
        );
        root.insert(
            serde_yaml::Value::String("model_routing".to_string()),
            serde_yaml::Value::Mapping(routing),
        );
    }

    serde_yaml::to_string(&serde_yaml::Value::Mapping(root))
        .context("Failed to render setup configuration")
}
