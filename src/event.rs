//! Event Handling - Async event processing for the TUI
//!
//! Handles keyboard, mouse, resize, and tick events using crossterm.
//! Supports both direct terminal mode and IPC mode (when stdin is a pipe).

use anyhow::Result;
use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};

// ────────────────────────────────────────────────────────────
// Event Types
// ────────────────────────────────────────────────────────────

/// Internal event type
#[derive(Debug)]
pub enum Event {
    /// Key press event
    Key(KeyEvent),
    /// Mouse event
    Mouse(MouseEvent),
    /// Terminal resize event
    Resize(u16, u16),
    /// Focus gained
    FocusGained,
    /// Focus lost
    FocusLost,
    /// Pasted text (bracketed paste)
    Paste(String),
    /// Tick event (for animation)
    Tick,
    /// IPC event
    Ipc(crate::ipc::IpcEvent),
    /// Shell result event
    ShellResult {
        cmd: String,
        output: String,
        exit_code: i32,
    },
    /// Save current session to database
    SaveSession,
    /// Tool execution completed
    ToolCompleted {
        id: String,
        output: String,
        is_error: bool,
        elapsed_ms: u64,
    },
    /// Event from an agent loop
    Agent(crate::agent::AgentEvent),
    /// Event from an agent loop in a specific workspace
    AgentInWorkspace(String, crate::agent::AgentEvent),
    /// Send a message to the agent
    SendMessage(String),
    /// Trigger agent synthesis after sub-agent completes
    RunSynthesis,
    /// Spawn parallel agents event
    SpawnParallel(crate::tools::SpawnParallelEvent),
    /// Inbox message received for an agent
    InboxMessage {
        to_agent: String,
        from_agent: String,
        message: String,
    },
    /// Swarm inter-agent message routed
    SwarmRelay {
        from: String,
        to: String,
        body: String,
    },
    /// Systematic error message
    Error(String),
}

// ────────────────────────────────────────────────────────────
// TTY Handle for IPC Mode
// ────────────────────────────────────────────────────────────

/// Wrapper for /dev/tty access when stdin is a pipe
pub struct TtyHandle {
    fd: i32,
}

impl TtyHandle {
    /// Open /dev/tty for direct terminal access
    /// Uses low-level open() to ensure we get the controlling terminal
    pub fn open() -> Result<Self> {
        // Open /dev/tty with O_RDWR to get read/write access
        const O_RDWR: i32 = 2;
        let fd = unsafe { libc::open(b"/dev/tty\0".as_ptr() as *const i8, O_RDWR) };

        if fd < 0 {
            return Err(anyhow::anyhow!(
                "Failed to open /dev/tty: {}",
                io::Error::last_os_error()
            ));
        }

        info!("Opened /dev/tty with fd={}", fd);
        Ok(Self { fd })
    }

    /// Get the file descriptor for polling
    pub fn fd(&self) -> i32 {
        self.fd
    }

    /// Check if there's data available to read
    pub fn poll(&self, timeout: Duration) -> io::Result<bool> {
        let mut pfd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };

        let timeout_ms = timeout.as_millis() as i32;
        let result = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };

        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(result > 0 && (pfd.revents & libc::POLLIN) != 0)
        }
    }

    /// Read a single byte
    pub fn read_byte(&mut self) -> io::Result<u8> {
        let mut buf = [0u8; 1];
        let n = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, 1) };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else if n == 0 {
            Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF on tty"))
        } else {
            Ok(buf[0])
        }
    }
}

impl Drop for TtyHandle {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

// ────────────────────────────────────────────────────────────
// Event Handler
// ────────────────────────────────────────────────────────────

/// Event handler that polls for terminal events
pub struct EventHandler {
    sender: mpsc::Sender<Event>,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(sender: mpsc::Sender<Event>) -> Self {
        Self { sender }
    }

    /// Check if we're in IPC mode (stdin is not a terminal)
    pub fn is_ipc_mode() -> bool {
        !atty::is(atty::Stream::Stdin)
    }

    /// Spawn the event handler task
    pub fn spawn(&self) -> Result<()> {
        let sender = self.sender.clone();
        let rt = tokio::runtime::Handle::current();

        if Self::is_ipc_mode() {
            info!("Running in IPC mode, using /dev/tty for terminal I/O");
            self.spawn_ipc_mode(sender, rt)?;
        } else {
            info!("Running in standalone mode");
            self.spawn_standalone_mode(sender, rt)?;
        }

        Ok(())
    }

    /// Spawn for standalone mode (stdin is terminal)
    fn spawn_standalone_mode(
        &self,
        sender: mpsc::Sender<Event>,
        rt: tokio::runtime::Handle,
    ) -> Result<()> {
        std::thread::spawn(move || {
            // Enable mouse capture, bracketed paste, and keyboard enhancements.
            // DISAMBIGUATE_ESCAPE_CODES lets the terminal distinguish modifier
            // keys (e.g. Ctrl+Tab vs Tab) via the kitty keyboard protocol.
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::event::EnableMouseCapture,
                crossterm::event::EnableBracketedPaste,
                crossterm::event::PushKeyboardEnhancementFlags(
                    crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                )
            );

            loop {
                match crossterm::event::poll(Duration::from_millis(16)) {
                    Ok(true) => match crossterm::event::read() {
                        Ok(event) => {
                            if let Some(e) = Self::convert_event(event) {
                                let _ = rt.block_on(sender.send(e));
                            }
                        }
                        Err(e) => error!("Error reading event: {}", e),
                    },
                    Ok(false) => {}
                    Err(e) => {
                        error!("Error polling events: {}", e);
                        break;
                    }
                }
            }

            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::event::PopKeyboardEnhancementFlags,
                crossterm::event::DisableBracketedPaste,
                crossterm::event::DisableMouseCapture
            );
        });

        Ok(())
    }

    /// Spawn for IPC mode (stdin is pipe, use /dev/tty)
    fn spawn_ipc_mode(
        &self,
        sender: mpsc::Sender<Event>,
        rt: tokio::runtime::Handle,
    ) -> Result<()> {
        let tty = TtyHandle::open()?;
        let tty_fd = tty.fd();

        info!("IPC mode: opened /dev/tty for input (fd={})", tty_fd);

        // Enable mouse capture
        let mouse_enable = b"\x1b[?1000h\x1b[?1002h\x1b[?1006h";
        unsafe {
            libc::write(
                tty_fd,
                mouse_enable.as_ptr() as *const libc::c_void,
                mouse_enable.len(),
            );
        }

        std::thread::spawn(move || {
            let mut tty = tty;
            let mut escape_buffer: Vec<u8> = Vec::new();
            let mut in_escape = false;

            info!("IPC mode: starting input loop on fd={}", tty_fd);

            loop {
                match tty.poll(Duration::from_millis(16)) {
                    Ok(true) => {
                        match tty.read_byte() {
                            Ok(byte) => {
                                info!(
                                    "Received byte on fd={}: 0x{:02x} ('{}')",
                                    tty_fd,
                                    byte,
                                    if byte >= 32 && byte < 127 {
                                        byte as char
                                    } else {
                                        '?'
                                    }
                                );
                                // Parse escape sequences for special keys
                                if byte == 0x1b {
                                    in_escape = true;
                                    escape_buffer.clear();
                                    escape_buffer.push(byte);
                                } else if in_escape {
                                    escape_buffer.push(byte);
                                    // Check if we have a complete escape sequence
                                    if let Some(event) = Self::parse_escape_sequence(&escape_buffer)
                                    {
                                        info!("Parsed escape sequence: {:?}", event);
                                        let _ = rt.block_on(sender.send(event));
                                        in_escape = false;
                                        escape_buffer.clear();
                                    } else if escape_buffer.len() > 8 {
                                        // Give up on incomplete sequence
                                        in_escape = false;
                                        escape_buffer.clear();
                                    }
                                } else {
                                    // Regular character
                                    if let Some(event) = Self::byte_to_key_event(byte) {
                                        info!("Sending key event: {:?}", event);
                                        let _ = rt.block_on(sender.send(event));
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Error reading from tty: {}", e);
                                break;
                            }
                        }
                    }
                    Ok(false) => {}
                    Err(e) => {
                        error!("Error polling tty: {}", e);
                        break;
                    }
                }
            }

            // Disable mouse capture
            let mouse_disable = b"\x1b[?1006l\x1b[?1002l\x1b[?1000l";
            unsafe {
                libc::write(
                    tty_fd,
                    mouse_disable.as_ptr() as *const libc::c_void,
                    mouse_disable.len(),
                );
            }
        });

        Ok(())
    }

    /// Convert crossterm event to internal event
    fn convert_event(event: CrosstermEvent) -> Option<Event> {
        match event {
            CrosstermEvent::Key(key) => Some(Event::Key(key)),
            CrosstermEvent::Mouse(mouse) => Some(Event::Mouse(mouse)),
            CrosstermEvent::Resize(w, h) => Some(Event::Resize(w, h)),
            CrosstermEvent::FocusGained => Some(Event::FocusGained),
            CrosstermEvent::FocusLost => Some(Event::FocusLost),
            CrosstermEvent::Paste(text) => Some(Event::Paste(text)),
            // Future crossterm event variants are safely ignored
        }
    }

    /// Convert a single byte to a key event
    fn byte_to_key_event(byte: u8) -> Option<Event> {
        use crossterm::event::{KeyCode, KeyModifiers};

        let char_code = byte as char;
        let modifiers = KeyModifiers::empty();

        let key_code = match byte {
            0x0d | 0x0a => KeyCode::Enter,
            0x09 => KeyCode::Tab,
            0x08 | 0x7f => KeyCode::Backspace,
            0x1b => KeyCode::Esc,
            // Ctrl-A through Ctrl-Z
            0x01..=0x1a => {
                return Some(Event::Key(KeyEvent::new(
                    KeyCode::Char((byte - 1 + b'a') as char),
                    KeyModifiers::CONTROL,
                )));
            }
            _ if char_code.is_ascii_graphic() || char_code.is_ascii_whitespace() => {
                KeyCode::Char(char_code)
            }
            _ => return None,
        };

        Some(Event::Key(KeyEvent::new(key_code, modifiers)))
    }

    /// Parse escape sequences for special keys
    fn parse_escape_sequence(bytes: &[u8]) -> Option<Event> {
        use crossterm::event::{KeyCode, KeyModifiers};

        if bytes.len() < 2 || bytes[0] != 0x1b {
            return None;
        }

        let seq = std::str::from_utf8(&bytes[1..]).ok()?;

        // Common escape sequences (CSI sequences start with [)
        let (key_code, modifiers) = match seq {
            // Arrow keys (CSI sequences: ESC [ A/B/C/D)
            "[A" => (KeyCode::Up, KeyModifiers::empty()),
            "[B" => (KeyCode::Down, KeyModifiers::empty()),
            "[C" => (KeyCode::Right, KeyModifiers::empty()),
            "[D" => (KeyCode::Left, KeyModifiers::empty()),
            // Home/End
            "[H" => (KeyCode::Home, KeyModifiers::empty()),
            "[F" => (KeyCode::End, KeyModifiers::empty()),
            // Function keys (VT100 style)
            "OP" => (KeyCode::F(1), KeyModifiers::empty()),
            "OQ" => (KeyCode::F(2), KeyModifiers::empty()),
            "OR" => (KeyCode::F(3), KeyModifiers::empty()),
            "OS" => (KeyCode::F(4), KeyModifiers::empty()),
            // Function keys (CSI style)
            "[11~" => (KeyCode::F(1), KeyModifiers::empty()),
            "[12~" => (KeyCode::F(2), KeyModifiers::empty()),
            "[13~" => (KeyCode::F(3), KeyModifiers::empty()),
            "[14~" => (KeyCode::F(4), KeyModifiers::empty()),
            "[15~" => (KeyCode::F(5), KeyModifiers::empty()),
            "[17~" => (KeyCode::F(6), KeyModifiers::empty()),
            "[18~" => (KeyCode::F(7), KeyModifiers::empty()),
            "[19~" => (KeyCode::F(8), KeyModifiers::empty()),
            "[20~" => (KeyCode::F(9), KeyModifiers::empty()),
            "[21~" => (KeyCode::F(10), KeyModifiers::empty()),
            // Page Up/Down
            "[5~" => (KeyCode::PageUp, KeyModifiers::empty()),
            "[6~" => (KeyCode::PageDown, KeyModifiers::empty()),
            // Delete
            "[3~" => (KeyCode::Delete, KeyModifiers::empty()),
            // Shift+Arrow keys
            "[1;2A" => (KeyCode::Up, KeyModifiers::SHIFT),
            "[1;2B" => (KeyCode::Down, KeyModifiers::SHIFT),
            "[1;2C" => (KeyCode::Right, KeyModifiers::SHIFT),
            "[1;2D" => (KeyCode::Left, KeyModifiers::SHIFT),
            // Alt+Arrow keys
            "[1;3A" => (KeyCode::Up, KeyModifiers::ALT),
            "[1;3B" => (KeyCode::Down, KeyModifiers::ALT),
            "[1;3C" => (KeyCode::Right, KeyModifiers::ALT),
            "[1;3D" => (KeyCode::Left, KeyModifiers::ALT),
            // More escape sequences can be added
            _ => return None,
        };

        Some(Event::Key(KeyEvent::new(key_code, modifiers)))
    }
}

// ────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────

/// Check if a key event is a press (not release)
pub fn is_key_press(event: &KeyEvent) -> bool {
    event.kind == crossterm::event::KeyEventKind::Press
}
