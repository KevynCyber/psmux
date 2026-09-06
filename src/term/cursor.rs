//! Cursor ANSI commands (ZDEP-025), shape-identical to crossterm::cursor.

use super::Command;

pub struct EnableBlinking;
impl Command for EnableBlinking {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[?12h")
    }
}

pub struct DisableBlinking;
impl Command for DisableBlinking {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[?12l")
    }
}

pub struct Hide;
impl Command for Hide {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[?25l")
    }
}

pub struct Show;
impl Command for Show {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[?25h")
    }
}

/// 0-indexed (column, row); emits the 1-indexed CUP sequence.
pub struct MoveTo(pub u16, pub u16);
impl Command for MoveTo {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(f, "\x1b[{};{}H", self.1 + 1, self.0 + 1)
    }
}
