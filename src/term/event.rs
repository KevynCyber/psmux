//! Event types shape-identical to crossterm 0.29's `event` module (ZDEP-025),
//! plus the mouse-capture / bracketed-paste ANSI commands and the
//! poll/read entry points backed by `super::console` on Windows.

use super::Command;
use std::io;
use std::time::Duration;

// ─── KeyModifiers ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(0b0000_0001);
    pub const CONTROL: Self = Self(0b0000_0010);
    pub const ALT: Self = Self(0b0000_0100);
    pub const SUPER: Self = Self(0b0000_1000);
    pub const HYPER: Self = Self(0b0001_0000);
    pub const META: Self = Self(0b0010_0000);

    pub const fn empty() -> Self { Self(0) }
    pub const fn all() -> Self {
        Self(Self::SHIFT.0 | Self::CONTROL.0 | Self::ALT.0 | Self::SUPER.0 | Self::HYPER.0 | Self::META.0)
    }
    pub const fn bits(self) -> u8 { self.0 }
    pub const fn from_bits_truncate(bits: u8) -> Self { Self(bits & Self::all().0) }
    pub const fn is_empty(self) -> bool { self.0 == 0 }
    pub const fn contains(self, other: Self) -> bool { self.0 & other.0 == other.0 }
    pub const fn intersects(self, other: Self) -> bool { self.0 & other.0 != 0 }
    pub fn insert(&mut self, other: Self) { self.0 |= other.0; }
    pub fn remove(&mut self, other: Self) { self.0 &= !other.0; }
    pub const fn difference(self, other: Self) -> Self { Self(self.0 & !other.0) }
    pub const fn union(self, other: Self) -> Self { Self(self.0 | other.0) }
}

impl std::ops::BitOr for KeyModifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
}
impl std::ops::BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}
impl std::ops::BitAnd for KeyModifiers {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self { Self(self.0 & rhs.0) }
}
impl std::ops::Sub for KeyModifiers {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self { self.difference(rhs) }
}
impl std::ops::Not for KeyModifiers {
    type Output = Self;
    fn not(self) -> Self { Self(!self.0 & Self::all().0) }
}

// ─── KeyEventState ────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct KeyEventState(u8);

impl KeyEventState {
    pub const NONE: Self = Self(0);
    pub const KEYPAD: Self = Self(0b0000_0001);
    pub const CAPS_LOCK: Self = Self(0b0000_1000);
    pub const NUM_LOCK: Self = Self(0b0001_0000);

    pub const fn empty() -> Self { Self(0) }
    pub const fn bits(self) -> u8 { self.0 }
    pub const fn is_empty(self) -> bool { self.0 == 0 }
    pub fn insert(&mut self, other: Self) { self.0 |= other.0; }
}

// ─── Key / Mouse / Event ──────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Backspace, Enter, Left, Right, Up, Down, Home, End, PageUp, PageDown,
    Tab, BackTab, Delete, Insert, F(u8), Char(char), Null, Esc,
    CapsLock, ScrollLock, NumLock, PrintScreen, Pause, Menu, KeypadBegin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyEventKind { Press, Repeat, Release }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub kind: KeyEventKind,
    pub state: KeyEventState,
}

impl KeyEvent {
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::NONE }
    }
    pub const fn new_with_kind(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> Self {
        Self { code, modifiers, kind, state: KeyEventState::NONE }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton { Left, Right, Middle }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseEventKind {
    Down(MouseButton), Up(MouseButton), Drag(MouseButton),
    Moved, ScrollDown, ScrollUp, ScrollLeft, ScrollRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub column: u16,
    pub row: u16,
    pub modifiers: KeyModifiers,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    FocusGained,
    FocusLost,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste(String),
    Resize(u16, u16),
}

// ─── Mouse capture / bracketed paste commands ────────────────────────────

/// Enables SGR mouse reporting (button, drag, and wheel modes) via DECSET,
/// and -- on Windows -- flips the native console mode so the OS itself
/// starts forwarding mouse records instead of doing quick-edit selection.
/// crossterm's Windows path only does the console-mode flip (no DECSET);
/// psmux is always-ANSI (VT processing is already enabled), so this writes
/// both.
pub struct EnableMouseCapture;
impl Command for EnableMouseCapture {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        #[cfg(windows)]
        {
            let _ = super::console::enable_mouse_capture_native();
        }
        f.write_str("\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1015h\x1b[?1006h")
    }
}

pub struct DisableMouseCapture;
impl Command for DisableMouseCapture {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        #[cfg(windows)]
        {
            let _ = super::console::disable_mouse_capture_native();
        }
        f.write_str("\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l")
    }
}

pub struct EnableBracketedPaste;
impl Command for EnableBracketedPaste {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[?2004h")
    }
}

pub struct DisableBracketedPaste;
impl Command for DisableBracketedPaste {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[?2004l")
    }
}

// ─── poll / read ──────────────────────────────────────────────────────────

#[cfg(windows)]
pub fn poll(timeout: Duration) -> io::Result<bool> {
    super::console::poll(timeout)
}

#[cfg(not(windows))]
pub fn poll(_timeout: Duration) -> io::Result<bool> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "psmux is Windows-only"))
}

#[cfg(windows)]
pub fn read() -> io::Result<Event> {
    super::console::read()
}

#[cfg(not(windows))]
pub fn read() -> io::Result<Event> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "psmux is Windows-only"))
}
