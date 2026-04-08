//! Setup and Init Command Implementations
//!
//! Covers two entry points:
//!   `d3vx init`  — scaffold .d3vx/ in a project
//!   `d3vx setup` — interactive global provider wizard

use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use crate::config::defaults::get_global_config_path;
use crate::config::onboarding::check_onboarding_status;
use crate::utils::project::{detect_project, generate_project_md};

use crate::cli::commands::helpers::{prompt_input, prompt_yes_no, provider_default_models};

// ─────────────────────────────────────────────────────────────────────────────
// Init
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) async fn execute_init(path: Option<&PathBuf>) -> Result<()> {
    let root = path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let d3vx_dir = root.join(".d3vx");

    if d3vx_dir.exists() {
        println!("  Warning: .d3vx/ already exists at {}", root.display());
        return Ok(());
    }

    println!("\n  Initializing d3vx in {}\n", root.display());

    let detected = detect_project(&root);

    fs::create_dir_all(d3vx_dir.join("memory"))?;
    fs::create_dir_all(d3vx_dir.join("sessions"))?;
    fs::create_dir_all(d3vx_dir.join("hooks"))?;

    write_project_config(&d3vx_dir)?;
    write_project_md(&d3vx_dir, &detected)?;
    write_gitignore_entry(&root)?;

    let label = if detected.framework != "Unknown" {
        format!("{} ({})", detected.framework, detected.language)
    } else {
        detected.language.clone()
    };

    println!("  \x1b[32m✔\x1b[0m .d3vx/ initialized — detected: {label}");
    if !detected.build_command.is_empty() {
        println!("      build: {}", detected.build_command);
    }
    if !detected.test_command.is_empty() {
        println!("      test:  {}", detected.test_command);
    }
    println!("\n  Edit .d3vx/project.md to describe your project context.");
    println!("  Run \x1b[1md3vx setup\x1b[0m to configure your LLM provider.\n");

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Setup wizard
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) async fn execute_setup(provider_arg: Option<&str>) -> Result<()> {
    use crate::providers::SUPPORTED_PROVIDERS;

    print_banner();

    // Detect first-run state and surface it early
    let status = check_onboarding_status();
    if status.is_first_run {
        println!("  \x1b[33m!\x1b[0m  First run detected — let's get you configured.\n");
    }

    let all_providers: Vec<_> = {
        // Sort for stable ordering: Anthropic first, then alphabetical
        let mut list: Vec<_> = SUPPORTED_PROVIDERS.all().collect();
        list.sort_by_key(|p| if p.id == "anthropic" { "\0" } else { p.id });
        list
    };

    let selected_id = select_provider(provider_arg, &all_providers)?;

    let provider_info = SUPPORTED_PROVIDERS
        .get(&selected_id)
        .ok_or_else(|| anyhow::anyhow!("Provider '{}' not found in registry", selected_id))?;

    println!("\n  \x1b[1mConfiguring: {}\x1b[0m", provider_info.name);
    println!("  Config path: {}\n", get_global_config_path());

    let (default_cheap, default_standard, default_premium) =
        provider_default_models(&selected_id);

    let standard_model = prompt_input("Standard model", Some(&default_standard))?;
    let routing_enabled = prompt_yes_no("Enable 3-tier model routing", true)?;

    let cheap_model = if routing_enabled {
        prompt_input("Cheap model  (research/fast tasks)", Some(&default_cheap))?
    } else {
        standard_model.clone()
    };
    let premium_model = if routing_enabled {
        prompt_input("Premium model (complex tasks)", Some(&default_premium))?
    } else {
        standard_model.clone()
    };

    // Custom base URL for proxies (OpenAI-compatible, LiteLLM, etc.)
    let base_url = prompt_base_url(provider_info.base_url)?;

    // Budget configuration for cost control
    let budget_enabled = prompt_yes_no("Enable budget enforcement (prevents runaway API costs)", true)?;
    let budget_per_session = if budget_enabled {
        let input = prompt_input("Per-session budget (USD)", Some("5.00"))?;
        input.parse::<f64>().unwrap_or(5.00)
    } else {
        0.0
    };
    let budget_per_day = if budget_enabled {
        let input = prompt_input("Per-day budget (USD)", Some("50.00"))?;
        input.parse::<f64>().unwrap_or(50.00)
    } else {
        0.0
    };

    let yaml = render_config_yaml(
        &selected_id,
        &standard_model,
        routing_enabled,
        &cheap_model,
        &premium_model,
        base_url.as_deref(),
        budget_enabled,
        budget_per_session,
        budget_per_day,
    )?;

    println!("\n  Planned config:\n\n{yaml}");

    if !prompt_yes_no("Write this to your global config", true)? {
        println!("\n  Setup cancelled.");
        return Ok(());
    }

    write_global_config(&yaml)?;
    print_api_key_instructions(provider_info);
    print_next_steps(provider_info);

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

fn print_banner() {
    println!("\n  \x1b[1md3vx setup\x1b[0m\n");
    println!("  {}\n", "─".repeat(46));
}

fn prompt_base_url(default_base_url: Option<&str>) -> Result<Option<String>> {
    let default = default_base_url.unwrap_or("");
    let input = prompt_input("Custom base URL (e.g. https://openai.api-proxy.com/v1)", if default.is_empty() { None } else { Some(default) })?;
    let trimmed = input.trim();
    Ok(if trimmed.is_empty() { None } else { Some(trimmed.to_string()) })
}

fn select_provider(arg: Option<&str>, all: &[&crate::providers::registry::ProviderInfo]) -> Result<String> {
    use crate::providers::SUPPORTED_PROVIDERS;

    if let Some(p) = arg {
        if !SUPPORTED_PROVIDERS.is_supported(p) {
            anyhow::bail!(
                "Unknown provider '{p}'. Supported: {}",
                all.iter().map(|x| x.id).collect::<Vec<_>>().join(", ")
            );
        }
        return Ok(p.to_string());
    }

    println!("  Select your LLM provider:\n");
    for (i, info) in all.iter().enumerate() {
        let marker = if info.id == "anthropic" { " \x1b[90m(default)\x1b[0m" } else { "" };
        let key_note = if info.requires_api_key {
            format!("needs {}", info.api_key_env)
        } else {
            "no key needed".to_string()
        };
        println!("    {}. {:<12} {:<20} {}{}", i + 1, info.id, info.name, key_note, marker);
    }
    println!("    0. Exit\n");

    let choice = prompt_input("Enter number or provider id", None)?;

    if choice == "0" || choice.is_empty() {
        anyhow::bail!("Setup cancelled.");
    }

    if let Ok(n) = choice.parse::<usize>() {
        if n == 0 || n > all.len() {
            anyhow::bail!("Invalid selection '{n}'");
        }
        return Ok(all[n - 1].id.to_string());
    }

    if SUPPORTED_PROVIDERS.is_supported(&choice) {
        return Ok(choice);
    }

    anyhow::bail!("Unknown provider '{choice}'");
}

fn render_config_yaml(
    provider: &str,
    standard_model: &str,
    routing_enabled: bool,
    cheap_model: &str,
    premium_model: &str,
    base_url: Option<&str>,
    budget_enabled: bool,
    budget_per_session: f64,
    budget_per_day: f64,
) -> Result<String> {
    let mut root = serde_yaml::Mapping::new();
    let sv = |s: &str| serde_yaml::Value::String(s.to_string());
    let bv = |b: bool| serde_yaml::Value::Bool(b);

    root.insert(sv("provider"), sv(provider));
    root.insert(sv("model"), sv(standard_model));

    // Provider-specific config with optional base_url
    if let Some(url) = base_url {
        if !url.is_empty() {
            let mut provider_cfg = serde_yaml::Mapping::new();
            provider_cfg.insert(sv("base_url"), sv(url));
            root.insert(
                sv("providers"),
                serde_yaml::Value::Mapping({
                    let mut providers = serde_yaml::Mapping::new();
                    let mut configs = serde_yaml::Mapping::new();
                    configs.insert(sv(provider), serde_yaml::Value::Mapping(provider_cfg));
                    providers.insert(sv("configs"), serde_yaml::Value::Mapping(configs));
                    providers
                }),
            );
        }
    }

    if routing_enabled {
        let mut routing = serde_yaml::Mapping::new();
        routing.insert(sv("enabled"), bv(true));
        routing.insert(sv("complexity_routing"), bv(true));
        routing.insert(sv("cheap_model"), sv(cheap_model));
        routing.insert(sv("standard_model"), sv(standard_model));
        routing.insert(sv("premium_model"), sv(premium_model));
        root.insert(sv("model_routing"), serde_yaml::Value::Mapping(routing));
    }

    // Budget configuration
    if budget_enabled {
        let mut budget = serde_yaml::Mapping::new();
        budget.insert(sv("enabled"), bv(true));
        budget.insert(sv("per_session"), sv(&format!("{:.2}", budget_per_session)));
        budget.insert(sv("per_day"), sv(&format!("{:.2}", budget_per_day)));
        budget.insert(sv("warn_at"), sv("0.8"));
        budget.insert(sv("pause_at"), sv("1.0"));
        root.insert(sv("budget"), serde_yaml::Value::Mapping(budget));
    }

    serde_yaml::to_string(&serde_yaml::Value::Mapping(root))
        .context("Failed to serialise configuration")
}

fn write_global_config(yaml: &str) -> Result<()> {
    let path = PathBuf::from(get_global_config_path());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, yaml)
        .with_context(|| format!("Failed to write config to {}", path.display()))?;
    println!("  \x1b[32m✔\x1b[0m Wrote {}", path.display());
    Ok(())
}

fn print_api_key_instructions(info: &crate::providers::registry::ProviderInfo) {
    if info.id == "ollama" {
        println!("\n  Ollama next steps:");
        println!("    1. Install Ollama: https://ollama.ai");
        println!("    2. Pull a model:   ollama pull {}", info.default_model);
        println!("    3. Start server:   ollama serve");
        return;
    }

    println!("\n  API key setup:");

    let url = match info.id {
        "anthropic"  => "https://console.anthropic.com/settings/keys",
        "openai"     => "https://platform.openai.com/api-keys",
        "groq"       => "https://console.groq.com/keys",
        "openrouter" => "https://openrouter.ai/keys",
        "xai"        => "https://console.x.ai",
        "mistral"    => "https://console.mistral.ai/api-keys",
        "deepseek"   => "https://platform.deepseek.com/api_keys",
        _            => "provider's dashboard",
    };

    println!("    1. Get your key: {url}");
    if !info.api_key_env.is_empty() {
        println!("    2. Add to your shell profile (~/.zshrc or ~/.bashrc):");
        println!("         export {}=\"your-key-here\"", info.api_key_env);
        println!("    3. Reload:  source ~/.zshrc");
    }
}

fn print_next_steps(info: &crate::providers::registry::ProviderInfo) {
    println!("\n  {}", "─".repeat(46));
    println!("  \x1b[1mYou're almost ready.\x1b[0m Run these to verify:\n");
    println!("    d3vx doctor");
    println!("    d3vx \"add input validation to the login form\" --vex\n");

    if info.id != "ollama" && !info.api_key_env.is_empty() {
        println!(
            "  \x1b[90mTip: set {} before running d3vx doctor\x1b[0m\n",
            info.api_key_env
        );
    }
}

fn write_project_config(d3vx_dir: &PathBuf) -> Result<()> {
    let config_path = d3vx_dir.join("config.yml");
    let mut file = File::create(&config_path)?;
    file.write_all(
        b"# d3vx project configuration\n\
          # Global config at ~/.d3vx/config.yml takes precedence for provider/model.\n\
          # Use this file for project-specific overrides.\n\
          \n\
          # Uncomment to override provider for this project:\n\
          # provider: openai\n\
          # model: gpt-4o\n\
          \n\
          permissions:\n\
            allow: []\n\
            deny:\n\
              - \"BashTool(cmd:sudo *)\"\n\
              - \"BashTool(cmd:rm -rf /)\"\n",
    )?;
    Ok(())
}

fn write_project_md(d3vx_dir: &PathBuf, detected: &crate::utils::project::DetectedProject) -> Result<()> {
    let content = generate_project_md(detected);
    fs::write(d3vx_dir.join("project.md"), content)?;
    Ok(())
}

fn write_gitignore_entry(root: &PathBuf) -> Result<()> {
    if !root.join(".git").exists() {
        return Ok(());
    }
    let gi_path = root.join(".gitignore");
    let existing = if gi_path.exists() {
        fs::read_to_string(&gi_path)?
    } else {
        String::new()
    };

    if existing.contains(".d3vx-worktrees") {
        return Ok(());
    }

    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str("\n# d3vx worktrees\n.d3vx-worktrees/\n");
    fs::write(&gi_path, content)?;
    Ok(())
}
