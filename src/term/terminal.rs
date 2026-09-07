//! Raw mode, alternate screen, and terminal-size ANSI/Win32 commands
//! (ZDEP-025/026), shape-identical to crossterm::terminal.

use super::Command;
use std::io;

pub struct EnterAlternateScreen;
impl Command for EnterAlternateScreen {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[?1049h")
    }
}

pub struct LeaveAlternateScreen;
impl Command for LeaveAlternateScreen {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[?1049l")
    }
}

#[cfg(windows)]
pub fn enable_raw_mode() -> io::Result<()> {
    super::console::enable_raw_mode()
}
#[cfg(not(windows))]
pub fn enable_raw_mode() -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "psmux is Windows-only"))
}

#[cfg(windows)]
pub fn disable_raw_mode() -> io::Result<()> {
    super::console::disable_raw_mode()
}
#[cfg(not(windows))]
pub fn disable_raw_mode() -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "psmux is Windows-only"))
}

#[cfg(windows)]
pub fn size() -> io::Result<(u16, u16)> {
    super::console::size()
}
#[cfg(not(windows))]
pub fn size() -> io::Result<(u16, u16)> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "psmux is Windows-only"))
}

/// Window pixel size is not implemented for the Windows console API
/// (matches crossterm's own `window_size()` on Windows).
pub fn window_size() -> io::Result<psmux_tui::backend::WindowSize> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "window pixel size not implemented",
    ))
}
