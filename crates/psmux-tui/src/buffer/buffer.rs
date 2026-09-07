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

    /// Resizes the buffer so `area` matches the given rect, truncating or
    /// padding with default cells (ZDEP-042, needed by `Terminal::resize`).
    /// Ported from `ratatui-core` 0.1.2's own `Buffer::resize`, minus the
    /// `merge`/`diff`-adjacent bookkeeping this reduced `Buffer` doesn't carry.
    pub fn resize(&mut self, area: Rect) {
        let len = area.width as usize * area.height as usize;
        if self.content.len() > len {
            self.content.truncate(len);
        } else {
            self.content.resize(len, Cell::default());
        }
        self.area = area;
    }

    /// Resets every cell to its default value in place (ZDEP-042), used by
    /// `Terminal::swap_buffers`/`clear` to force a full redraw on the next
    /// diff without reallocating.
    pub fn reset(&mut self) {
        for cell in &mut self.content {
            *cell = Cell::default();
        }
    }

    /// Yields `(x, y, &Cell)` for every cell in `self` that differs from the
    /// corresponding cell in `other` (ZDEP-042): a plain cell-by-cell
    /// comparison, unlike upstream's `BufferDiff`, which also special-cases
    /// multi-width-glyph trailing cells and VS16 emoji sequences -- this
    /// crate's `Cell` (ZDEP-031) has no width field to drive that logic, and
    /// no widget in the reduced surface (`render_text.rs`, ZDEP-033) blanks a
    /// wide glyph's trailing column itself, so there is nothing for a
    /// width-aware diff to additionally catch here.
    ///
    /// # Panics
    ///
    /// Panics if the two buffers have different `width` (mirrors upstream).
    pub fn diff<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = (u16, u16, &'a Cell)> {
        assert_eq!(self.area.width, other.area.width, "buffer areas must have the same width");
        let width = self.area.width as usize;
        let height = self.area.height.min(other.area.height) as usize;
        let len = width * height;
        self.content[..len.min(self.content.len())]
            .iter()
            .zip(other.content[..len.min(other.content.len())].iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(move |(i, (_, cell))| {
                let x = other.area.x + (i % width) as u16;
                let y = other.area.y + (i / width) as u16;
                (x, y, cell)
            })
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

    // Covers: ZDEP-042
    #[test]
    fn resize_grows_and_truncates() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 2));
        buf.resize(Rect::new(0, 0, 3, 3));
        assert_eq!(buf.content().len(), 9);
        buf.resize(Rect::new(0, 0, 1, 1));
        assert_eq!(buf.content().len(), 1);
    }

    // Covers: ZDEP-042
    #[test]
    fn reset_clears_every_cell() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        buf[(0, 0)].set_symbol("x");
        buf.reset();
        assert_eq!(buf[(0, 0)].symbol(), " ");
    }

    // Covers: ZDEP-042
    #[test]
    fn diff_yields_only_changed_cells() {
        let mut prev = Buffer::empty(Rect::new(0, 0, 3, 1));
        let mut next = Buffer::empty(Rect::new(0, 0, 3, 1));
        prev[(0, 0)].set_symbol("a");
        next[(0, 0)].set_symbol("a");
        next[(1, 0)].set_symbol("b");
        let changes: Vec<_> = prev.diff(&next).collect();
        assert_eq!(changes.len(), 1);
        assert_eq!((changes[0].0, changes[0].1), (1, 0));
        assert_eq!(changes[0].2.symbol(), "b");
    }

    // Covers: ZDEP-042
    #[test]
    fn diff_of_identical_buffers_is_empty() {
        let buf = Buffer::empty(Rect::new(0, 0, 2, 2));
        assert_eq!(buf.diff(&buf).count(), 0);
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
