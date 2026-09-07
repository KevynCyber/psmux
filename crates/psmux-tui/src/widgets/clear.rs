//! Ported from `ratatui-widgets` 0.3.2 `src/clear.rs` (ZDEP-036): a unit
//! widget that resets every cell in `area` to its default (used ~26 times
//! in this repo to clear space for a popup before drawing over it).
//! `Cell::reset()` is not ported (S8a, ZDEP-031) so this assigns
//! `Cell::default()` directly instead -- same effect.

use crate::buffer::{Buffer, Cell};
use crate::layout::{Position, Rect};
use crate::widgets::Widget;

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Clear;

impl Widget for Clear {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                buf[Position::new(x, y)] = Cell::default();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-036
    #[test]
    fn resets_every_cell_in_area() {
        let mut b = Buffer::empty(Rect::new(0, 0, 3, 2));
        b[(1, 1)].set_symbol("x");
        Clear.render(b.area, &mut b);
        assert_eq!(b[(1, 1)].symbol(), " ");
    }

    // Covers: ZDEP-036
    #[test]
    fn leaves_cells_outside_area_untouched() {
        let mut b = Buffer::empty(Rect::new(0, 0, 3, 3));
        b[(0, 0)].set_symbol("x");
        Clear.render(Rect::new(1, 1, 2, 2), &mut b);
        assert_eq!(b[(0, 0)].symbol(), "x");
    }
}
