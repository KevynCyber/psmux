//! Ported from `ratatui-core` 0.1.2 `src/terminal.rs` +
//! `src/terminal/{init,backend,buffers,cursor,render,resize}.rs` (ZDEP-044),
//! reduced to the FULLSCREEN viewport only: nothing in this repo's 2026-09-06
//! inventory calls `Terminal::with_options`, so `Viewport`/`TerminalOptions`/
//! `Viewport::Inline`/`Viewport::Fixed` and the inline-scrolling machinery in
//! upstream's `terminal/inline.rs` are not ported. `insert_before` (inline-
//! only) and `try_draw` (no fallible render callback in this repo's
//! inventory) are likewise dropped. `Terminal::clear`'s cursor-position
//! save/restore dance is simplified to the fullscreen case: just
//! `ClearType::All` plus resetting the back buffer.

mod frame;

pub use frame::{CompletedFrame, Frame};

use crate::backend::{Backend, ClearType};
use crate::buffer::Buffer;
use crate::layout::{Position, Rect};

/// Ties a [`Backend`] to a double-buffered, diffing renderer, always
/// covering the full backend area (see the module doc for what's dropped).
pub struct Terminal<B: Backend> {
    backend: B,
    buffers: [Buffer; 2],
    current: usize,
    hidden_cursor: bool,
    area: Rect,
    frame_count: usize,
}

impl<B: Backend> Terminal<B> {
    /// Creates a new `Terminal`, sizing both internal buffers to the
    /// backend's current area.
    pub fn new(backend: B) -> Result<Self, B::Error> {
        let area: Rect = backend.size()?.into();
        Ok(Self {
            backend,
            buffers: [Buffer::empty(area), Buffer::empty(area)],
            current: 0,
            hidden_cursor: false,
            area,
            frame_count: 0,
        })
    }

    pub const fn backend(&self) -> &B {
        &self.backend
    }

    pub const fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn size(&self) -> Result<crate::layout::Size, B::Error> {
        self.backend.size()
    }

    pub const fn current_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.current]
    }

    /// Returns a [`Frame`] for manual rendering; see upstream's own doc for
    /// why this is an escape hatch (tests, or callers managing presentation
    /// themselves) rather than the normal `draw` path.
    pub fn get_frame(&mut self) -> Frame<'_> {
        let count = self.frame_count;
        let area = self.area;
        Frame { cursor_position: None, area, buffer: &mut self.buffers[self.current], count }
    }

    /// Diffs the current buffer against the previous one and sends only the
    /// changed cells to the backend.
    pub fn flush(&mut self) -> Result<(), B::Error> {
        let previous_buffer = &self.buffers[1 - self.current];
        let current_buffer = &self.buffers[self.current];
        let updates = previous_buffer.diff(current_buffer);
        self.backend.draw(updates)
    }

    /// Clears the inactive buffer and swaps it with the current buffer.
    pub fn swap_buffers(&mut self) {
        self.buffers[1 - self.current].reset();
        self.current = 1 - self.current;
    }

    pub fn hide_cursor(&mut self) -> Result<(), B::Error> {
        self.backend.hide_cursor()?;
        self.hidden_cursor = true;
        Ok(())
    }

    pub fn show_cursor(&mut self) -> Result<(), B::Error> {
        self.backend.show_cursor()?;
        self.hidden_cursor = false;
        Ok(())
    }

    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), B::Error> {
        self.backend.set_cursor_position(position)
    }

    /// Clears the whole terminal and resets the back buffer so the next
    /// `draw` is treated as a full redraw (fullscreen-only, see module doc).
    pub fn clear(&mut self) -> Result<(), B::Error> {
        self.backend.clear_region(ClearType::All)?;
        self.buffers[1 - self.current].reset();
        Ok(())
    }

    /// Resizes both internal buffers to `area` and forces a full redraw.
    pub fn resize(&mut self, area: Rect) -> Result<(), B::Error> {
        self.buffers[self.current].resize(area);
        self.buffers[1 - self.current].resize(area);
        self.area = area;
        self.clear()
    }

    /// Re-queries the backend size and resizes if it changed; called
    /// automatically by [`Terminal::draw`].
    fn autoresize(&mut self) -> Result<(), B::Error> {
        let area: Rect = self.backend.size()?.into();
        if area != self.area {
            self.resize(area)?;
        }
        Ok(())
    }

    /// Runs one render pass: autoresize, call `render_callback` with a
    /// [`Frame`], flush the buffer diff, apply the frame's requested cursor
    /// state, swap buffers, and flush the backend.
    pub fn draw<F>(&mut self, render_callback: F) -> Result<CompletedFrame<'_>, B::Error>
    where
        F: FnOnce(&mut Frame),
    {
        self.autoresize()?;

        let mut frame = self.get_frame();
        render_callback(&mut frame);
        let cursor_position = frame.cursor_position;

        self.flush()?;
        match cursor_position {
            None => self.hide_cursor()?,
            Some(position) => {
                self.show_cursor()?;
                self.set_cursor_position(position)?;
            }
        }
        self.swap_buffers();
        self.backend.flush()?;

        let completed = CompletedFrame {
            buffer: &self.buffers[1 - self.current],
            area: self.area,
            count: self.frame_count,
        };
        self.frame_count = self.frame_count.wrapping_add(1);
        Ok(completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::TestBackend;
    use crate::layout::Size;

    // Covers: ZDEP-044
    #[test]
    fn new_sizes_buffers_to_backend_area() {
        let backend = TestBackend::new(5, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        assert_eq!(terminal.size().unwrap(), Size::new(5, 3));
        assert_eq!(terminal.get_frame().area(), Rect::new(0, 0, 5, 3));
    }

    // Covers: ZDEP-044
    #[test]
    fn draw_writes_only_the_diff_to_the_backend() {
        let backend = TestBackend::new(3, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| { f.buffer_mut()[(1, 0)].set_symbol("x"); }).unwrap();
        assert_eq!(terminal.backend().buffer()[(1, 0)].symbol(), "x");
        assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), " ");
    }

    // Covers: ZDEP-044
    #[test]
    fn draw_hides_cursor_by_default_and_shows_it_when_requested() {
        let backend = TestBackend::new(3, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|_f| {}).unwrap();
        assert!(!terminal.backend().cursor_visible());
        terminal.draw(|f| f.set_cursor_position((2, 0))).unwrap();
        assert!(terminal.backend().cursor_visible());
        assert_eq!(terminal.backend().cursor_position(), Position::new(2, 0));
    }

    // Covers: ZDEP-044
    #[test]
    fn each_draw_redraws_from_an_empty_current_buffer() {
        let backend = TestBackend::new(3, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| { f.buffer_mut()[(0, 0)].set_symbol("x"); }).unwrap();
        // Second frame writes nothing -- the previous "x" must be cleared.
        terminal.draw(|_f| {}).unwrap();
        assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), " ");
    }

    // Covers: ZDEP-044
    #[test]
    fn resize_updates_backend_facing_area_and_clears() {
        let backend = TestBackend::new(3, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| { f.buffer_mut()[(0, 0)].set_symbol("x"); }).unwrap();
        terminal.resize(Rect::new(0, 0, 4, 2)).unwrap();
        assert_eq!(terminal.get_frame().area(), Rect::new(0, 0, 4, 2));
    }

    // Covers: ZDEP-044
    #[test]
    fn backend_and_backend_mut_expose_the_same_instance() {
        let backend = TestBackend::new(2, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.backend_mut().set_cursor_position((1, 1)).unwrap();
        assert_eq!(terminal.backend().cursor_position(), Position::new(1, 1));
    }
}
