//! One-shot and Interactive Command Implementations
//!
//! Launch modes: single-query processing and full interactive TUI session.

use anyhow::Result;
use std::io::Write;
use std::path::PathBuf;

use crate::config::{
    defaults::default_config, get_provider_config, load_config,
    onboarding::check_onboarding_status, LoadConfigOptions,
};
use crate::ui::runner::{run_tui, TuiOptions};

use crate::cli::args::Cli;

fn cwd_string(cwd: &Option<PathBuf>) -> Option<String> {
    cwd.as_ref().map(|p| p.to_string_lossy().to_string())
}

/// Detect first run and optionally guide user through quick setup.
/// Returns `true` if the user should still proceed to TUI (setup succeeded was skipped,
/// and the user chose to skip).
fn handle_first_run_if_needed() -> bool {
    let status = check_onboarding_status();
    if !status.is_first_run {
        return true;
    }

    // Show first-run greeting
    println!();
    println!("  \x1b[1mWelcome to d3vx! \x1b[0mThe autonomous software engineering CLI.\n");
    println!("  {}", "─".repeat(50));
    println!("  \x1b[90mFirst-time setup required. Choose an option:\x1b[0m\n");
    println!("    1. \x1b[1mQuick setup (recommended)\x1b[0m — interactive provider + model selection");
    println!("    2. \x1b[1mSkip — configure later via \x1b[33m`d3vx setup`\x1b[0m\n");
    println!("  \x1b[90mNo config found at {}\x1b[0m\n", crate::config::defaults::get_global_config_path());
    println!("  Run quick setup? [Y/n]: ");
    let _ = std::io::stdout().flush();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return true;
    }

    match input.trim().to_lowercase().as_str() {
        "n" | "no" => {
            show_skip_hint();
            true
        }
        _ => {
            run_quick_setup();
            false
        }
    }
}

fn show_skip_hint() {
    println!("\n  \x1b[90mSkipping setup. You'll need to configure d3vx before use:\x1b[0m");
    println!("    d3vx setup");
    println!("    d3vx doctor\x1b[90m          — check your environment\x1b[0m");
    println!();
}

/// Run a streamlined, synchronous setup wizard for first-time users.
fn run_quick_setup() {
    use crate::cli::commands::helpers::provider_default_models;
    use crate::providers::SUPPORTED_PROVIDERS;

    print!("\n  \x1b[1md3vx setup\x1b[0m\n  {}\n\n", "─".repeat(46));

    if check_onboarding_status().is_first_run {
        println!("  \x1b[33m!\x1b[0m First run detected — let's get you configured.\n");
    }

    let all_providers: Vec<_> = {
        let mut list: Vec<_> = SUPPORTED_PROVIDERS.all().collect();
        list.sort_by_key(|p| if p.id == "anthropic" { "\0" } else { p.id });
        list
    };

    println!("  Select your LLM provider:\n");
    for (i, info) in all_providers.iter().enumerate() {
        let key_note = if info.requires_api_key {
            format!("\x1b[90mneeds {}\x1b[0m", info.api_key_env)
        } else {
            "\x1b[90mno key needed\x1b[0m".to_string()
        };
        let marker = if info.id == "anthropic" { " \x1b[90m(default)\x1b[0m" } else { "" };
        println!("    {}. {:<12} {:<20} {}{}", i + 1, info.id, info.name, key_note, marker);
    }
    println!("    0. Exit\n");

    print!("  Enter number or provider id: ");
    let _ = std::io::stdout().flush();
    let mut choice = String::new();
    if std::io::stdin().read_line(&mut choice).is_err() {
        println!("\n  \x1b[31mSetup cancelled.\x1b[0m");
        return;
    }
    let choice = choice.trim();
    if choice == "0" || choice.is_empty() {
        println!("\n  Setup cancelled.");
        return;
    }

    let selected_id = if let Ok(n) = choice.parse::<usize>() {
        if n == 0 || n > all_providers.len() {
            println!("\n  \x1b[31mInvalid selection '{}'\x1b[0m", n);
            return;
        }
        all_providers[n - 1].id.to_string()
    } else if SUPPORTED_PROVIDERS.is_supported(choice) {
        choice.to_string()
    } else {
        println!("\n  \x1b[31mUnknown provider '{}'\x1b[0m", choice);
        return;
    };

    let provider_info = SUPPORTED_PROVIDERS
        .get(&selected_id)
        .expect("valid provider");

    println!("\n  \x1b[1mConfiguring: {}\x1b[0m", provider_info.name);
    println!(
        "  Config will be written to: {}\n",
        crate::config::defaults::get_global_config_path()
    );

    let (default_cheap, default_standard, default_premium) =
        provider_default_models(&selected_id);

    print!("  Standard model [{}]: ", default_standard);
    let _ = std::io::stdout().flush();
    let mut model_input = String::new();
    std::io::stdin().read_line(&mut model_input).unwrap_or_default();
    let standard_model = if model_input.trim().is_empty() {
        default_standard
    } else {
        model_input.trim().to_string()
    };

    print!("  Enable 3-tier model routing? [Y/n]: ");
    let _ = std::io::stdout().flush();
    let mut routing_input = String::new();
    std::io::stdin().read_line(&mut routing_input).unwrap_or_default();
    let routing_enabled = routing_input.trim().to_lowercase() != "n";

    let (cheap_model, premium_model) = if routing_enabled {
        print!("  Cheap model (research/fast tasks) [{}]: ", default_cheap);
        let _ = std::io::stdout().flush();
        let mut cheap_input = String::new();
        std::io::stdin().read_line(&mut cheap_input).unwrap_or_default();
        let cheap = if cheap_input.trim().is_empty() {
            default_cheap
        } else {
            cheap_input.trim().to_string()
        };

        print!("  Premium model (complex tasks) [{}]: ", default_premium);
        let _ = std::io::stdout().flush();
        let mut premium_input = String::new();
        std::io::stdin().read_line(&mut premium_input).unwrap_or_default();
        let premium = if premium_input.trim().is_empty() {
            default_premium
        } else {
            premium_input.trim().to_string()
        };

        (cheap, premium)
    } else {
        (standard_model.clone(), standard_model.clone())
    };

    // Build YAML config
    let yaml = {
        let mut root = serde_yaml::Mapping::new();
        let sv = |s: &str| serde_yaml::Value::String(s.to_string());
        let bv = |b: bool| serde_yaml::Value::Bool(b);
        root.insert(sv("provider"), sv(&selected_id));
        root.insert(sv("model"), sv(&standard_model));
        if routing_enabled {
            let mut routing = serde_yaml::Mapping::new();
            routing.insert(sv("enabled"), bv(true));
            routing.insert(sv("complexity_routing"), bv(true));
            routing.insert(sv("cheap_model"), sv(&cheap_model));
            routing.insert(sv("standard_model"), sv(&standard_model));
            routing.insert(sv("premium_model"), sv(&premium_model));
            root.insert(sv("model_routing"), serde_yaml::Value::Mapping(routing));
        }
        serde_yaml::to_string(&serde_yaml::Value::Mapping(root))
            .unwrap_or_else(|_| panic!("serialise config"))
    };

    println!("\n  Planned config:\n\n{yaml}");

    print!("  Write this to your global config? [Y/n]: ");
    let _ = std::io::stdout().flush();
    let mut confirm = String::new();
    std::io::stdin().read_line(&mut confirm).unwrap_or_default();
    if confirm.trim().to_lowercase().as_str() == "n"
        || confirm.trim().to_lowercase().as_str() == "no"
    {
        println!("\n  Setup cancelled.");
        return;
    }

    // Write config
    let config_path = std::path::PathBuf::from(crate::config::defaults::get_global_config_path());
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&config_path, &yaml) {
        eprintln!("\n  \x1b[31mFailed to write config: {e}\x1b[0m");
        return;
    }
    println!("\n  \x1b[32m✔\x1b[0m Wrote {}", config_path.display());

    // API key instructions
    if selected_id != "ollama" {
        let url = match selected_id.as_str() {
            "anthropic" => "https://console.anthropic.com/settings/keys",
            "openai" => "https://platform.openai.com/api-keys",
            "groq" => "https://console.groq.com/keys",
            "openrouter" => "https://openrouter.ai/keys",
            "xai" => "https://console.x.ai",
            "mistral" => "https://console.mistral.ai/api-keys",
            "deepseek" => "https://platform.deepseek.com/api_keys",
            _ => "your provider's dashboard",
        };
        println!("\n  \x1b[1mAPI key setup:\x1b[0m");
        println!("    1. Get your key: {url}");
        println!(
            "    2. Add to your shell (~/.zshrc or ~/.bashrc):"
        );
        println!(
            "         export {}=\"your-key-here\"",
            provider_info.api_key_env
        );
        println!("    3. Reload:  source ~/.zshrc (or restart terminal)");
    } else {
        println!("\n  \x1b[1mOllama next steps:\x1b[0m");
        println!("    1. Install Ollama: https://ollama.ai");
        println!(
            "    2. Pull a model:   ollama pull {}",
            provider_info.default_model
        );
        println!("    3. Start server:   ollama serve");
    }

    println!("\n  {}", "─".repeat(46));
    println!("  \x1b[1mYou're all set! \x1b[0m");
    println!("    d3vx doctor\x1b[90m — verify your environment\x1b[0m");
    println!("    d3vx\x1b[90m          — launch the TUI\x1b[0m\n");
}

pub(crate) async fn execute_oneshot(query: &str, cli: &Cli) -> Result<()> {
    // First-run detection before anything else
    let should_proceed = handle_first_run_if_needed();
    if !should_proceed {
        return Ok(());
    }

    let config_result = load_config(LoadConfigOptions {
        project_root: cwd_string(&cli.cwd),
        ..Default::default()
    });

    let (mut config, resolved_model) = match config_result {
        Ok(cfg) => {
            let (model, _, _) = get_provider_config(&cfg);
            let final_model = cli.model.clone().unwrap_or(model);
            (Some(cfg), Some(final_model))
        }
        Err(_) => (None, cli.model.clone()),
    };

    if cli.bypass_permissions || cli.trust {
        if let Some(ref mut c) = config {
            c.permissions.trust_mode = true;
        } else {
            let mut default_cfg = default_config();
            default_cfg.permissions.trust_mode = true;
            config = Some(default_cfg);
        }
    }

    let tui_opts = TuiOptions {
        verbose: cli.verbose,
        cwd: cwd_string(&cli.cwd),
        model: resolved_model,
        session_id: cli.session.clone(),
        ui_mode: cli.ui.clone(),
        stream_out: cli.stream_out.clone(),
        config,
    };

    println!("Processing query: {}", query);
    // TODO: Ideally one-shot wouldn't launch the full TUI if --json is passed
    // but for now, we'll just run the TUI which is the main interface.
    run_tui(tui_opts).await
}

pub(crate) async fn execute_interactive(cli: &Cli) -> Result<()> {
    // First-run detection before anything else
    let should_proceed = handle_first_run_if_needed();
    if !should_proceed {
        return Ok(());
    }

    let config_result = load_config(LoadConfigOptions {
        project_root: cwd_string(&cli.cwd),
        ..Default::default()
    });

    let (mut config, resolved_model) = match config_result {
        Ok(cfg) => {
            let (model, _, _) = get_provider_config(&cfg);
            let final_model = cli.model.clone().unwrap_or(model);
            (Some(cfg), Some(final_model))
        }
        Err(_) => (None, cli.model.clone()),
    };

    if cli.bypass_permissions || cli.trust {
        if let Some(ref mut c) = config {
            c.permissions.trust_mode = true;
        } else {
            let mut default_cfg = default_config();
            default_cfg.permissions.trust_mode = true;
            config = Some(default_cfg);
        }
    }

    let tui_opts = TuiOptions {
        verbose: cli.verbose,
        cwd: cwd_string(&cli.cwd),
        model: resolved_model,
        session_id: cli.session.clone(),
        ui_mode: cli.ui.clone(),
        stream_out: cli.stream_out.clone(),
        config,
    };

    run_tui(tui_opts).await
}
