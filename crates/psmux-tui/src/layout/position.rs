//! Ported from `ratatui-core` 0.1.2 `src/layout/position.rs` (ZDEP-028):
//! fields, `new`, `Default`, and `From<(u16, u16)>` only.

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

impl Position {
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

impl From<(u16, u16)> for Position {
    fn from((x, y): (u16, u16)) -> Self {
        Self::new(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-028
    #[test]
    fn from_tuple() {
        assert_eq!(Position::from((1, 2)), Position::new(1, 2));
    }

    // Covers: ZDEP-028
    #[test]
    fn default_is_origin() {
        assert_eq!(Position::default(), Position::new(0, 0));
    }
}
