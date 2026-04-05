//! Embedded static assets for the dashboard HTML page.

/// Returns the single-page dashboard HTML with inline CSS and vanilla JS.
///
/// The HTML is embedded at compile time via `include_str!`.
pub fn dashboard_html() -> &'static str {
    include_str!("dashboard.html")
}
