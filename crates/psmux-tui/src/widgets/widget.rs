//! Ported from `ratatui-core` 0.1.2 `src/widgets/widget.rs` (ZDEP-033),
//! reduced to the bare trait: this repo implements `Widget` for zero of its
//! own types (`grep -rn "impl Widget for"` outside this crate is empty), so
//! upstream's blanket impls for `&str`/`String`/`Option<W>` are not ported
//! -- only the ported widgets in this crate need the trait to exist.

use crate::buffer::Buffer;
use crate::layout::Rect;

/// A `Widget` draws itself into `buf` within `area`.
pub trait Widget {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized;
}
