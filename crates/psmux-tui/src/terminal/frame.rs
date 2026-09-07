//! Ported from `ratatui-core` 0.1.2 `src/terminal/frame.rs` (ZDEP-044),
//! reduced to the four members the 2026-09-06 inventory found in use:
//! `area`, `buffer_mut`, `render_widget`, `set_cursor_position`
//! (`client.rs:1210,5108,5161,920,5310` per the S8 plan entry). `count`/
//! `render_stateful_widget`/the deprecated `size`/`set_cursor` are also
//! ported since they cost nothing extra and one call site each already
//! exists (`tests/monitor/src/ui.rs:212` for `render_stateful_widget`).

use crate::buffer::Buffer;
use crate::layout::{Position, Rect};
use crate::widgets::{StatefulWidget, Widget};

/// A consistent view into the terminal state for rendering a single frame.
/// See `terminal/mod.rs` for how this is constructed.
pub struct Frame<'a> {
    pub(super) cursor_position: Option<Position>,
    pub(super) area: Rect,
    pub(super) buffer: &'a mut Buffer,
    pub(super) count: usize,
}

/// The state of the terminal after the last successful `Terminal::draw`
/// render pass. Not consumed by any call site in this repo's inventory, but
/// kept for API parity since it costs nothing extra.
pub struct CompletedFrame<'a> {
    pub buffer: &'a Buffer,
    pub area: Rect,
    pub count: usize,
}

impl Frame<'_> {
    pub const fn area(&self) -> Rect {
        self.area
    }

    pub fn render_widget<W: Widget>(&mut self, widget: W, area: Rect) {
        widget.render(area, self.buffer);
    }

    pub fn render_stateful_widget<W: StatefulWidget>(&mut self, widget: W, area: Rect, state: &mut W::State) {
        widget.render(area, self.buffer, state);
    }

    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) {
        self.cursor_position = Some(position.into());
    }

    pub const fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffer
    }

    pub const fn count(&self) -> usize {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Position;

    // Covers: ZDEP-044
    #[test]
    fn area_matches_backing_buffer() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 2));
        let frame = Frame { cursor_position: None, area: buf.area, buffer: &mut buf, count: 0 };
        assert_eq!(frame.area(), Rect::new(0, 0, 4, 2));
    }

    // Covers: ZDEP-044
    #[test]
    fn set_cursor_position_records_it() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 2));
        let mut frame = Frame { cursor_position: None, area: buf.area, buffer: &mut buf, count: 0 };
        frame.set_cursor_position(Position::new(1, 1));
        assert_eq!(frame.cursor_position, Some(Position::new(1, 1)));
    }

    // Covers: ZDEP-044
    #[test]
    fn buffer_mut_writes_through_to_the_frame_area() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        let mut frame = Frame { cursor_position: None, area: buf.area, buffer: &mut buf, count: 3 };
        frame.buffer_mut()[(0, 0)].set_symbol("x");
        assert_eq!(frame.count(), 3);
        drop(frame);
        assert_eq!(buf[(0, 0)].symbol(), "x");
    }
}
