//! Unicode-box-drawing table renderer for the markdown widget.
//!
//! Replaces a previous implementation that emitted per-column ASCII
//! boxes — which produced disjoint rectangles (`+---+ +---+`) and
//! double pipes at cell boundaries (`| a || b |`). This renderer uses
//! shared borders and the proper corner / junction glyphs:
//!
//! ```text
//! ┌───────┬──────────┐
//! │ head1 │ header2  │
//! ├───────┼──────────┤
//! │ val1  │ value2   │
//! └───────┴──────────┘
//! ```
//!
//! Column widths auto-size to the widest cell per column. The first
//! row is rendered bold in the theme's brand colour (header
//! convention from GitHub-flavoured markdown).

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::ui::theme::Theme;

/// Turn a 2D grid of cell strings into a vec of styled lines. An empty
/// grid returns an empty vec (no phantom border). The first row is
/// treated as the header.
pub(super) fn render_table(rows: &[Vec<String>], theme: &Theme) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return Vec::new();
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return Vec::new();
    }

    let widths = column_widths(rows, num_cols);
    let border_style = Style::default().fg(theme.ui.border);
    let header_style = Style::default()
        .fg(theme.brand)
        .add_modifier(Modifier::BOLD);
    let body_style = Style::default().fg(theme.ui.text);

    let mut out = Vec::with_capacity(rows.len() + 3);
    out.push(Line::styled(
        build_border(&widths, '┌', '┬', '┐'),
        border_style,
    ));

    for (r, row) in rows.iter().enumerate() {
        out.push(build_row(row, &widths, num_cols, border_style, cell_style(r, header_style, body_style)));
        if r == 0 {
            out.push(Line::styled(
                build_border(&widths, '├', '┼', '┤'),
                border_style,
            ));
        }
    }
    out.push(Line::styled(
        build_border(&widths, '└', '┴', '┘'),
        border_style,
    ));
    out
}

fn cell_style(row_idx: usize, header: Style, body: Style) -> Style {
    if row_idx == 0 {
        header
    } else {
        body
    }
}

fn column_widths(rows: &[Vec<String>], num_cols: usize) -> Vec<usize> {
    let mut widths = vec![0usize; num_cols];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                widths[i] = widths[i].max(display_width(cell));
            }
        }
    }
    widths
}

fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn build_border(widths: &[usize], left: char, junction: char, right: char) -> String {
    let mut out = String::with_capacity(widths.iter().sum::<usize>() * 2 + widths.len() * 2 + 2);
    out.push(left);
    for (i, w) in widths.iter().enumerate() {
        // `+ 2` for the single-space padding inside each cell.
        for _ in 0..(*w + 2) {
            out.push('─');
        }
        out.push(if i + 1 == widths.len() { right } else { junction });
    }
    out
}

fn build_row(
    row: &[String],
    widths: &[usize],
    num_cols: usize,
    border_style: Style,
    text_style: Style,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(num_cols * 2 + 1);
    spans.push(Span::styled("│".to_string(), border_style));
    for i in 0..num_cols {
        let cell = row.get(i).cloned().unwrap_or_default();
        let pad = widths[i].saturating_sub(display_width(&cell));
        spans.push(Span::styled(
            format!(" {cell}{} ", " ".repeat(pad)),
            text_style,
        ));
        spans.push(Span::styled("│".to_string(), border_style));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(rows: Vec<Vec<&str>>) -> Vec<String> {
        let theme = Theme::dark();
        let rows: Vec<Vec<String>> =
            rows.into_iter().map(|r| r.into_iter().map(String::from).collect()).collect();
        render_table(&rows, &theme)
            .into_iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect()
    }

    #[test]
    fn empty_rows_returns_empty_vec() {
        let out = render_table(&[], &Theme::dark());
        assert!(out.is_empty());
    }

    #[test]
    fn single_header_row_still_renders_as_a_table() {
        let out = render(vec![vec!["a", "b"]]);
        // top border, header row, bottom border = 3 lines (no mid-rule
        // since there's no body).
        assert!(out[0].starts_with('┌'));
        assert!(out.last().unwrap().starts_with('└'));
    }

    #[test]
    fn header_and_body_use_shared_borders_no_double_pipes() {
        let out = render(vec![vec!["a", "bb"], vec!["1", "2"]]);
        let joined = out.join("\n");
        assert!(!joined.contains("||"), "no double pipes: {joined:?}");
        // Top/mid/bottom rules all use the connected glyphs.
        assert!(out.iter().any(|l| l.contains('┬')));
        assert!(out.iter().any(|l| l.contains('┼')));
        assert!(out.iter().any(|l| l.contains('┴')));
    }

    #[test]
    fn column_widths_autosize_to_widest_cell() {
        let out = render(vec![
            vec!["h", "header"],
            vec!["x", "y"],
        ]);
        // Both rows must be the same rendered width — a mis-sized
        // column would produce a row of different length.
        let row_lens: Vec<usize> = out.iter().map(|l| l.chars().count()).collect();
        let first = row_lens[0];
        for len in &row_lens {
            assert_eq!(*len, first, "all rows should be equal width: {out:?}");
        }
    }

    #[test]
    fn ragged_row_pads_missing_cells_as_empty() {
        // Second row is shorter than first; render should still
        // produce a rectangular grid.
        let out = render(vec![vec!["a", "b", "c"], vec!["1"]]);
        let first_len = out[0].chars().count();
        for line in &out {
            assert_eq!(line.chars().count(), first_len);
        }
    }

    #[test]
    fn handles_unicode_width_cells_without_misaligning() {
        // 'é' is 1 column wide, but we still route width via
        // UnicodeWidthStr to stay correct for CJK / emoji.
        let out = render(vec![vec!["café", "x"], vec!["1", "y"]]);
        let first_len = out[0].chars().count();
        for line in &out {
            assert_eq!(line.chars().count(), first_len);
        }
    }
}
