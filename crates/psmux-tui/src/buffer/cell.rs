//! Ported from `ratatui-core` 0.1.2 `src/buffer/cell.rs` (ZDEP-031),
//! reduced to `symbol`/`set_symbol`/`set_char`/`set_style`/`style`/the
//! public `fg`/`bg`/`modifier`/`underline_color` fields. `EMPTY`/`new`/
//! `reset`/`skip`/`CellDiffOption`/`merge_symbol` are not ported.
//!
//! Deviates from upstream: the symbol is a plain `String` (empty string ==
//! "unset", same as upstream's `None`) instead of
//! `Option<compact_str::CompactString>` -- see `buffer/mod.rs` for why.

use crate::style::{Color, Modifier, Style};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Cell {
    symbol: String,
    pub fg: Color,
    pub bg: Color,
    #[cfg(feature = "underline-color")]
    pub underline_color: Color,
    pub modifier: Modifier,
}

impl Cell {
    /// Returns the cell's symbol, or a single space if unset.
    pub fn symbol(&self) -> &str {
        if self.symbol.is_empty() {
            " "
        } else {
            &self.symbol
        }
    }

    pub fn set_symbol(&mut self, symbol: &str) -> &mut Self {
        self.symbol = symbol.to_string();
        self
    }

    pub fn set_char(&mut self, ch: char) -> &mut Self {
        self.symbol = ch.to_string();
        self
    }

    /// Applies `style`'s `Some` colors and folds its `add_modifier`/
    /// `sub_modifier` into this cell's own modifier set, same accounting as
    /// upstream.
    pub fn set_style<S: Into<Style>>(&mut self, style: S) -> &mut Self {
        let style = style.into();
        if let Some(c) = style.fg {
            self.fg = c;
        }
        if let Some(c) = style.bg {
            self.bg = c;
        }
        #[cfg(feature = "underline-color")]
        if let Some(c) = style.underline_color {
            self.underline_color = c;
        }
        self.modifier.insert(style.add_modifier);
        self.modifier.remove(style.sub_modifier);
        self
    }

    pub fn style(&self) -> Style {
        Style {
            fg: Some(self.fg),
            bg: Some(self.bg),
            #[cfg(feature = "underline-color")]
            underline_color: Some(self.underline_color),
            add_modifier: self.modifier,
            sub_modifier: Modifier::empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-031
    #[test]
    fn default_symbol_is_a_space() {
        assert_eq!(Cell::default().symbol(), " ");
    }

    // Covers: ZDEP-031
    #[test]
    fn set_symbol_and_set_char() {
        let mut cell = Cell::default();
        cell.set_symbol("x");
        assert_eq!(cell.symbol(), "x");
        cell.set_char('y');
        assert_eq!(cell.symbol(), "y");
    }

    // Covers: ZDEP-031
    #[test]
    fn set_style_applies_colors_and_modifier() {
        let mut cell = Cell::default();
        cell.set_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
        assert_eq!(cell.fg, Color::Red);
        assert!(cell.modifier.contains(Modifier::BOLD));
    }

    // Covers: ZDEP-031
    #[test]
    fn style_round_trips_through_set_style() {
        let mut cell = Cell::default();
        let style = Style::default().fg(Color::Green).bg(Color::Blue).add_modifier(Modifier::ITALIC);
        cell.set_style(style);
        assert_eq!(cell.style().fg, Some(Color::Green));
        assert_eq!(cell.style().bg, Some(Color::Blue));
        assert!(cell.style().add_modifier.contains(Modifier::ITALIC));
    }

    // Covers: ZDEP-031
    #[test]
    fn set_style_sub_modifier_removes_flag() {
        let mut cell = Cell::default();
        cell.set_style(Style::default().add_modifier(Modifier::BOLD));
        cell.set_style(Style::default().remove_modifier(Modifier::BOLD));
        assert!(!cell.modifier.contains(Modifier::BOLD));
    }
}
