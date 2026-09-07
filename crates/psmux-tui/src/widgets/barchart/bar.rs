//! Ported from `ratatui-widgets` 0.3.2 `src/barchart/bar.rs` (ZDEP-039),
//! reduced to `.value`/`.label(Line)`/`.style`/`.value_style`/`.text_value`
//! -- `Bar::new`/`with_label` are never called anywhere in this repo's
//! inventory (the one call site always starts from `Bar::default()`), and
//! `.label` here takes a `Line` directly rather than upstream's
//! `Into<Line>` (the one call site always passes `Line::from(label)`).

use crate::style::Style;
use crate::text::Line;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Bar<'a> {
    pub(super) value: u64,
    pub(super) label: Option<Line<'a>>,
    pub(super) style: Style,
    pub(super) value_style: Style,
    pub(super) text_value: Option<String>,
}

impl<'a> Bar<'a> {
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn value(mut self, value: u64) -> Self {
        self.value = value;
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn label(mut self, label: Line<'a>) -> Self {
        self.label = Some(label);
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn value_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.value_style = style.into();
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn text_value<T: Into<String>>(mut self, text_value: T) -> Self {
        self.text_value = Some(text_value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-039
    #[test]
    fn builder_sets_all_fields() {
        let bar = Bar::default().value(42).label(Line::from("l")).text_value("v");
        assert_eq!(bar.value, 42);
        assert_eq!(bar.label, Some(Line::from("l")));
        assert_eq!(bar.text_value, Some("v".to_string()));
    }
}
