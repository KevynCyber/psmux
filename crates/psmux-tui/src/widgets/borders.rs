//! Ported from `ratatui-widgets` 0.3.2 `src/borders.rs` (ZDEP-034), reduced
//! to what this repo's inventory found in use: `Borders::ALL` only (no
//! other flag, no `|` composition, no `border!` macro -- every
//! `.borders(...)` call site in this repo passes `Borders::ALL`) and the 4
//! `BorderType`s Plain/Rounded/Double/Thick (no dashed/quadrant variants,
//! no `Display`/`EnumString` derives since nothing parses or prints one).

use crate::symbols::border;

/// Reduced to a single named constant (no bitflag composition is exercised
/// anywhere in this repo) rather than porting the `bitflags!`-generated
/// type.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Borders(u8);

impl Borders {
    const TOP: u8 = 0b0001;
    const RIGHT: u8 = 0b0010;
    const BOTTOM: u8 = 0b0100;
    const LEFT: u8 = 0b1000;

    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(Self::TOP | Self::RIGHT | Self::BOTTOM | Self::LEFT);

    pub(crate) const fn has_top(self) -> bool {
        self.0 & Self::TOP != 0
    }

    pub(crate) const fn has_right(self) -> bool {
        self.0 & Self::RIGHT != 0
    }

    pub(crate) const fn has_bottom(self) -> bool {
        self.0 & Self::BOTTOM != 0
    }

    pub(crate) const fn has_left(self) -> bool {
        self.0 & Self::LEFT != 0
    }
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BorderType {
    #[default]
    Plain,
    Rounded,
    Double,
    Thick,
}

impl BorderType {
    pub(crate) const fn to_border_set(self) -> border::Set {
        match self {
            Self::Plain => border::PLAIN,
            Self::Rounded => border::ROUNDED,
            Self::Double => border::DOUBLE,
            Self::Thick => border::THICK,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-034
    #[test]
    fn none_has_no_sides() {
        let b = Borders::NONE;
        assert!(!b.has_top() && !b.has_right() && !b.has_bottom() && !b.has_left());
    }

    // Covers: ZDEP-034
    #[test]
    fn all_has_every_side() {
        let b = Borders::ALL;
        assert!(b.has_top() && b.has_right() && b.has_bottom() && b.has_left());
    }

    // Covers: ZDEP-034
    #[test]
    fn default_is_none() {
        assert_eq!(Borders::default(), Borders::NONE);
    }

    // Covers: ZDEP-034
    #[test]
    fn border_type_default_is_plain() {
        assert_eq!(BorderType::default(), BorderType::Plain);
    }

    // Covers: ZDEP-034
    #[test]
    fn each_border_type_maps_to_its_own_set() {
        assert_eq!(BorderType::Plain.to_border_set().horizontal, border::PLAIN.horizontal);
        assert_eq!(BorderType::Rounded.to_border_set().top_left, border::ROUNDED.top_left);
        assert_eq!(BorderType::Double.to_border_set().vertical, border::DOUBLE.vertical);
        assert_eq!(BorderType::Thick.to_border_set().bottom_right, border::THICK.bottom_right);
    }
}
