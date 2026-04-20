//! Help Modal Widget
//!
//! Minimal popup showing essential keyboard shortcuts.

use crate::ui::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// A help modal that displays essential keybindings and slash commands.
pub struct HelpModal {
    theme: Theme,
    scroll: usize,
}

impl HelpModal {
    pub fn new(theme: Theme, scroll: usize) -> Self {
        Self { theme, scroll }
    }

    /// Helper to create centered rect of relative size
    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}

impl Widget for HelpModal {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let popup_area = self.centered_rect(70, 70, area);
        Clear.render(popup_area, buf);

        let block = Block::default()
            .title(Span::styled(
                " Help & Commands ",
                Style::default()
                    .fg(self.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.brand))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let mut text = Vec::new();

        // 0. Getting Started
        text.push(Line::from(vec![
            Span::styled("─ Getting Started ", Style::default().fg(self.theme.brand)),
            Span::styled(
                "─────────────────────────────────────",
                Style::default().fg(self.theme.ui.border),
            ),
        ]));

        let steps = [
            ("1.", "Describe what you want done"),
            ("2.", "Review the result in the conversation"),
            ("3.", "Approve changes or ask for adjustments"),
        ];
        for (num, desc) in steps {
            text.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", num),
                    Style::default()
                        .fg(self.theme.brand)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc, Style::default().fg(self.theme.ui.text)),
            ]));
        }

        text.push(Line::from(vec![
            Span::styled("  Add ", Style::default().fg(self.theme.ui.text_dim)),
            Span::styled(
                "--vex ",
                Style::default()
                    .fg(self.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "to run a task in an isolated worktree.",
                Style::default().fg(self.theme.ui.text_dim),
            ),
        ]));

        text.push(Line::from(""));

        // 1. Keyboard Shortcuts
        text.push(Line::from(vec![
            Span::styled(
                "─ Keyboard Shortcuts ",
                Style::default().fg(self.theme.brand),
            ),
            Span::styled(
                "──────────────────────────",
                Style::default().fg(self.theme.ui.border),
            ),
        ]));

        // Grouped by *where you'd use them* so a user can scan the
        // block that matches their current context rather than
        // memorise a flat list of 20 bindings.
        //
        // NOTE: this table is the canonical keybinding documentation.
        // When adding or changing a handler in
        // `src/app/handlers/keyboard/`, update the matching row here.
        // The previous Ctrl+L drift ("Toggle left sidebar" in help,
        // right-sidebar in code) shipped because these two lived in
        // different files with no enforcement.
        let shortcut_groups: &[(&str, &[(&str, &str)])] = &[
            (
                "Chat",
                &[
                    ("Enter", "Send message"),
                    ("\\ + Enter", "New line within a message"),
                    ("↑ / ↓", "History (with prefix search)"),
                    ("Esc", "Stop streaming · close modal · dismiss welcome"),
                    ("Esc Esc", "Double-tap: open undo picker"),
                    ("Ctrl+C", "Interrupt · press twice to quit"),
                    ("Ctrl+U", "Clear input line"),
                    ("Ctrl+X", "Pop last message from queue"),
                ],
            ),
            (
                "Slash palette",
                &[
                    ("/", "Open the live command palette"),
                    ("↑ / ↓", "Navigate palette while open"),
                    ("Tab", "Complete the highlighted command"),
                    ("Enter", "Accept and run the highlighted command"),
                ],
            ),
            (
                "Views & layout",
                &[
                    ("Ctrl+1..4", "Switch right-panel tab"),
                    ("Ctrl+L / Ctrl+R", "Toggle right sidebar"),
                    ("Ctrl+N", "Focus the sidebar / navigator"),
                    ("Ctrl+W", "Toggle detail drawer"),
                    ("Ctrl+O", "Expand/collapse tool output"),
                    ("Ctrl+A", "Pin agent monitor to the sidebar"),
                    ("Ctrl+S", "Toggle agent-strip expanded view"),
                    ("Ctrl+P", "Toggle Power Mode"),
                    ("?", "Quick help · Esc to close"),
                ],
            ),
            (
                "Agents & workspaces",
                &[
                    ("Ctrl+↑ / ↓", "Navigate inline agents"),
                    ("Ctrl+E", "Select highlighted agent"),
                    ("Alt+← / →", "Switch workspace"),
                    ("Alt+PgUp / PgDn", "Scroll selected agent transcript"),
                    ("Ctrl+F", "Cycle focus mode (chat → build → plan …)"),
                ],
            ),
            (
                "Diff preview",
                &[
                    ("Ctrl+D", "Toggle full-screen diff view"),
                    ("Ctrl+← / →", "Cycle changed files"),
                    ("Esc / q", "Close diff"),
                ],
            ),
            (
                "Board / list",
                &[
                    ("j / k or ↑ / ↓", "Move selection up / down"),
                    ("h / l or ← / →", "Move selection left / right (board)"),
                    ("H / L", "Move task between columns (board)"),
                    ("a", "Quick-add task in Inbox column (board)"),
                    ("Space", "Toggle task done (list)"),
                    ("Enter", "Open selected task / switch workspace"),
                    ("Esc / q", "Return to chat"),
                ],
            ),
        ];

        for (group_title, bindings) in shortcut_groups {
            text.push(Line::from(vec![Span::styled(
                format!("  {}", group_title),
                Style::default()
                    .fg(self.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            )]));
            for (key, desc) in *bindings {
                text.push(Line::from(vec![
                    Span::styled(
                        format!("  {:>14} ", key),
                        Style::default()
                            .fg(self.theme.brand)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*desc, Style::default().fg(self.theme.ui.text_dim)),
                ]));
            }
            text.push(Line::raw(""));
        }

        // 2. Slash Commands — grouped by category
        text.push(Line::from(vec![
            Span::styled("─ Slash Commands ", Style::default().fg(self.theme.brand)),
            Span::styled(
                "─────────────────────────────",
                Style::default().fg(self.theme.ui.border),
            ),
        ]));

        use crate::app::slash_commands::{CATEGORY_ORDER, SLASH_COMMANDS};
        for category in CATEGORY_ORDER {
            let matching: Vec<_> = SLASH_COMMANDS
                .iter()
                .filter(|c| c.category == *category)
                .collect();
            if matching.is_empty() {
                continue;
            }

            text.push(Line::from(vec![Span::styled(
                format!("  {}", category),
                Style::default()
                    .fg(self.theme.brand_secondary)
                    .add_modifier(Modifier::BOLD),
            )]));

            for cmd in matching {
                text.push(Line::from(vec![
                    Span::styled(
                        format!("  /{:<11} ", cmd.name),
                        Style::default()
                            .fg(self.theme.brand)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(cmd.description, Style::default().fg(self.theme.ui.text_dim)),
                ]));
            }
            text.push(Line::raw(""));
        }

        // 3. CLI flags — set when launching `d3vx`, not inside the TUI.
        // Users reading this while running `cargo run --` need to know
        // what the flags do and that restarting is required to apply
        // them.
        text.push(Line::from(vec![
            Span::styled("─ CLI flags (at launch) ", Style::default().fg(self.theme.brand)),
            Span::styled(
                "──────────────────────",
                Style::default().fg(self.theme.ui.border),
            ),
        ]));
        let cli_flags: &[(&str, &str)] = &[
            ("--vex \"<task>\"", "Run the task in an isolated git worktree; daemon owns it"),
            ("--parallel-agents", "Enable parallel sub-agent orchestration"),
            ("--trust", "Auto-approve every tool (skip permission prompts)"),
            ("--bypass-permissions", "Skip all permission checks (superset of --trust)"),
            ("--no-daemon", "Don't auto-start the background daemon"),
            ("--no-stream", "Buffer the full response instead of streaming"),
            ("--verbose", "Enable debug-level tracing"),
            ("--resume", "Open the session picker on launch"),
            ("--continue", "Auto-resume the most recent session"),
            ("--ui <mode>", "Start in chat / kanban / list view"),
            ("--model <name>", "Pick a model for this run (or D3VX_MODEL env)"),
            ("--provider <name>", "Pick a provider (anthropic / openai / groq / ...)"),
        ];
        for (flag, desc) in cli_flags {
            text.push(Line::from(vec![
                Span::styled(
                    format!("  {:<22} ", flag),
                    Style::default()
                        .fg(self.theme.brand)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*desc, Style::default().fg(self.theme.ui.text_dim)),
            ]));
        }
        text.push(Line::raw(""));

        text.push(Line::from(""));
        text.push(Line::from(vec![Span::styled(
            "  Use ↑/↓ or j/k to scroll. Esc to close.",
            Style::default().fg(self.theme.ui.text_muted),
        )]));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Left)
            .scroll((self.scroll as u16, 0));

        paragraph.render(popup_area, buf);
    }
}
