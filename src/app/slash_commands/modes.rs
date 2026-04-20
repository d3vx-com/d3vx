//! Mode toggle slash commands: verbose, power, vibe, plan, focus mode, model

use anyhow::Result;

use super::*;

pub fn handle_verbose(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.verbose = !app.ui.verbose;
    let state = if app.ui.verbose { "ON" } else { "OFF" };
    app.add_system_message(&format!("Verbose mode: {}", state));
    Ok(())
}

pub fn handle_power(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.power_mode = !app.ui.power_mode;
    app.ui.right_sidebar_visible = app.ui.power_mode;
    app.ui.verbose = app.ui.power_mode;

    let state = if app.ui.power_mode {
        "ON (Advanced telemetry enabled)"
    } else {
        "OFF (Vibe mode restoring)"
    };
    app.add_system_message(&format!("Power Mode: {}", state));
    Ok(())
}

pub fn handle_vibe(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.power_mode = false;
    app.ui.right_sidebar_visible = false;
    app.ui.verbose = false;

    app.add_system_message("Vibe Mode: ON (Telemetry hidden for maximum focus)");
    Ok(())
}

pub fn handle_plan(app: &mut App, _args: &[&str]) -> Result<()> {
    app.ui.plan_mode = !app.ui.plan_mode;
    if app.ui.plan_mode {
        app.ui.mode = crate::app::state::AppMode::Plan;
        app.add_system_message(
            "PLAN MODE ACTIVATED\nWrite, edit, and bash tools are now blocked.\nThe agent can only read files and formulate plans.",
        );
    } else {
        app.ui.mode = crate::app::state::AppMode::Chat;
        app.add_system_message("EXECUTION MODE ACTIVATED\nAll tools are now available.");
    }
    Ok(())
}

pub fn handle_mode(app: &mut App, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        app.add_system_message(&format!(
            "Current focus mode: {}. Available modes: chat, build, plan, docs, test, review.",
            app.ui.focus_mode.label()
        ));
        return Ok(());
    }

    let mode = match args[0].to_ascii_lowercase().as_str() {
        "chat" => crate::app::FocusMode::Chat,
        "build" => crate::app::FocusMode::Build,
        "plan" => crate::app::FocusMode::Plan,
        "docs" => crate::app::FocusMode::Docs,
        "test" => crate::app::FocusMode::Test,
        "review" => crate::app::FocusMode::Review,
        _ => {
            app.add_system_message(
                "Unknown focus mode. Use one of: chat, build, plan, docs, test, review.",
            );
            return Ok(());
        }
    };

    app.ui.focus_mode = mode;
    app.add_system_message(&format!(
        "Focus mode set to {}. {}",
        mode.label(),
        mode.hint()
    ));
    Ok(())
}

pub fn handle_model(app: &mut App, args: &[&str]) -> Result<()> {
    if let Some(new_model) = args.first() {
        // Warn on significant cost deltas. A user typing "/model gpt-4"
        // while on haiku deserves a heads-up — ~50× output cost. We
        // warn if output price changes ≥ 2×. 2× is a bright line: big
        // enough to be meaningful, small enough that intra-tier
        // switches (sonnet ↔ opus) don't nag.
        let old_model = app.model.clone();
        if let Some(warning) = cost_delta_warning(old_model.as_deref(), new_model) {
            app.add_system_message(&warning);
        }
        app.model = Some(new_model.to_string());
        app.add_system_message(&format!("Model switched to: {}", new_model));
    } else {
        // Open the dynamic picker
        app.ui.show_model_picker = true;
        app.ui.model_picker_filter.clear();
        app.ui.model_picker_selected_index = 0;
        app.ui.model_picker_entering_api_key = false;
        app.ui.model_picker_api_key_input.clear();

        app.add_system_message("Opening model configuration...");

        // Trigger a refresh of models in case they haven't been fetched yet
        // Note: Background fetch is handled in App::new initialization
        let _event_tx = app.event_tx.clone();
    }
    Ok(())
}

/// Return a warning string when swapping `old` for `new` would
/// meaningfully change cost. `None` if there's nothing to warn about
/// (no prior model, unknown pricing, or delta below threshold).
///
/// Threshold is a 2× factor on *output* price — output dominates total
/// cost for generative workloads, and 2× is large enough that a typo
/// or misclick is worth surfacing without pestering users who flip
/// between similarly-priced models (e.g. sonnet ↔ opus).
pub(crate) fn cost_delta_warning(old: Option<&str>, new: &str) -> Option<String> {
    let old = old?;
    if old == new {
        return None;
    }
    let old_p = crate::agent::cost::get_pricing(old);
    let new_p = crate::agent::cost::get_pricing(new);
    // If either lookup hit the "Unknown model" hardcoded fallback, the
    // ratio would be misleading — skip the warning.
    if old_p.output <= 0.0 || new_p.output <= 0.0 {
        return None;
    }
    let ratio = new_p.output / old_p.output;
    if ratio >= 2.0 {
        Some(format!(
            "⚠ {new} is ~{ratio:.1}× more expensive than {old} for output tokens (${:.2} vs ${:.2} per 1M). /cost to track.",
            new_p.output, old_p.output
        ))
    } else if ratio <= 0.5 {
        Some(format!(
            "ℹ {new} is ~{inverse:.1}× cheaper than {old} for output tokens (${:.2} vs ${:.2} per 1M).",
            new_p.output,
            old_p.output,
            inverse = 1.0 / ratio,
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_when_no_old_model() {
        assert!(cost_delta_warning(None, "claude-3-5-sonnet").is_none());
    }

    #[test]
    fn returns_none_when_models_are_identical() {
        assert!(cost_delta_warning(Some("claude-3-5-sonnet"), "claude-3-5-sonnet").is_none());
    }

    #[test]
    fn warns_when_jumping_to_a_much_pricier_model() {
        // claude-3-5-haiku ($4 output) → claude-3-opus ($75 output)
        // is roughly 18× — well above the 2× threshold.
        let result = cost_delta_warning(Some("claude-3-5-haiku"), "claude-3-opus");
        assert!(result.is_some(), "expected warning for haiku → opus");
        let msg = result.unwrap();
        assert!(msg.contains("more expensive"), "unexpected msg: {msg}");
    }

    #[test]
    fn informs_when_dropping_to_a_much_cheaper_model() {
        let result = cost_delta_warning(Some("claude-3-opus"), "claude-3-5-haiku");
        assert!(result.is_some(), "expected info for opus → haiku");
        let msg = result.unwrap();
        assert!(msg.contains("cheaper"), "unexpected msg: {msg}");
    }
}
