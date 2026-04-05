//! Model Picker Widget
//!
//! Provides a rich interface for selecting models for different complexity tiers.
//! Fetches data from the ModelRegistry and supports filtering and API key entry.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::state::UIState;
use crate::providers::{ComplexityTier, ModelInfo};
use crate::ui::theme::Theme;

pub struct ModelPicker;

impl ModelPicker {
    pub fn render(
        f: &mut Frame,
        area: Rect,
        app_state: &UIState,
        models: &[&ModelInfo],
        theme: &Theme,
    ) {
        // Calculate popup dimensions
        let popup_height = std::cmp::min(30, area.height.saturating_sub(2));
        let popup_width = std::cmp::min(120, area.width.saturating_sub(4));

        let popup_area = Rect {
            x: (area.width.saturating_sub(popup_width)) / 2,
            y: (area.height.saturating_sub(popup_height)) / 2,
            width: popup_width,
            height: popup_height,
        };

        // Clear the area first
        f.render_widget(Clear, popup_area);

        // Define layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title/Legend
                Constraint::Length(3), // Search
                Constraint::Min(10),   // Tiers
                Constraint::Length(if app_state.model_picker_entering_api_key {
                    5
                } else {
                    3
                }), // Footer/Key prompt
            ])
            .split(popup_area);

        // 1. Title
        let title = Paragraph::new(vec![Line::from(vec![
            Span::styled(
                " Model Configuration ",
                Style::default()
                    .fg(theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " (Tab/Arrows to navigate, Enter to select, Esc to close)",
                Style::default().fg(theme.ui.text_dim),
            ),
        ])])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.ui.border)),
        );
        f.render_widget(title, chunks[0]);

        // 2. Search Bar
        let search_text = if app_state.model_picker_filter.is_empty()
            && !app_state.model_picker_entering_api_key
        {
            Span::styled(
                " Type to filter models...",
                Style::default().fg(theme.ui.text_dim),
            )
        } else {
            Span::raw(&app_state.model_picker_filter)
        };

        let search_block = Paragraph::new(Line::from(search_text)).block(
            Block::default()
                .title(" Filter Models ")
                .borders(Borders::ALL)
                .border_style(
                    Style::default().fg(if !app_state.model_picker_entering_api_key {
                        theme.brand
                    } else {
                        theme.ui.border
                    }),
                ),
        );
        f.render_widget(search_block, chunks[1]);

        // 3. Three Tiers Layout
        let tier_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(chunks[2]);

        let tiers = [
            ComplexityTier::Simple,
            ComplexityTier::Standard,
            ComplexityTier::Complex,
        ];
        let tier_names = [" Cheap (Simple) ", " Standard ", " Complex (Reasoning) "];

        for (i, tier) in tiers.iter().enumerate() {
            let is_active_tier = app_state.model_picker_selected_tier == *tier;

            // Filter models for this tier and search term
            let tier_models: Vec<&&ModelInfo> = models
                .iter()
                .filter(|m| m.tier == *tier)
                .filter(|m| {
                    app_state.model_picker_filter.is_empty()
                        || m.name
                            .to_lowercase()
                            .contains(&app_state.model_picker_filter.to_lowercase())
                        || m.id
                            .to_lowercase()
                            .contains(&app_state.model_picker_filter.to_lowercase())
                })
                .collect();

            let items: Vec<ListItem> = tier_models
                .iter()
                .enumerate()
                .map(|(idx, m)| {
                    let is_selected =
                        is_active_tier && idx == app_state.model_picker_selected_index;

                    let cost_info = if let (Some(input), Some(output)) =
                        (m.cost_per_input_mtok, m.cost_per_output_mtok)
                    {
                        format!(" (${:.2}/${:.2})", input, output)
                    } else {
                        String::new()
                    };

                    let style = if is_selected {
                        Style::default().fg(Color::Black).bg(theme.brand)
                    } else {
                        Style::default().fg(theme.ui.text_muted)
                    };

                    let content = vec![
                        Line::from(vec![
                            Span::styled(format!(" {} ", m.name), style),
                            Span::styled(cost_info, Style::default().fg(theme.ui.text_dim)),
                        ]),
                        Line::from(vec![Span::styled(
                            format!("    {}", m.id),
                            Style::default().fg(theme.ui.text_dim),
                        )]),
                    ];

                    ListItem::new(content)
                })
                .collect();

            let block = Block::default()
                .title(tier_names[i])
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if is_active_tier {
                    theme.brand
                } else {
                    theme.ui.border
                }));

            let list = List::new(items).block(block);
            f.render_widget(list, tier_chunks[i]);
        }

        // 4. Footer / API Key Prompt
        if app_state.model_picker_entering_api_key {
            let selected_model = Self::get_selected_model(app_state, models);
            let provider = selected_model
                .map(|m| m.provider.as_str())
                .unwrap_or("Provider");

            let prompt = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!(" NO API KEY FOUND for {}. ", provider),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Please enter it below (will be saved to global config):"),
                ]),
                Line::from(Span::raw(&app_state.model_picker_api_key_input)),
            ])
            .block(
                Block::default()
                    .title(" API Key Required ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            )
            .wrap(Wrap { trim: true });

            f.render_widget(prompt, chunks[3]);
        } else {
            let footer = Paragraph::new(
                " [Tab] Switch Tier  [Arrows] Navigate  [Enter] Select  [Esc] Done ",
            )
            .style(Style::default().fg(theme.ui.text_dim))
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(theme.ui.border)),
            );
            f.render_widget(footer, chunks[3]);
        }
    }

    fn get_selected_model<'a>(
        app_state: &UIState,
        models: &'a [&ModelInfo],
    ) -> Option<&'a ModelInfo> {
        let tier_models: Vec<&&ModelInfo> = models
            .iter()
            .filter(|m| m.tier == app_state.model_picker_selected_tier)
            .filter(|m| {
                app_state.model_picker_filter.is_empty()
                    || m.name
                        .to_lowercase()
                        .contains(&app_state.model_picker_filter.to_lowercase())
                    || m.id
                        .to_lowercase()
                        .contains(&app_state.model_picker_filter.to_lowercase())
            })
            .collect();

        tier_models
            .get(app_state.model_picker_selected_index)
            .map(|m| **m)
    }
}
