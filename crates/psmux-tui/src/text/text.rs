//! Ported from `ratatui-core` 0.1.2 `src/text/text.rs` (ZDEP-030, extended
//! ZDEP-035 for `Paragraph::new`'s `Into<Text>` bound), reduced to the
//! `From` conversions this repo's inventory found in use: `&str`, `String`,
//! `Line`, `Vec<Line>`. Upstream's `Text::from(&str)` splits on `\n` via
//! `str::lines()`; this port (S8a, ZDEP-030) does not, so a multi-line
//! `&str`/`String` passed to `Paragraph::new` renders as one line instead
//! of upstream's per-line split -- flagged as a ZDEP-035 risk since no call
//! site in the current inventory does this (all multi-line paragraph
//! content already arrives as `Vec<Line>`), but S8c must re-check before
//! flipping any call site that builds a `Paragraph` from a literal
//! containing `\n`.

use super::{Line, Span};
use crate::layout::Alignment;
use crate::style::Style;

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Text<'a> {
    pub alignment: Option<Alignment>,
    pub style: Style,
    pub lines: Vec<Line<'a>>,
}

impl<'a> From<&'a str> for Text<'a> {
    fn from(s: &'a str) -> Self {
        Self { lines: vec![Line::from(s)], ..Default::default() }
    }
}

impl From<String> for Text<'_> {
    fn from(s: String) -> Self {
        Self { lines: vec![Line::from(s)], ..Default::default() }
    }
}

/// Ported from `ratatui-core` 0.1.2 `src/text/text.rs` `From<Span> for
/// Text` -- needed by `Paragraph::new(Span::styled(...))` call sites (e.g.
/// `src/client.rs:5472`).
impl<'a> From<Span<'a>> for Text<'a> {
    fn from(span: Span<'a>) -> Self {
        Self { lines: vec![Line::from(span)], ..Default::default() }
    }
}

impl<'a> From<Line<'a>> for Text<'a> {
    fn from(line: Line<'a>) -> Self {
        Self { lines: vec![line], ..Default::default() }
    }
}

impl<'a> From<Vec<Line<'a>>> for Text<'a> {
    fn from(lines: Vec<Line<'a>>) -> Self {
        Self { lines, ..Default::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-030
    #[test]
    fn from_str() {
        let text = Text::from("hello");
        assert_eq!(text.lines, vec![Line::from("hello")]);
    }

    // Covers: ZDEP-030
    #[test]
    fn from_vec_of_lines() {
        let lines = vec![Line::from("a"), Line::from("b")];
        let text = Text::from(lines.clone());
        assert_eq!(text.lines, lines);
    }

    // Covers: ZDEP-035
    #[test]
    fn from_string() {
        let text = Text::from(String::from("hello"));
        assert_eq!(text.lines, vec![Line::from("hello")]);
    }

    // Covers: ZDEP-035
    #[test]
    fn from_line() {
        let line = Line::from("hello");
        let text = Text::from(line.clone());
        assert_eq!(text.lines, vec![line]);
    }
}
