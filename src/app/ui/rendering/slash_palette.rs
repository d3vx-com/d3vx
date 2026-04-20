//! Live slash-command palette.
//!
//! Renders a compact dropdown above the input area whenever the
//! prompt starts with `/` and the user is still typing the command
//! token (no whitespace yet). The dropdown lists slash commands
//! filtered by prefix + substring on the typed fragment, showing the
//! name and a one-line description.
//!
//! This is the killer discovery surface — a user who types `/` at
//! the prompt sees every command the app offers, with descriptions,
//! without having to open a modal or memorise shortcuts. Filtering
//! happens live as they keep typing.
//!
//! Module shape:
//!
//! - **Pure helpers** (`palette_visible_for`, `match_commands`,
//!   `wrap_next`, `wrap_prev`) — no `App`, no `Frame`, fully
//!   unit-testable.
//! - **`impl App`** methods — thin wrappers that feed `App` state
//!   into those helpers and perform the side-effects (replace buffer
//!   on accept, move cursor, trigger render).
//!
//! Behaviour rules:
//!
//! - Visibility is a pure function of `input_buffer`. No extra mode
//!   flag — the prompt *is* the state.
//! - Up/Down navigate; Tab accepts (completes name + space); Enter
//!   runs. Wired in keyboard handlers, not here.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
use ratatui::Frame;

use crate::app::slash_commands::{SlashCommand, SLASH_COMMANDS};
use crate::app::App;

/// Maximum rows of commands to show at once. Beyond this, the user
/// keeps typing to narrow. 8 covers the common case without
/// dominating vertical real estate on small terminals.
const MAX_VISIBLE: usize = 8;

// ── Pure helpers (testable without an `App`) ────────────────────

/// True if the palette should be visible given a prompt buffer.
/// Visibility is "starts with `/`, no whitespace yet" — the moment
/// the user types a space, they're supplying arguments and the
/// dropdown would be in the way.
pub fn palette_visible_for(buffer: &str) -> bool {
    let buf = buffer.trim_start();
    buf.starts_with('/') && !buf.chars().any(char::is_whitespace)
}

/// Rank-filter slash commands against a fragment. Prefix matches
/// first (by declaration order inside `SLASH_COMMANDS`), then
/// substring matches on name or description. Empty fragment →
/// return everything.
pub fn match_commands(fragment: &str) -> Vec<&'static SlashCommand> {
    let frag = fragment.to_ascii_lowercase();
    if frag.is_empty() {
        return SLASH_COMMANDS.iter().collect();
    }

    let mut prefix: Vec<&'static SlashCommand> = Vec::new();
    let mut substring: Vec<&'static SlashCommand> = Vec::new();
    for cmd in SLASH_COMMANDS {
        let name = cmd.name.to_ascii_lowercase();
        if name.starts_with(&frag) {
            prefix.push(cmd);
        } else if name.contains(&frag)
            || cmd.description.to_ascii_lowercase().contains(&frag)
        {
            substring.push(cmd);
        }
    }
    prefix.extend(substring);
    prefix
}

/// Wrap-around advance. `len == 0` is a no-op. A `current` that's
/// past the end is clamped to the last element before advancing, so
/// a shrinking match list never points at thin air.
pub fn wrap_next(current: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let cur = current.min(len - 1);
    (cur + 1) % len
}

/// Wrap-around back. `len == 0` is a no-op.
pub fn wrap_prev(current: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let cur = current.min(len - 1);
    if cur == 0 {
        len - 1
    } else {
        cur - 1
    }
}

/// Extract the fragment after the leading `/`. `"/dash"` → `"dash"`,
/// `"/"` → `""`.
fn fragment_of(buffer: &str) -> &str {
    buffer.trim_start().strip_prefix('/').unwrap_or("")
}

// ── `impl App` wrappers ──────────────────────────────────────────

impl App {
    pub fn is_slash_palette_open(&self) -> bool {
        palette_visible_for(&self.ui.input_buffer)
    }

    pub fn slash_palette_matches(&self) -> Vec<&'static SlashCommand> {
        match_commands(fragment_of(&self.ui.input_buffer))
    }

    pub fn slash_palette_select_next(&mut self) {
        let n = self.slash_palette_matches().len();
        self.ui.slash_palette_selected = wrap_next(self.ui.slash_palette_selected, n);
    }

    pub fn slash_palette_select_prev(&mut self) {
        let n = self.slash_palette_matches().len();
        self.ui.slash_palette_selected = wrap_prev(self.ui.slash_palette_selected, n);
    }

    /// Replace the input buffer with the selected command name plus a
    /// trailing space (so the user can type arguments). Resets the
    /// selection cursor. No-op if no match is selected.
    pub fn slash_palette_accept(&mut self) {
        let matches = self.slash_palette_matches();
        if matches.is_empty() {
            return;
        }
        let idx = self.ui.slash_palette_selected.min(matches.len() - 1);
        let name = matches[idx].name;
        let replacement = format!("/{name} ");
        self.ui.input_buffer = replacement.clone();
        self.ui.cursor_position = replacement.len();
        self.ui.slash_palette_selected = 0;
    }

    /// Render the palette immediately above `input_area`. Callers
    /// must gate on `is_slash_palette_open`.
    pub fn render_slash_palette(&self, f: &mut Frame, input_area: Rect) {
        let matches = self.slash_palette_matches();
        if matches.is_empty() {
            return;
        }

        let visible = matches.len().min(MAX_VISIBLE);
        let popup_width = input_area.width.min(60).max(32);
        let popup_height = (visible as u16).saturating_add(2); // +2 for borders

        let x = input_area.x;
        let y = input_area.y.saturating_sub(popup_height);
        let popup_area = Rect {
            x,
            y,
            width: popup_width,
            height: popup_height,
        };
        if popup_area.height < 3 || popup_area.width < 4 {
            return;
        }

        f.render_widget(Clear, popup_area);

        let sel = self.ui.slash_palette_selected.min(visible.saturating_sub(1));
        let items: Vec<ListItem> = matches
            .iter()
            .take(visible)
            .enumerate()
            .map(|(i, cmd)| render_row(cmd, i == sel, &self.ui.theme))
            .collect();

        let title = format!(" / {} ", matches.len());
        let list = List::new(items).block(
            Block::default()
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(self.ui.theme.brand)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.ui.theme.ui.border_muted)),
        );
        f.render_widget(list, popup_area);
    }
}

fn render_row(
    cmd: &SlashCommand,
    selected: bool,
    theme: &crate::ui::theme::Theme,
) -> ListItem<'static> {
    let name_style = if selected {
        Style::default()
            .fg(theme.brand)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.brand).add_modifier(Modifier::DIM)
    };
    let desc_style = if selected {
        Style::default().fg(theme.ui.text)
    } else {
        Style::default().fg(theme.ui.text_muted)
    };

    let row = Line::from(vec![
        Span::styled(" /", name_style),
        Span::styled(cmd.name, name_style),
        Span::raw("  "),
        Span::styled(cmd.description, desc_style),
    ]);

    let item_style = if selected {
        Style::default().bg(Color::Rgb(30, 30, 40))
    } else {
        Style::default()
    };
    ListItem::new(row).style(item_style)
}
