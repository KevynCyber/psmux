//! Ported from `ratatui-widgets` 0.3.2 `src/list.rs` + `src/list/*.rs`
//! (ZDEP-038), reduced to `List::new`/`.block`/`.highlight_style` rendered
//! via `StatefulWidget` (this repo's only call site, tests/monitor,
//! always goes through `render_stateful_widget`). Not ported:
//! `highlight_symbol`/`repeat_highlight_symbol`/`highlight_spacing`/
//! `scroll_padding`/`ListDirection` (all unused) and upstream's
//! variable-item-height windowing (`ListItem::height()` -- every item here
//! is exactly one `Line`, so this port hardcodes item height 1 rather than
//! porting the general `get_items_bounds` algorithm).

mod item;
mod state;

pub use item::ListItem;
pub use state::ListState;

use crate::buffer::Buffer;
use crate::layout::{Alignment, Rect};
use crate::style::Style;
use crate::widgets::render_text::{fill_style, render_line};
use crate::widgets::{Block, StatefulWidget, Widget};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct List<'a> {
    items: Vec<ListItem<'a>>,
    block: Option<Block<'a>>,
    style: Style,
    highlight_style: Style,
}

impl<'a> List<'a> {
    pub fn new(items: Vec<ListItem<'a>>) -> Self {
        Self { items, ..Default::default() }
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn highlight_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.highlight_style = style.into();
        self
    }
}

impl<'a> StatefulWidget for &List<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        fill_style(area, buf, self.style);
        if let Some(block) = &self.block {
            block.render(area, buf);
        }
        let inner = self.block.as_ref().map_or(area, |b| b.inner(area));
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        if self.items.is_empty() {
            state.select(None);
            return;
        }

        if let Some(s) = state.selected() {
            if s >= self.items.len() {
                state.select(Some(self.items.len() - 1));
            }
        }

        let height = inner.height as usize;
        let mut offset = state.offset().min(self.items.len().saturating_sub(1));
        if let Some(sel) = state.selected() {
            if sel < offset {
                offset = sel;
            } else if sel >= offset + height {
                offset = sel + 1 - height;
            }
        }
        state.set_offset(offset);

        let end = (offset + height).min(self.items.len());
        for (i, item) in self.items.iter().enumerate().skip(offset).take(end - offset) {
            let row = inner.y + (i - offset) as u16;
            let row_area = Rect::new(inner.x, row, inner.width, 1);
            render_line(&item.content, row_area, buf, 0, Alignment::Left, 0);
            if state.selected() == Some(i) {
                fill_style(row_area, buf, self.highlight_style);
            }
        }
    }
}

impl<'a> StatefulWidget for List<'a> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        (&self).render(area, buf, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Modifier;
    use crate::text::Line;

    fn items(n: usize) -> Vec<ListItem<'static>> {
        (0..n).map(|i| ListItem::new(Line::from(format!("item{i}")))).collect()
    }

    fn buf(w: u16, h: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, w, h))
    }

    // Covers: ZDEP-038
    #[test]
    fn renders_each_item_on_its_own_row() {
        let mut b = buf(6, 3);
        let list = List::new(items(3));
        let mut state = ListState::default();
        (&list).render(b.area, &mut b, &mut state);
        assert_eq!(b[(0, 0)].symbol(), "i");
        assert_eq!(b[(0, 1)].symbol(), "i");
        assert_eq!(b[(0, 2)].symbol(), "i");
    }

    // Covers: ZDEP-038
    #[test]
    fn empty_items_clears_selection() {
        let mut b = buf(6, 3);
        let list = List::new(Vec::new());
        let mut state = ListState::default();
        state.select(Some(0));
        (&list).render(b.area, &mut b, &mut state);
        assert_eq!(state.offset(), 0);
    }

    // Covers: ZDEP-038
    #[test]
    fn scrolls_offset_so_selection_stays_visible() {
        let mut b = buf(6, 2);
        let list = List::new(items(5));
        let mut state = ListState::default();
        state.select(Some(4));
        (&list).render(b.area, &mut b, &mut state);
        assert_eq!(state.offset(), 3);
    }

    // Covers: ZDEP-038
    #[test]
    fn selected_row_gets_highlight_style() {
        let mut b = buf(6, 3);
        let list = List::new(items(3)).highlight_style(Style::default().add_modifier(Modifier::BOLD));
        let mut state = ListState::default();
        state.select(Some(1));
        (&list).render(b.area, &mut b, &mut state);
        assert!(b[(0, 1)].modifier.contains(Modifier::BOLD));
        assert!(!b[(0, 0)].modifier.contains(Modifier::BOLD));
    }

    // Covers: ZDEP-038
    #[test]
    fn out_of_bounds_selection_clamps_to_last_item() {
        let mut b = buf(6, 3);
        let list = List::new(items(2));
        let mut state = ListState::default();
        state.select(Some(50));
        (&list).render(b.area, &mut b, &mut state);
        assert_eq!(state.selected(), Some(1));
    }

    // Covers: ZDEP-038
    #[test]
    fn block_reserves_inner_area() {
        use crate::widgets::Borders;
        let mut b = buf(6, 3);
        let list = List::new(items(1)).block(Block::default().borders(Borders::ALL));
        let mut state = ListState::default();
        (&list).render(b.area, &mut b, &mut state);
        assert_eq!(b[(1, 1)].symbol(), "i");
    }
}
