//! Text-output ANSI command (ZDEP-025), shape-identical to crossterm::style.

use super::Command;
use std::fmt::Display;

pub struct Print<T: Display>(pub T);
impl<T: Display> Command for Print<T> {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
