//! Ported from `ratatui-core` 0.1.2 `src/symbols/border.rs` (ZDEP-033),
//! reduced to the 4 `BorderType`s this repo's inventory found in use
//! (Plain/Rounded/Double/Thick) and the 6 glyphs `Block`'s straight-edge
//! rendering needs (corners + one horizontal + one vertical each) -- no
//! `vertical_left/right`/`horizontal_top/bottom` split (that split exists
//! upstream only so borders can differ per edge via `border_set`, which
//! this repo never calls), no dashed/quadrant/McGugan variants (unused).

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Set {
    pub top_left: &'static str,
    pub top_right: &'static str,
    pub bottom_left: &'static str,
    pub bottom_right: &'static str,
    pub vertical: &'static str,
    pub horizontal: &'static str,
}

pub const PLAIN: Set = Set {
    top_left: "\u{250C}",
    top_right: "\u{2510}",
    bottom_left: "\u{2514}",
    bottom_right: "\u{2518}",
    vertical: "\u{2502}",
    horizontal: "\u{2500}",
};

pub const ROUNDED: Set = Set {
    top_left: "\u{256D}",
    top_right: "\u{256E}",
    bottom_left: "\u{2570}",
    bottom_right: "\u{256F}",
    vertical: "\u{2502}",
    horizontal: "\u{2500}",
};

pub const DOUBLE: Set = Set {
    top_left: "\u{2554}",
    top_right: "\u{2557}",
    bottom_left: "\u{255A}",
    bottom_right: "\u{255D}",
    vertical: "\u{2551}",
    horizontal: "\u{2550}",
};

pub const THICK: Set = Set {
    top_left: "\u{250F}",
    top_right: "\u{2513}",
    bottom_left: "\u{2517}",
    bottom_right: "\u{251B}",
    vertical: "\u{2503}",
    horizontal: "\u{2501}",
};

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-033
    #[test]
    fn plain_matches_upstream_glyphs() {
        assert_eq!(PLAIN.top_left, "\u{250C}");
        assert_eq!(PLAIN.horizontal, "\u{2500}");
        assert_eq!(PLAIN.vertical, "\u{2502}");
        assert_eq!(PLAIN.bottom_right, "\u{2518}");
    }

    // Covers: ZDEP-033
    #[test]
    fn each_border_type_set_is_distinct() {
        assert_ne!(PLAIN.top_left, ROUNDED.top_left);
        assert_ne!(PLAIN.horizontal, DOUBLE.horizontal);
        assert_ne!(THICK.vertical, DOUBLE.vertical);
    }
}
