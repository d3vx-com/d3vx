use include_dir::{include_dir, Dir};
use std::path::Path;

/// The embedded React SPA build directory.
///
/// Built with Vite + React, the output lives in `dashboard/web/dist/` and
/// is embedded into the binary at compile time.
static STATIC_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/pipeline/dashboard/web/dist");

/// Return a path that `tower_http::ServeDir` can serve from.
///
/// Because `include_dir!` produces an in-memory tree (no real filesystem
/// path), we write the contents to a temporary directory on first use and
/// return that. The temp dir is created once via `std::sync::OnceLock`.
pub fn static_dir() -> &'static str {
    use std::sync::OnceLock;

    static DIR_PATH: OnceLock<String> = OnceLock::new();

    DIR_PATH.get_or_init(|| {
        // We extract the embedded files to a temp dir so that ServeDir
        // can read them via the real filesystem.
        let tmp = std::env::temp_dir().join(format!("d3vx-dashboard-static-{}", std::process::id()));
        if !tmp.join("index.html").exists() {
            let _ = std::fs::create_dir_all(&tmp);
            extract_dir(&STATIC_DIR, &tmp);
        }
        tmp.to_string_lossy().into_owned()
    })
}

/// Serve a specific static file by path.
pub fn serve_static_file(path: &str) -> Option<&'static [u8]> {
    STATIC_DIR.get_file(path).map(|f| f.contents())
}

/// Serve the root index.html.
pub fn serve_static() -> Option<&'static [u8]> {
    serve_static_file("index.html")
}

/// Recursively extract files from the embedded Dir to a real path.
fn extract_dir(dir: &'static Dir<'static>, target: &Path) {
    for entry in dir.entries() {
        let child = target.join(entry.path());
        if let Some(f) = entry.as_file() {
            if let Some(parent) = child.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&child, f.contents());
        } else if let Some(d) = entry.as_dir() {
            extract_dir(d, &child);
        }
    }
}
