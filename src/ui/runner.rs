//! TUI Runner - Orchestrates terminal setup and app loop
//!
//! This module handles terminal raw mode, alternate screen setup,
//! and the main application loop.

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::fs::File;
use std::io::{self, Write};
use std::os::unix::io::AsRawFd;

use crate::app::App;
use crate::config::D3vxConfig;
use crate::pipeline::dashboard::Dashboard;

/// Options for launching the TUI
pub struct TuiOptions {
    pub verbose: bool,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub ui_mode: Option<String>,
    pub stream_out: Option<std::path::PathBuf>,
    pub config: Option<D3vxConfig>,
    pub dashboard: Option<Dashboard>,
    pub resume: bool,
}

impl std::fmt::Debug for TuiOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TuiOptions")
            .field("verbose", &self.verbose)
            .field("cwd", &self.cwd)
            .field("model", &self.model)
            .field("session_id", &self.session_id)
            .field("ui_mode", &self.ui_mode)
            .field("stream_out", &self.stream_out)
            .field("dashboard", &self.dashboard.as_ref().map(|_| "Some(Dashboard)"))
            .field("resume", &self.resume)
            .finish_non_exhaustive()
    }
}

/// Check if running in IPC mode (stdin is not a terminal)
pub fn is_ipc_mode() -> bool {
    if std::env::var("D3VX_TUI_MODE").ok().as_deref() == Some("standalone") {
        return false;
    }
    !atty::is(atty::Stream::Stdin)
}

/// Run the TUI in the appropriate mode
pub async fn run_tui(opts: TuiOptions) -> Result<()> {
    if is_ipc_mode() {
        run_ipc_mode(opts).await
    } else {
        run_standalone_mode(opts).await
    }
}

/// Run in standalone mode (direct terminal access)
async fn run_standalone_mode(opts: TuiOptions) -> Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Force standalone mode - no IPC parent process in pure Rust
    std::env::set_var("D3VX_TUI_MODE", "standalone");

    let mut app = App::new(
        opts.cwd.clone(),
        opts.model.clone(),
        opts.session_id.clone(),
        opts.stream_out.clone(),
        opts.resume,
        opts.dashboard.clone(),
    )
    .await?;
    app.apply_initial_ui_mode(opts.ui_mode.as_deref());
    let result = app.run(&mut terminal).await;

    // Clean up terminal state before exiting
    cleanup_terminal()?;
    terminal::disable_raw_mode()?;

    result
}

/// Clean up terminal state for a clean exit
fn cleanup_terminal() -> Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Clear entire screen and move cursor to home position
    execute!(handle, crossterm::cursor::Show)?;
    execute!(handle, crossterm::cursor::MoveTo(0, 0))?;
    execute!(
        handle,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
    )?;

    // Leave alternate screen properly
    execute!(handle, LeaveAlternateScreen)?;

    // Disable mouse capture
    execute!(handle, DisableMouseCapture)?;

    // Reset any styles
    execute!(handle, crossterm::style::ResetColor)?;

    handle.flush()?;

    Ok(())
}

/// Run in IPC mode (stdin is pipe, use /dev/tty)
async fn run_ipc_mode(opts: TuiOptions) -> Result<()> {
    let (mut tty_file, original_termios) = setup_tty_terminal()?;
    let tty_fd = tty_file.as_raw_fd();

    let backend = CrosstermBackend::new(&mut tty_file);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Force standalone mode - no IPC parent process in pure Rust
    std::env::set_var("D3VX_TUI_MODE", "standalone");

    let mut app = App::new(
        opts.cwd.clone(),
        opts.model.clone(),
        opts.session_id.clone(),
        opts.stream_out.clone(),
        opts.resume,
        opts.dashboard.clone(),
    )
    .await?;
    app.apply_initial_ui_mode(opts.ui_mode.as_deref());
    let result = app.run(&mut terminal).await;

    // Clean up terminal state for IPC mode
    cleanup_ipc_terminal(tty_fd)?;

    unsafe {
        libc::tcsetattr(tty_fd, libc::TCSANOW, &original_termios);
    }

    result
}

/// Clean up terminal state for IPC mode
fn cleanup_ipc_terminal(tty_fd: i32) -> io::Result<()> {
    // Terminal cleanup escape sequences:
    // \x1b[?1049l - Leave alternate screen
    // \x1b[?1006l - Disable mouse SGR mode
    // \x1b[?1002l - Disable mouse drag tracking
    // \x1b[?1000l - Disable mouse mode
    // \x1b[0m - Reset SGR attributes
    // \x1b[H - Move cursor to home
    // \x1b[2J - Clear entire screen
    let cleanup = b"\x1b[?1049l\x1b[?1006l\x1b[?1002l\x1b[?1000l\x1b[0m\x1b[H\x1b[2J";
    unsafe {
        libc::write(
            tty_fd,
            cleanup.as_ptr() as *const libc::c_void,
            cleanup.len(),
        );
    }
    Ok(())
}

/// Setup terminal for IPC mode using /dev/tty
fn setup_tty_terminal() -> Result<(File, libc::termios)> {
    let mut tty = File::options().read(true).write(true).open("/dev/tty")?;
    let fd = tty.as_raw_fd();

    let mut original_termios: libc::termios = unsafe { std::mem::zeroed() };
    unsafe {
        if libc::tcgetattr(fd, &mut original_termios) != 0 {
            return Err(anyhow::anyhow!("Failed to get terminal attributes"));
        }

        let mut raw_termios = original_termios;
        libc::cfmakeraw(&mut raw_termios);
        raw_termios.c_oflag = original_termios.c_oflag;

        if libc::tcsetattr(fd, libc::TCSANOW, &raw_termios) != 0 {
            return Err(anyhow::anyhow!("Failed to set raw mode"));
        }
    }

    tty.write_all(b"\x1b[?1049h\x1b[?1000h\x1b[?1006h")?;
    tty.flush()?;

    Ok((tty, original_termios))
}
