//! Ported from `ratatui-core` 0.1.2 `src/text/line.rs` (ZDEP-030), reduced
//! to the `From` conversions the spec scopes in: `Vec<Span>`, `Span`,
//! `&str`, `String`.

use super::Span;
use crate::layout::Alignment;
use crate::style::Style;

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Line<'a> {
    pub style: Style,
    pub alignment: Option<Alignment>,
    pub spans: Vec<Span<'a>>,
}

impl From<String> for Line<'_> {
    fn from(s: String) -> Self {
        Self { spans: vec![Span::raw(s)], ..Default::default() }
    }
}

impl<'a> From<&'a str> for Line<'a> {
    fn from(s: &'a str) -> Self {
        Self { spans: vec![Span::raw(s)], ..Default::default() }
    }
}

impl<'a> From<Vec<Span<'a>>> for Line<'a> {
    fn from(spans: Vec<Span<'a>>) -> Self {
        Self { spans, ..Default::default() }
    }
}

impl<'a> From<Span<'a>> for Line<'a> {
    fn from(span: Span<'a>) -> Self {
        Self::from(vec![span])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-030
    #[test]
    fn from_str() {
        let line = Line::from("hello");
        assert_eq!(line.spans, vec![Span::raw("hello")]);
    }

    // Covers: ZDEP-030
    #[test]
    fn from_string() {
        let line = Line::from(String::from("hello"));
        assert_eq!(line.spans, vec![Span::raw("hello")]);
    }

    // Covers: ZDEP-030
    #[test]
    fn from_span() {
        let span = Span::styled("hi", crate::style::Color::Red);
        let line = Line::from(span.clone());
        assert_eq!(line.spans, vec![span]);
    }

    // Covers: ZDEP-030
    #[test]
    fn from_vec_of_spans() {
        let spans = vec![Span::raw("a"), Span::raw("b")];
        let line = Line::from(spans.clone());
        assert_eq!(line.spans, spans);
    }
}
