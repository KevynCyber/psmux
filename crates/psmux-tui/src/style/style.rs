//! Ported from `ratatui-core` 0.1.2 `src/style.rs` `Style` (ZDEP-029).
//! `add_modifier`/`remove_modifier`/`patch` keep upstream's exact accounting
//! between `add_modifier`/`sub_modifier`.

use super::{Color, Modifier};

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    #[cfg(feature = "underline-color")]
    pub underline_color: Option<Color>,
    pub add_modifier: Modifier,
    pub sub_modifier: Modifier,
}

impl Style {
    pub const fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    pub const fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    #[cfg(feature = "underline-color")]
    pub const fn underline_color(mut self, color: Color) -> Self {
        self.underline_color = Some(color);
        self
    }

    /// Adds `modifier`, clearing it from `sub_modifier` (mirrors upstream:
    /// a style can't simultaneously add and subtract the same flag).
    pub fn add_modifier(mut self, modifier: Modifier) -> Self {
        self.sub_modifier = self.sub_modifier.difference(modifier);
        self.add_modifier = self.add_modifier.union(modifier);
        self
    }

    /// Removes `modifier` from `add_modifier` and records it in
    /// `sub_modifier`.
    pub fn remove_modifier(mut self, modifier: Modifier) -> Self {
        self.add_modifier = self.add_modifier.difference(modifier);
        self.sub_modifier = self.sub_modifier.union(modifier);
        self
    }

    /// Ported from `ratatui-core` 0.1.2 `src/style.rs` `Style::patch`:
    /// layers `other` onto `self` field-by-field, so a field `other` never
    /// sets (`fg`/`bg` still `None`, or a modifier bit `other` never
    /// touches) leaves `self`'s existing value in place, rather than
    /// overwriting wholesale. `Into<Style>` (not a bare `Style` param)
    /// matches upstream's signature so callers can pass e.g. a `Color`
    /// directly. Needed at `src/client.rs:5500,5546,5585`.
    #[must_use = "`patch` returns the modified style without modifying the original"]
    pub fn patch<S: Into<Self>>(mut self, other: S) -> Self {
        let other = other.into();
        self.fg = other.fg.or(self.fg);
        self.bg = other.bg.or(self.bg);

        #[cfg(feature = "underline-color")]
        {
            self.underline_color = other.underline_color.or(self.underline_color);
        }

        self.add_modifier.remove(other.sub_modifier);
        self.add_modifier.insert(other.add_modifier);
        self.sub_modifier.remove(other.add_modifier);
        self.sub_modifier.insert(other.sub_modifier);

        self
    }
}

/// Not upstream's own impl location (upstream generates this via its
/// `color!` macro alongside `Stylize`, which this port drops), but the same
/// resulting conversion: lets call sites pass a bare `Color` anywhere
/// `Into<Style>` is accepted (e.g. `Span::styled(text, Color::Red)`).
impl From<Color> for Style {
    fn from(color: Color) -> Self {
        Style::default().fg(color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-029
    #[test]
    fn default_has_no_colors_or_modifiers() {
        let style = Style::default();
        assert_eq!(style.fg, None);
        assert_eq!(style.bg, None);
        assert!(style.add_modifier.is_empty());
        assert!(style.sub_modifier.is_empty());
    }

    // Covers: ZDEP-029
    #[test]
    fn fg_and_bg_set_fields() {
        let style = Style::default().fg(Color::Red).bg(Color::Blue);
        assert_eq!(style.fg, Some(Color::Red));
        assert_eq!(style.bg, Some(Color::Blue));
    }

    #[cfg(feature = "underline-color")]
    // Covers: ZDEP-029
    #[test]
    fn underline_color_sets_field() {
        let style = Style::default().underline_color(Color::Green);
        assert_eq!(style.underline_color, Some(Color::Green));
    }

    // Covers: ZDEP-029
    #[test]
    fn add_modifier_clears_the_flag_from_sub_modifier() {
        // add_modifier(X) always clears X from sub_modifier, even if a prior
        // remove_modifier(X) had set it -- matches upstream's own
        // last-write-wins accounting between the two fields.
        let style = Style::default().remove_modifier(Modifier::BOLD).add_modifier(Modifier::BOLD);
        assert_eq!(style.add_modifier, Modifier::BOLD);
        assert!(!style.sub_modifier.contains(Modifier::BOLD));
    }

    // Covers: ZDEP-029
    #[test]
    fn remove_modifier_records_sub_modifier() {
        let style = Style::default()
            .add_modifier(Modifier::BOLD | Modifier::ITALIC)
            .remove_modifier(Modifier::ITALIC);
        assert_eq!(style.add_modifier, Modifier::BOLD);
        assert_eq!(style.sub_modifier, Modifier::ITALIC);
    }

    // Covers: ZDEP-029
    #[test]
    fn equal_styles_compare_equal() {
        assert_eq!(
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        );
    }
}
