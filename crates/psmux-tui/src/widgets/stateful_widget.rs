//! Ported from `ratatui-core` 0.1.2 `src/widgets/stateful_widget.rs`
//! (ZDEP-033), reduced to the bare trait -- see `widget.rs`'s doc note for
//! why no blanket impls are ported.

use crate::buffer::Buffer;
use crate::layout::Rect;

/// A `StatefulWidget` draws itself into `buf` within `area`, reading and
/// updating `state` across draw calls (e.g. `List`'s scroll offset).
pub trait StatefulWidget {
    type State;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State);
}
