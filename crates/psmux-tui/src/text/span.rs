//! Ported from `ratatui-core` 0.1.2 `src/text/span.rs` (ZDEP-030), reduced
//! to `raw`/`styled`/the `style`+`content` fields.

use crate::style::Style;
use std::borrow::Cow;

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Span<'a> {
    pub style: Style,
    pub content: Cow<'a, str>,
}

impl<'a> Span<'a> {
    pub fn raw<T: Into<Cow<'a, str>>>(content: T) -> Self {
        Self { content: content.into(), style: Style::default() }
    }

    pub fn styled<T: Into<Cow<'a, str>>, S: Into<Style>>(content: T, style: S) -> Self {
        Self { content: content.into(), style: style.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;

    // Covers: ZDEP-030
    #[test]
    fn raw_has_default_style() {
        let span = Span::raw("hello");
        assert_eq!(span.content, "hello");
        assert_eq!(span.style, Style::default());
    }

    // Covers: ZDEP-030
    #[test]
    fn styled_accepts_style_or_color() {
        let span = Span::styled("hi", Style::default().fg(Color::Red));
        assert_eq!(span.style.fg, Some(Color::Red));

        let span = Span::styled("hi", Color::Blue);
        assert_eq!(span.style.fg, Some(Color::Blue));
    }
}
