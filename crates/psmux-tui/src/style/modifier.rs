//! Hand-rolled replacement for `ratatui-core` 0.1.2's `bitflags!`-generated
//! `Modifier` (`src/style.rs`, ZDEP-029) -- same `u16` bit layout, so a
//! `Modifier` value round-trips identically to upstream's. `.all()` is not
//! ported (spec: not in the reduced surface); `insert`/`remove` are kept
//! (not individually called out by the spec, but required internally by
//! `Style::add_modifier`/`remove_modifier` and `Cell::set_style`, mirroring
//! upstream's own use of `bitflags`' generated `insert`/`remove`).

use std::ops::BitOr;

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Modifier(u16);

impl Modifier {
    pub const BOLD: Self = Self(0b0000_0000_0001);
    pub const DIM: Self = Self(0b0000_0000_0010);
    pub const ITALIC: Self = Self(0b0000_0000_0100);
    pub const UNDERLINED: Self = Self(0b0000_0000_1000);
    pub const SLOW_BLINK: Self = Self(0b0000_0001_0000);
    pub const RAPID_BLINK: Self = Self(0b0000_0010_0000);
    pub const REVERSED: Self = Self(0b0000_0100_0000);
    pub const HIDDEN: Self = Self(0b0000_1000_0000);
    pub const CROSSED_OUT: Self = Self(0b0001_0000_0000);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl BitOr for Modifier {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-029
    #[test]
    fn empty_contains_nothing() {
        assert!(Modifier::empty().is_empty());
        assert!(!Modifier::empty().contains(Modifier::BOLD));
    }

    // Covers: ZDEP-029
    #[test]
    fn bitor_composes_flags() {
        let m = Modifier::BOLD | Modifier::ITALIC;
        assert!(m.contains(Modifier::BOLD));
        assert!(m.contains(Modifier::ITALIC));
        assert!(!m.contains(Modifier::DIM));
    }

    // Covers: ZDEP-029
    #[test]
    fn insert_and_remove() {
        let mut m = Modifier::empty();
        m.insert(Modifier::BOLD | Modifier::DIM);
        assert!(m.contains(Modifier::BOLD) && m.contains(Modifier::DIM));
        m.remove(Modifier::DIM);
        assert!(m.contains(Modifier::BOLD));
        assert!(!m.contains(Modifier::DIM));
    }

    // Covers: ZDEP-029
    #[test]
    fn all_nine_flags_are_distinct_bits() {
        let all = [
            Modifier::BOLD,
            Modifier::DIM,
            Modifier::ITALIC,
            Modifier::UNDERLINED,
            Modifier::SLOW_BLINK,
            Modifier::RAPID_BLINK,
            Modifier::REVERSED,
            Modifier::HIDDEN,
            Modifier::CROSSED_OUT,
        ];
        let mut union = Modifier::empty();
        let mut popcount = 0u32;
        for flag in all {
            union.insert(flag);
            popcount += 1;
        }
        assert_eq!(popcount, 9);
        for flag in all {
            assert!(union.contains(flag));
        }
    }
}
