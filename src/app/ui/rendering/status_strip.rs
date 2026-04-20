//! Bottom status strip — one row, always on.
//!
//! This is the discoverability spine. Everything a new user needs to
//! know *ambiently* lives here:
//!
//! ```text
//!  ○ chat · opus · $0.03 · 23%            dash ● :9876 · daemon ● · 2 bg · ? help
//! ```
//!
//! Left half = identity (mode, model, cost, context%). Right half =
//! live system indicators for the surfaces that were previously
//! invisible: dashboard, daemon, background tasks. The dots are
//! filled (`●`) when that subsystem is active, hollow (`○`) when
//! absent — so the user learns what *exists* just by looking at the
//! bottom of the screen.
//!
//! Nothing here owns new state; every value comes from existing
//! fields on `App`. That's the rule — this strip is a *reader* of
//! state, not a source of truth.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

/// A color pair used across the strip: dim text for labels, slightly
/// less dim for values. Kept local to this module so a future theme
/// rework only has to touch these three constants.
const LABEL: Color = Color::Rgb(90, 90, 105);
const VALUE: Color = Color::Rgb(150, 150, 165);
const SEP: Color = Color::Rgb(55, 55, 68);
const DOT_ON: Color = Color::Rgb(80, 200, 120); // green
const DOT_OFF: Color = Color::Rgb(70, 70, 82); // muted

impl App {
    /// Render the bottom discoverability strip. `area` is assumed to
    /// be exactly 1 row tall — callers allocate that row via the
    /// main layout split.
    pub fn render_discovery_strip(&self, f: &mut Frame, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let left_spans = self.strip_left_spans();
        let right_spans = self.strip_right_spans();

        f.render_widget(Clear, area);

        // Left half: identity info, left-aligned.
        let left = Paragraph::new(Line::from(left_spans)).alignment(Alignment::Left);
        f.render_widget(left, area);

        // Right half: system indicators, right-aligned. Ratatui
        // overdraws in document order; rendering right over left on
        // the same rect works because the left paragraph won't reach
        // the far right unless the terminal is tiny — and in that
        // tiny case, prefer the right half (actionable info) over
        // cost/context%.
        let right = Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right);
        f.render_widget(right, area);
    }

    fn strip_left_spans(&self) -> Vec<Span<'static>> {
        use crate::app::AppMode;

        let mut spans: Vec<Span<'static>> = Vec::new();

        // Mode dot + label.
        let (mode_dot, mode_label) = mode_indicator(self);
        spans.push(Span::styled(
            format!(" {mode_dot} "),
            Style::default().fg(self.ui.theme.brand),
        ));
        spans.push(Span::styled(
            mode_label.to_string(),
            Style::default().fg(VALUE).add_modifier(Modifier::BOLD),
        ));

        // Exit hint when in a mode that takes over the main area.
        // Without this, users who typed `/board` or `/list` get stuck
        // in the view with no visible way back — Esc works but is
        // undiscoverable. Hint is dim so it doesn't fight for
        // attention with real status.
        if matches!(self.ui.mode, AppMode::Board | AppMode::List) {
            spans.push(Span::styled(
                "  Esc to return".to_string(),
                Style::default().fg(LABEL),
            ));
        }

        spans.push(sep());

        // Model.
        spans.push(Span::styled(
            self.model
                .as_deref()
                .unwrap_or("claude")
                .to_string(),
            Style::default().fg(VALUE),
        ));

        // Cost (only if we have one).
        if let Some(cost) = self.session.token_usage.total_cost {
            spans.push(sep());
            spans.push(Span::styled(
                format!("${cost:.3}"),
                Style::default().fg(VALUE),
            ));
        }

        spans
    }

    fn strip_right_spans(&self) -> Vec<Span<'static>> {
        let mut spans: Vec<Span<'static>> = Vec::new();

        // Setup state — most critical signal, leftmost of the right
        // cluster. Only appears when there's a problem (missing
        // config/key). Green dot would be clutter; the absence of a
        // warning *is* the success state.
        let onboarding = crate::config::check_onboarding_status();
        if onboarding.is_first_run || onboarding.needs_api_key_setup {
            spans.push(Span::styled(
                "setup ",
                Style::default().fg(Color::Rgb(220, 100, 100)),
            ));
            spans.push(Span::styled(
                "\u{25CF} ",
                Style::default().fg(Color::Rgb(220, 100, 100)),
            ));
            spans.push(Span::styled(
                "/setup",
                Style::default()
                    .fg(Color::Rgb(220, 180, 60))
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(sep());
        }

        // Dashboard indicator. Dot + port when running; hollow dot
        // when absent.
        let (dash_dot, dash_color) = if self.dashboard.is_some() {
            ("\u{25CF}", DOT_ON)
        } else {
            ("\u{25CB}", DOT_OFF)
        };
        spans.push(Span::styled("dash ", Style::default().fg(LABEL)));
        spans.push(Span::styled(
            format!("{dash_dot} "),
            Style::default().fg(dash_color),
        ));
        if let Some(d) = self.dashboard.as_ref() {
            spans.push(Span::styled(
                format!(":{}", d.config().port),
                Style::default().fg(VALUE),
            ));
        }

        spans.push(sep());

        // Daemon indicator — this is the "will my vex tasks survive?"
        // signal. Filled-green = on, hollow-amber = off (important
        // enough to warrant a non-muted colour when absent).
        let daemon_on = crate::app::slash_commands::daemon_is_running();
        let (dmn_dot, dmn_color) = if daemon_on {
            ("\u{25CF}", DOT_ON)
        } else {
            ("\u{25CB}", Color::Rgb(220, 180, 60)) // amber = warning
        };
        spans.push(Span::styled("daemon ", Style::default().fg(LABEL)));
        spans.push(Span::styled(
            format!("{dmn_dot} "),
            Style::default().fg(dmn_color),
        ));

        spans.push(sep());

        // Background tasks count.
        let bg = self.background_active_tasks.len();
        let bg_color = if bg > 0 { DOT_ON } else { DOT_OFF };
        spans.push(Span::styled(
            format!("{bg} "),
            Style::default().fg(bg_color).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled("bg", Style::default().fg(LABEL)));

        // Plans indicator — only rendered when there's in-flight
        // planner work. Hidden when zero, so the strip doesn't
        // advertise a feature the user isn't currently using.
        let plans = crate::app::slash_commands::plans_count_active(self.cwd.as_deref());
        if plans > 0 {
            spans.push(sep());
            spans.push(Span::styled(
                format!("{plans} "),
                Style::default().fg(DOT_ON).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled("plans", Style::default().fg(LABEL)));
        }

        spans.push(sep());

        // Help hint — always on, always reachable.
        spans.push(Span::styled(
            "? help ",
            Style::default().fg(LABEL).add_modifier(Modifier::DIM),
        ));

        spans
    }
}

fn sep() -> Span<'static> {
    Span::styled(" · ".to_string(), Style::default().fg(SEP))
}

/// Map the current AppMode to a glyph + short label for the strip.
fn mode_indicator(app: &App) -> (&'static str, &'static str) {
    use crate::app::AppMode;
    match app.ui.mode {
        AppMode::Chat => ("\u{25CB}", "chat"),
        AppMode::Board => ("\u{25A3}", "board"),
        AppMode::List => ("\u{2261}", "list"),
        AppMode::CommandPalette => ("\u{25C9}", "palette"),
        AppMode::Help => ("?", "help"),
        _ => ("\u{25CB}", "chat"),
    }
}
