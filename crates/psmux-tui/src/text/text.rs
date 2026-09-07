//! Ported from `ratatui-core` 0.1.2 `src/text/text.rs` (ZDEP-030), reduced
//! to the `From` conversions the spec scopes in: `Vec<Line>`, `&str`.

use super::Line;
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
}
