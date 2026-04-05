//! Theme System - Color configuration for the TUI
//!
//! Provides a unified theming system with semantic color names,
//! matching the TypeScript theme in src/tui/utils/theme.ts

use ratatui::style::Color;

// ────────────────────────────────────────────────────────────
// Theme Mode
// ────────────────────────────────────────────────────────────

/// Theme mode (dark/light)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    DarkAnsi,
    LightAnsi,
    DarkDaltonized,
    LightDaltonized,
}

// ────────────────────────────────────────────────────────────
// Shimmer Colors
// ────────────────────────────────────────────────────────────

/// Shimmer/glimmer colors for streaming text
#[derive(Debug, Clone, Copy)]
pub struct ShimmerColors {
    /// Base assistant color
    pub assistant: Color,
    /// Glimmer highlight for assistant
    pub assistant_glimmer: Color,
    /// Base tool use color
    pub tool_use: Color,
    /// Glimmer highlight for tools
    pub tool_use_glimmer: Color,
    /// Base spinner color
    pub spinner: Color,
    /// Glimmer highlight for spinner
    pub spinner_glimmer: Color,
}

// ────────────────────────────────────────────────────────────
// Role Colors
// ────────────────────────────────────────────────────────────

/// Colors for message roles
#[derive(Debug, Clone, Copy)]
pub struct RoleColors {
    pub user: Color,
    pub assistant: Color,
    pub system: Color,
    pub shell: Color,
}

// ────────────────────────────────────────────────────────────
// State Colors
// ────────────────────────────────────────────────────────────

/// Colors for status states
#[derive(Debug, Clone, Copy)]
pub struct StateColors {
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub pending: Color,
}

// ────────────────────────────────────────────────────────────
// Diff Colors
// ────────────────────────────────────────────────────────────

/// Colors for diff display
#[derive(Debug, Clone, Copy)]
pub struct DiffColors {
    pub added: Color,
    pub removed: Color,
    pub added_dimmed: Color,
    pub removed_dimmed: Color,
    pub added_text: Color,
    pub removed_text: Color,
}

// ────────────────────────────────────────────────────────────
// Syntax Highlighting Colors
// ────────────────────────────────────────────────────────────

/// Colors for syntax highlighting
#[derive(Debug, Clone, Copy)]
pub struct SyntaxColors {
    pub keyword: Color,
    pub string: Color,
    pub number: Color,
    pub comment: Color,
    pub function: Color,
    pub variable: Color,
    pub code: Color,
}

// ────────────────────────────────────────────────────────────
// UI Colors
// ────────────────────────────────────────────────────────────

/// Colors for UI elements
#[derive(Debug, Clone, Copy)]
pub struct UiColors {
    pub border: Color,
    pub border_active: Color,
    pub border_muted: Color,
    pub separator: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_dim: Color,
    pub suggestion: Color,
}

// ────────────────────────────────────────────────────────────
// Main Theme
// ────────────────────────────────────────────────────────────

/// Complete theme configuration
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub mode: ThemeMode,
    pub brand: Color,
    pub brand_secondary: Color,
    pub shimmer: ShimmerColors,
    pub role: RoleColors,
    pub state: StateColors,
    pub diff: DiffColors,
    pub syntax: SyntaxColors,
    pub ui: UiColors,

    // Legacy compatibility (matching TypeScript)
    pub claude: Color,
    pub bash_border: Color,
    pub permission: Color,
    pub secondary_border: Color,
}

impl Theme {
    /// Get the dark theme
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            brand: Color::Rgb(16, 185, 129),            // #10B981
            brand_secondary: Color::Rgb(113, 113, 122), // #71717A

            shimmer: ShimmerColors {
                assistant: Color::Rgb(217, 119, 87),          // #D97757
                assistant_glimmer: Color::Rgb(241, 149, 117), // #F19575
                tool_use: Color::Rgb(139, 92, 246),           // #8B5CF6
                tool_use_glimmer: Color::Rgb(167, 139, 250),  // #A78BFA
                spinner: Color::Rgb(16, 185, 129),            // #10B981
                spinner_glimmer: Color::Rgb(52, 211, 153),    // #34D399
            },

            role: RoleColors {
                user: Color::Gray,
                assistant: Color::Gray,
                system: Color::Yellow,
                shell: Color::Yellow,
            },

            state: StateColors {
                success: Color::Green,
                error: Color::Red,
                warning: Color::Yellow,
                info: Color::Blue,
                pending: Color::Yellow,
            },

            diff: DiffColors {
                added: Color::Rgb(34, 92, 43),           // #225c2b
                removed: Color::Rgb(122, 41, 54),        // #7a2936
                added_dimmed: Color::Rgb(71, 88, 74),    // #47584a
                removed_dimmed: Color::Rgb(105, 72, 77), // #69484d
                added_text: Color::Green,
                removed_text: Color::Red,
            },

            syntax: SyntaxColors {
                keyword: Color::Magenta,
                string: Color::Green,
                number: Color::Yellow,
                comment: Color::Gray,
                function: Color::Blue,
                variable: Color::Cyan,
                code: Color::Cyan,
            },

            ui: UiColors {
                border: Color::Gray,
                border_active: Color::Cyan,
                border_muted: Color::Gray,
                separator: Color::Gray,
                text: Color::Rgb(255, 255, 255),
                text_muted: Color::Rgb(153, 153, 153),
                text_dim: Color::Rgb(102, 102, 102),
                suggestion: Color::Rgb(177, 185, 249),
            },

            // Legacy
            claude: Color::Rgb(217, 119, 87),
            bash_border: Color::Rgb(253, 93, 177),
            permission: Color::Rgb(177, 185, 249),
            secondary_border: Color::Rgb(136, 136, 136),
        }
    }

    /// Get the light theme
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            brand: Color::Rgb(5, 150, 105),          // #059669
            brand_secondary: Color::Rgb(82, 82, 91), // #52525B

            shimmer: ShimmerColors {
                assistant: Color::Rgb(217, 119, 87),         // #D97757
                assistant_glimmer: Color::Rgb(184, 100, 74), // #B8644A
                tool_use: Color::Rgb(124, 58, 237),          // #7C3AED
                tool_use_glimmer: Color::Rgb(109, 40, 217),  // #6D28D9
                spinner: Color::Rgb(5, 150, 105),            // #059669
                spinner_glimmer: Color::Rgb(4, 120, 87),     // #047857
            },

            role: RoleColors {
                user: Color::Gray,
                assistant: Color::Gray,
                system: Color::Yellow,
                shell: Color::Yellow,
            },

            state: StateColors {
                success: Color::Green,
                error: Color::Red,
                warning: Color::Yellow,
                info: Color::Blue,
                pending: Color::Yellow,
            },

            diff: DiffColors {
                added: Color::Rgb(105, 219, 124),          // #69db7c
                removed: Color::Rgb(255, 168, 180),        // #ffa8b4
                added_dimmed: Color::Rgb(199, 225, 203),   // #c7e1cb
                removed_dimmed: Color::Rgb(253, 210, 216), // #fdd2d8
                added_text: Color::Green,
                removed_text: Color::Red,
            },

            syntax: SyntaxColors {
                keyword: Color::Magenta,
                string: Color::Green,
                number: Color::Yellow,
                comment: Color::Gray,
                function: Color::Blue,
                variable: Color::Cyan,
                code: Color::Cyan,
            },

            ui: UiColors {
                border: Color::Gray,
                border_active: Color::Cyan,
                border_muted: Color::Gray,
                separator: Color::Gray,
                text: Color::Rgb(0, 0, 0),
                text_muted: Color::Rgb(102, 102, 102),
                text_dim: Color::Rgb(153, 153, 153),
                suggestion: Color::Rgb(87, 105, 247),
            },

            // Legacy
            claude: Color::Rgb(217, 119, 87),
            bash_border: Color::Rgb(255, 0, 135),
            permission: Color::Rgb(87, 105, 247),
            secondary_border: Color::Rgb(153, 153, 153),
        }
    }

    /// Get theme by mode
    pub fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark | ThemeMode::DarkAnsi | ThemeMode::DarkDaltonized => Self::dark(),
            ThemeMode::Light | ThemeMode::LightAnsi | ThemeMode::LightDaltonized => Self::light(),
        }
    }

    /// Interpolate between two colors
    pub fn interpolate_color(c1: Color, c2: Color, t: f32) -> Color {
        match (c1, c2) {
            (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => Color::Rgb(
                (r1 as f32 + (r2 as f32 - r1 as f32) * t) as u8,
                (g1 as f32 + (g2 as f32 - g1 as f32) * t) as u8,
                (b1 as f32 + (b2 as f32 - b1 as f32) * t) as u8,
            ),
            // Fallback for non-RGB colors
            (c1, _) if t < 0.5 => c1,
            (_, c2) => c2,
        }
    }

    /// Get shimmer color based on progress (0.0-1.0)
    pub fn shimmer_color(&self, base: Color, glimmer: Color, progress: f32) -> Color {
        Self::interpolate_color(base, glimmer, progress)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

// ────────────────────────────────────────────────────────────
// Tool Colors
// ────────────────────────────────────────────────────────────

/// Get color for a tool name
pub fn get_tool_color(tool_name: &str) -> Color {
    match tool_name {
        // File tools
        "ReadTool" | "WriteTool" | "EditTool" | "MultiEditTool" => Color::Blue,

        // Search tools
        "GrepTool" | "GlobTool" => Color::Cyan,

        // Execute tools
        "BashTool" | "Bash" | "Task" => Color::Magenta,

        // Network tools
        "WebSearchTool" | "WebFetchTool" | "webSearchTool" => Color::Blue,

        // Special tools
        "ThinkTool" => Color::Gray,
        "QuestionTool" => Color::Yellow,
        "TodoWriteTool" => Color::Cyan,

        // Default
        _ => Color::White,
    }
}

// ────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme() {
        let theme = Theme::dark();
        assert_eq!(theme.brand, Color::Rgb(16, 185, 129));
    }

    #[test]
    fn test_light_theme() {
        let theme = Theme::light();
        assert_eq!(theme.brand, Color::Rgb(5, 150, 105));
    }

    #[test]
    fn test_color_interpolation() {
        let c1 = Color::Rgb(0, 0, 0);
        let c2 = Color::Rgb(100, 100, 100);
        let mid = Theme::interpolate_color(c1, c2, 0.5);
        assert_eq!(mid, Color::Rgb(50, 50, 50));
    }

    #[test]
    fn test_tool_colors() {
        assert_eq!(get_tool_color("ReadTool"), Color::Blue);
        assert_eq!(get_tool_color("BashTool"), Color::Magenta);
        assert_eq!(get_tool_color("GrepTool"), Color::Cyan);
    }
}
