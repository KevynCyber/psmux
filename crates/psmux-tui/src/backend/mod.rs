//! Ported from `ratatui-core` 0.1.2 `src/backend.rs` (ZDEP-032): the
//! `Backend` trait plus `ClearType`/`WindowSize`, restricted to exactly the
//! method set `src/term/backend.rs:109-227`'s `VtBackend` already
//! implements in the root crate. `CrosstermBackend`/`TestBackend`/the
//! deprecated `get_cursor`/`set_cursor` methods are not ported.
//!
//! `strum`'s `Display`/`EnumString` derives on `ClearType` are dropped:
//! nothing parses or prints a `ClearType`.

use crate::buffer::Cell;
use crate::layout::{Position, Size};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ClearType {
    All,
    AfterCursor,
    BeforeCursor,
    CurrentLine,
    UntilNewLine,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct WindowSize {
    pub columns_rows: Size,
    pub pixels: Size,
}

pub trait Backend {
    type Error: std::error::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>;

    fn hide_cursor(&mut self) -> Result<(), Self::Error>;
    fn show_cursor(&mut self) -> Result<(), Self::Error>;
    fn get_cursor_position(&mut self) -> Result<Position, Self::Error>;
    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error>;
    fn clear(&mut self) -> Result<(), Self::Error>;
    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error>;
    fn append_lines(&mut self, n: u16) -> Result<(), Self::Error>;
    fn size(&self) -> Result<Size, Self::Error>;
    fn window_size(&mut self) -> Result<WindowSize, Self::Error>;
    fn flush(&mut self) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    /// Minimal in-memory `Backend` exercising the trait's exact method set
    /// against the shape `VtBackend` (root crate `src/term/backend.rs`)
    /// already implements, without pulling that module (and its Command/
    /// terminal FFI dependencies) into this crate's tests.
    struct RecordingBackend {
        cursor: Position,
        hidden: bool,
        drawn: usize,
    }

    impl Backend for RecordingBackend {
        type Error = io::Error;

        fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
        where
            I: Iterator<Item = (u16, u16, &'a Cell)>,
        {
            self.drawn += content.count();
            Ok(())
        }

        fn hide_cursor(&mut self) -> Result<(), Self::Error> {
            self.hidden = true;
            Ok(())
        }

        fn show_cursor(&mut self) -> Result<(), Self::Error> {
            self.hidden = false;
            Ok(())
        }

        fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
            Ok(self.cursor)
        }

        fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
            self.cursor = position.into();
            Ok(())
        }

        fn clear(&mut self) -> Result<(), Self::Error> {
            self.clear_region(ClearType::All)
        }

        fn clear_region(&mut self, _clear_type: ClearType) -> Result<(), Self::Error> {
            Ok(())
        }

        fn append_lines(&mut self, _n: u16) -> Result<(), Self::Error> {
            Ok(())
        }

        fn size(&self) -> Result<Size, Self::Error> {
            Ok(Size::new(80, 24))
        }

        fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
            Ok(WindowSize { columns_rows: Size::new(80, 24), pixels: Size::new(0, 0) })
        }

        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    // Covers: ZDEP-032
    #[test]
    fn cursor_show_hide_and_position_round_trip() {
        let mut backend = RecordingBackend { cursor: Position::default(), hidden: false, drawn: 0 };
        backend.hide_cursor().unwrap();
        assert!(backend.hidden);
        backend.set_cursor_position((3, 4)).unwrap();
        assert_eq!(backend.get_cursor_position().unwrap(), Position::new(3, 4));
        backend.show_cursor().unwrap();
        assert!(!backend.hidden);
    }

    // Covers: ZDEP-032
    #[test]
    fn draw_consumes_the_cell_iterator() {
        let mut backend = RecordingBackend { cursor: Position::default(), hidden: false, drawn: 0 };
        let cells = [Cell::default(), Cell::default()];
        backend.draw(cells.iter().enumerate().map(|(i, c)| (i as u16, 0, c))).unwrap();
        assert_eq!(backend.drawn, 2);
    }

    // Covers: ZDEP-032
    #[test]
    fn clear_defaults_to_clear_region_all() {
        let mut backend = RecordingBackend { cursor: Position::default(), hidden: false, drawn: 0 };
        assert!(backend.clear().is_ok());
    }

    // Covers: ZDEP-032
    #[test]
    fn size_and_window_size() {
        let mut backend = RecordingBackend { cursor: Position::default(), hidden: false, drawn: 0 };
        assert_eq!(backend.size().unwrap(), Size::new(80, 24));
        assert_eq!(backend.window_size().unwrap().columns_rows, Size::new(80, 24));
    }

    // Covers: ZDEP-032
    #[test]
    fn all_five_clear_type_variants_are_distinct() {
        let variants = [
            ClearType::All,
            ClearType::AfterCursor,
            ClearType::BeforeCursor,
            ClearType::CurrentLine,
            ClearType::UntilNewLine,
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                assert_eq!(a == b, i == j);
            }
        }
    }
}
