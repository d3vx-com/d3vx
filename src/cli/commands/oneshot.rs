//! One-shot and Interactive Command Implementations
//!
//! Launch modes: single-query processing and full interactive TUI session.

use std::path::PathBuf;

use anyhow::Result;

use crate::config::{
    defaults::default_config, get_provider_config, load_config, LoadConfigOptions,
};
use crate::ui::runner::{run_tui, TuiOptions};

use crate::cli::args::Cli;

fn cwd_string(cwd: &Option<PathBuf>) -> Option<String> {
    cwd.as_ref().map(|p| p.to_string_lossy().to_string())
}

pub(crate) async fn execute_oneshot(query: &str, cli: &Cli) -> Result<()> {
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
