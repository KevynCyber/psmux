//! Ported from `ratatui-core` 0.1.2 `src/buffer/buffer.rs` (ZDEP-031),
//! reduced to `empty`/`content()`/the `content`+`area` fields/`Index`+
//! `IndexMut<(u16, u16)>`/`Clone`. `new_empty`/`get`/`get_mut`/`cell`/
//! `cell_mut`/`set_string`/`set_style`/`resize`/`diff` are not ported.

use super::Cell;
use crate::layout::{Position, Rect};
use std::ops::{Index, IndexMut};

#[derive(Debug, Clone, PartialEq)]
pub struct Buffer {
    pub area: Rect,
    pub content: Vec<Cell>,
}

impl Buffer {
    pub fn empty(area: Rect) -> Self {
        let size = area.width as usize * area.height as usize;
        Self { area, content: vec![Cell::default(); size] }
    }

    pub fn content(&self) -> &[Cell] {
        &self.content
    }

    fn index_of(&self, x: u16, y: u16) -> usize {
        (y - self.area.y) as usize * self.area.width as usize + (x - self.area.x) as usize
    }
}

impl<P: Into<Position>> Index<P> for Buffer {
    type Output = Cell;

    fn index(&self, position: P) -> &Self::Output {
        let position = position.into();
        &self.content[self.index_of(position.x, position.y)]
    }
}

impl<P: Into<Position>> IndexMut<P> for Buffer {
    fn index_mut(&mut self, position: P) -> &mut Self::Output {
        let position = position.into();
        let index = self.index_of(position.x, position.y);
        &mut self.content[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: ZDEP-031
    #[test]
    fn empty_fills_area_with_default_cells() {
        let buf = Buffer::empty(Rect::new(0, 0, 3, 2));
        assert_eq!(buf.content().len(), 6);
        assert_eq!(buf.area, Rect::new(0, 0, 3, 2));
    }

    // Covers: ZDEP-031
    #[test]
    fn index_and_index_mut_by_tuple_and_position() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 2));
        buf[(1, 1)].set_symbol("x");
        assert_eq!(buf[Position::new(1, 1)].symbol(), "x");
    }

    // Covers: ZDEP-031
    #[test]
    fn index_respects_non_origin_area() {
        let mut buf = Buffer::empty(Rect::new(5, 5, 2, 2));
        buf[(5, 5)].set_symbol("a");
        buf[(6, 6)].set_symbol("b");
        assert_eq!(buf[(5, 5)].symbol(), "a");
        assert_eq!(buf[(6, 6)].symbol(), "b");
    }

    // Covers: ZDEP-031
    #[test]
    fn clone_is_independent() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        buf[(0, 0)].set_symbol("a");
        let mut cloned = buf.clone();
        cloned[(0, 0)].set_symbol("b");
        assert_eq!(buf[(0, 0)].symbol(), "a");
        assert_eq!(cloned[(0, 0)].symbol(), "b");
    }
}
