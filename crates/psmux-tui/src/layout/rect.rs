//! Ported from `ratatui-core` 0.1.2 `src/layout/rect.rs` (ZDEP-028): fields,
//! `new`, `Default`, and `contains(Position)` only -- see `layout/mod.rs`
//! for what's deliberately excluded.

use super::Position;

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    /// Clamps `width`/`height` so `x + width` and `y + height` stay within
    /// `u16`, matching upstream's own overflow-safety guarantee.
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        let width = x.saturating_add(width) - x;
        let height = y.saturating_add(height) - y;
        Self { x, y, width, height }
    }

    pub const fn contains(self, position: Position) -> bool {
        let right = self.x.saturating_add(self.width);
        let bottom = self.y.saturating_add(self.height);
        position.x >= self.x && position.x < right && position.y >= self.y && position.y < bottom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-028
    #[test]
    fn new_clamps_overflowing_width_and_height() {
        let rect = Rect::new(u16::MAX - 100, u16::MAX - 1000, 200, 2000);
        assert_eq!(
            rect,
            Rect { x: u16::MAX - 100, y: u16::MAX - 1000, width: 100, height: 1000 }
        );
    }

    // Covers: ZDEP-028
    #[test]
    fn default_is_zeroed() {
        assert_eq!(Rect::default(), Rect { x: 0, y: 0, width: 0, height: 0 });
    }

    // Covers: ZDEP-028
    #[test]
    fn contains() {
        let rect = Rect::new(1, 2, 3, 4);
        assert!(rect.contains(Position::new(1, 2)));
        assert!(rect.contains(Position::new(3, 5)));
        assert!(!rect.contains(Position::new(0, 2)));
        assert!(!rect.contains(Position::new(4, 2)));
        assert!(!rect.contains(Position::new(1, 6)));
    }

    // Covers: ZDEP-028
    #[test]
    fn contains_empty_rect_never_true() {
        let rect = Rect::new(1, 1, 0, 0);
        assert!(!rect.contains(Position::new(1, 1)));
    }
}
