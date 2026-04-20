//! Cross-platform URL opener.
//!
//! Shells out to the OS's default URL handler so a slash command like
//! `/dashboard` can actually pop the browser for the user. We inline
//! the three per-platform invocations rather than depend on the
//! `open` crate — the whole module is <40 lines of substance, gives
//! us auditable control over what we spawn, and avoids another MSRV
//! variable in the dependency tree.
//!
//! The spawn is intentionally *detached*: we don't wait on the child.
//! On macOS/Linux the opener returns immediately; on Windows `start`
//! is a shell built-in that also returns immediately. Blocking would
//! stall the TUI render loop for the lifetime of the browser tab.

use std::io;
use std::process::{Command, Stdio};

/// Attempt to open `url` in the user's default browser. Returns
/// `Ok(())` if the opener was successfully spawned — note that this
/// does *not* confirm the browser actually handled it; the OS-level
/// handler chain owns that outcome.
pub fn open_url(url: &str) -> io::Result<()> {
    let (program, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        // `cmd /C start "" <url>` — the empty title is required because
        // `start` treats a single quoted argument as the window title.
        ("cmd", vec!["/C", "start", "", url])
    } else {
        // Every mainstream Linux / *BSD desktop ships xdg-open.
        ("xdg-open", vec![url])
    };

    Command::new(program)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nothing_at_the_api_layer() {
        // The opener itself does not validate URLs — it hands any
        // string to the OS. This test documents that contract so a
        // future refactor adding validation is a conscious change,
        // not an accident.
        //
        // We do NOT actually call `open_url` in tests because it
        // would pop a browser on the developer's machine. The
        // compile-only assertion below ensures the signature stays
        // stable without side effects.
        fn _signature_check(s: &str) -> io::Result<()> {
            open_url(s)
        }
        let _ = _signature_check;
    }
}
