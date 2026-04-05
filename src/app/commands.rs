//! Command-related Logic

use crate::app::slash_commands::{self, SlashCommand};
use crate::app::App;

impl App {
    /// Get filtered list of slash commands for the command palette
    pub fn get_filtered_commands(&self) -> Vec<SlashCommand> {
        let filter = self.command_palette_filter.to_lowercase();
        slash_commands::SLASH_COMMANDS
            .iter()
            .filter(|cmd| {
                cmd.name.to_lowercase().contains(&filter)
                    || cmd.description.to_lowercase().contains(&filter)
            })
            .cloned()
            .collect()
    }
}
